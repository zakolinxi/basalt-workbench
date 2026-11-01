use std::time::Instant;

use tauri::Manager;
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;

use crate::db::{get_model_endpoint, now, save_model_endpoint};
use crate::error::{AppError, AppResult};
use crate::model_interfaces;
use crate::models::ModelEndpoint;
use crate::state::AppState;

use super::{chat_adapter, load_runtime, ChatMessage, ChatRequest};

async fn probe(
    state: &AppState,
    provider_id: &str,
    model: &str,
    protocol: &str,
) -> Result<u64, AppError> {
    let mut rt = load_runtime(state, provider_id)?;
    rt.provider.protocol = protocol.to_string();
    rt.provider.stream = false;
    rt.provider.timeout_secs = rt.provider.timeout_secs.clamp(5, 20);
    let adapter = chat_adapter(protocol)?;
    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage::text("user", "OK")],
        tools: Vec::new(),
        max_tokens: Some(1),
        temperature: Some(0.0),
        reasoning_effort: None,
        cache_prompt: false,
    };
    let (tx, _rx) = unbounded_channel();
    let token = CancellationToken::new();
    let started = Instant::now();
    adapter
        .chat(&state.http, &rt, &request, &tx, &token)
        .await?;
    Ok(started.elapsed().as_millis() as u64)
}

pub async fn optimize_model(
    state: &AppState,
    provider_id: &str,
    model: &str,
    force: bool,
) -> AppResult<ModelEndpoint> {
    let rt = load_runtime(state, provider_id)?;
    if rt.provider.category != "chat" || rt.provider.is_template {
        return Err(AppError::msg("只有已保存的对话供应商可以自动检测接口"));
    }
    let exists = rt.provider.models.iter().any(|entry| entry.id == model);
    if !exists {
        return Err(AppError::msg(format!("供应商中不存在模型 {model}")));
    }
    if !force {
        let existing = {
            let conn = state.db();
            get_model_endpoint(&conn, provider_id, model)
        };
        if let Some(endpoint) = existing {
            return Ok(endpoint);
        }
    }

    let candidates = model_interfaces::protocol_candidates(&rt.provider.protocol, model);
    if candidates.len() == 1 {
        let endpoint = ModelEndpoint {
            provider_id: provider_id.to_string(),
            model: model.to_string(),
            protocol: candidates[0].clone(),
            latency_ms: None,
            checked_at: now(),
        };
        let conn = state.db();
        save_model_endpoint(&conn, &endpoint)?;
        return Ok(endpoint);
    }

    let recommended = model_interfaces::profile(model)
        .map(|profile| profile.recommended_protocol);
    let mut successes: Vec<(String, u64)> = Vec::new();
    let mut last_error: Option<AppError> = None;
    for protocol in &candidates {
        match probe(state, provider_id, model, protocol).await {
            Ok(latency) => successes.push((protocol.clone(), latency)),
            Err(error) => last_error = Some(error),
        }
    }

    if successes.is_empty() {
        if force {
            let detail = last_error
                .map(|error| error.user_message())
                .unwrap_or_else(|| "所有候选接口均不可用".into());
            return Err(AppError::msg(format!("接口检测失败：{detail}")));
        }
        let endpoint = ModelEndpoint {
            provider_id: provider_id.to_string(),
            model: model.to_string(),
            protocol: rt.provider.protocol.clone(),
            latency_ms: None,
            checked_at: now(),
        };
        let conn = state.db();
        save_model_endpoint(&conn, &endpoint)?;
        return Ok(endpoint);
    }

    successes.sort_by_key(|(_, latency)| *latency);
    let fastest = successes[0].1;
    let within_fast_window = |latency: u64| latency <= fastest.saturating_add(60);
    let chosen = recommended
        .as_ref()
        .and_then(|protocol| {
            successes
                .iter()
                .find(|(candidate, latency)| candidate == protocol && within_fast_window(*latency))
        })
        .or_else(|| successes.first())
        .cloned()
        .expect("successes is not empty");

    let endpoint = ModelEndpoint {
        provider_id: provider_id.to_string(),
        model: model.to_string(),
        protocol: chosen.0.clone(),
        latency_ms: Some(chosen.1),
        checked_at: now(),
    };
    let conn = state.db();
    save_model_endpoint(&conn, &endpoint)?;
    state.log(
        "INFO",
        &format!(
            "模型接口检测：{provider_id}/{model} → {}（{}ms）",
            endpoint.protocol, chosen.1
        ),
    );
    Ok(endpoint)
}

pub fn schedule_model_optimization(state: &AppState, provider_id: &str, model: &str) {
    let exists = {
        let conn = state.db();
        get_model_endpoint(&conn, provider_id, model).is_some()
    };
    if exists {
        return;
    }
    let app = state.app.clone();
    let provider_id = provider_id.to_string();
    let model = model.to_string();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = optimize_model(&state, &provider_id, &model, false).await {
            state.log(
                "WARN",
                &format!(
                    "模型接口检测失败：{provider_id}/{model}：{}",
                    error.user_message()
                ),
            );
        }
    });
}

pub fn endpoint_view(
    endpoint: ModelEndpoint,
) -> crate::models::ModelEndpointView {
    let profile = model_interfaces::profile(&endpoint.model);
    crate::models::ModelEndpointView {
        model: endpoint.model,
        protocol: endpoint.protocol,
        latency_ms: endpoint.latency_ms,
        checked_at: endpoint.checked_at,
        recommended_protocol: profile
            .as_ref()
            .map(|profile| profile.recommended_protocol.clone()),
        cost_tier: profile.as_ref().map(|profile| profile.cost_tier.clone()),
        speed: profile.as_ref().map(|profile| profile.speed.clone()),
        note: profile.as_ref().map(|profile| profile.note.clone()),
        source: profile.map(|profile| profile.source),
    }
}
