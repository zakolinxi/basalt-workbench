use std::time::Instant;

use rusqlite::params;
use tauri::State;
use uuid::Uuid;

use crate::db::{get_provider_row, list_provider_rows, now};
use crate::error::{redact, AppError, AppResult};
use crate::models::{
    DiscoveredModel, ModelCapabilities, Provider, ProviderInput, ProviderModel,
    ProviderTestResult, CAP_FAILED, CAP_UNAVAILABLE, CAP_UNSUPPORTED, CAP_UNVERIFIED, CAP_VERIFIED,
};
use crate::providers::{
    capability_baseline, chat_adapter, image_adapter, load_runtime, mark_capabilities,
};
use crate::secrets;
use crate::state::AppState;

fn attach_key(mut provider: Provider) -> Provider {
    provider.has_key = secrets::has_api_key(&provider.id);
    provider
}

fn valid_state(value: &str) -> bool {
    matches!(
        value,
        CAP_VERIFIED | CAP_UNVERIFIED | CAP_UNSUPPORTED | CAP_FAILED | CAP_UNAVAILABLE
    )
}

fn validate_caps(caps: &ModelCapabilities) -> AppResult<()> {
    for value in [
        &caps.chat,
        &caps.stream,
        &caps.tools,
        &caps.vision,
        &caps.image_gen,
        &caps.image_edit,
        &caps.json,
        &caps.long_context,
        &caps.multi_tool,
        &caps.reasoning,
    ] {
        if !valid_state(value) {
            return Err(AppError::msg(format!("无效的能力状态：{value}")));
        }
    }
    Ok(())
}

fn validate_input(input: &ProviderInput) -> AppResult<()> {
    if input.name.trim().is_empty() || input.name.chars().count() > 120 {
        return Err(AppError::msg("供应商名称不能为空且不得超过 120 字符"));
    }
    if !matches!(input.category.as_str(), "chat" | "image") {
        return Err(AppError::msg("供应商分类必须是 chat 或 image"));
    }
    let protocols: &[&str] = if input.category == "chat" {
        &["openai_chat", "openai_responses", "anthropic", "gemini", "ollama"]
    } else {
        &["openai_images", "gemini_images"]
    };
    if !protocols.contains(&input.protocol.as_str()) {
        return Err(AppError::msg(format!(
            "协议 {} 不适用于 {} 分类",
            input.protocol, input.category
        )));
    }
    if !input.base_url.trim().is_empty() {
        let url = url::Url::parse(input.base_url.trim())
            .map_err(|e| AppError::msg(format!("Base URL 无效：{e}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::msg("Base URL 必须使用 http 或 https"));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(AppError::msg("Base URL 不能包含用户名或密码"));
        }
    }
    if !input.headers.is_null() && !input.headers.is_object() {
        return Err(AppError::msg("自定义请求头必须是 JSON 对象"));
    }
    if let Some(headers) = input.headers.as_object() {
        for (key, value) in headers {
            if key.trim().is_empty()
                || key.len() > 100
                || key.contains(':')
                || key.contains('\r')
                || key.contains('\n')
            {
                return Err(AppError::msg("请求头名称无效"));
            }
            let Some(text) = value.as_str() else {
                return Err(AppError::msg("请求头的值必须是字符串"));
            };
            if text.contains('\r') || text.contains('\n') {
                return Err(AppError::msg("请求头的值不能包含换行"));
            }
        }
    }
    if !input.params.is_null() && !input.params.is_object() {
        return Err(AppError::msg("自定义参数必须是 JSON 对象"));
    }
    if input.timeout_secs < 5 || input.timeout_secs > 600 {
        return Err(AppError::msg("超时时间必须在 5 到 600 秒之间"));
    }
    if input.models.len() > 300 {
        return Err(AppError::msg("模型数量不能超过 300"));
    }
    let mut seen = std::collections::HashSet::new();
    for model in &input.models {
        if model.id.trim().is_empty() || model.id.chars().count() > 200 {
            return Err(AppError::msg("模型 ID 不能为空且不得超过 200 字符"));
        }
        if !seen.insert(model.id.clone()) {
            return Err(AppError::msg(format!("模型 ID 重复：{}", model.id)));
        }
        validate_caps(&model.capabilities)?;
    }
    if let Some(key) = &input.api_key {
        if key.len() > 8192 {
            return Err(AppError::msg("API Key 过长"));
        }
    }
    Ok(())
}

fn ensure_default_models(input: &mut ProviderInput) {
    let baseline = ModelCapabilities::chat_baseline(input.stream);
    let image_baseline = ModelCapabilities::image_baseline(true, true);
    let make = |id: &str, caps: ModelCapabilities| ProviderModel {
        id: id.to_string(),
        name: id.to_string(),
        context_length: None,
        capabilities: caps,
        source: "manual".into(),
    };
    let defaults = [
        input.default_chat_model.clone(),
        input.default_vision_model.clone(),
        input.default_image_model.clone(),
    ];
    for default in defaults.into_iter().flatten() {
        if default.trim().is_empty() {
            continue;
        }
        if !input.models.iter().any(|model| model.id == default) {
            let caps = if input.category == "image" {
                image_baseline.clone()
            } else {
                baseline.clone()
            };
            input.models.push(make(default.trim(), caps));
        }
    }
    if input.default_chat_model.is_none() && input.category == "chat" {
        input.default_chat_model = input.models.first().map(|m| m.id.clone());
    }
    if input.default_image_model.is_none() && input.category == "image" {
        input.default_image_model = input.models.first().map(|m| m.id.clone());
    }
}

fn save_provider(state: &AppState, input: ProviderInput) -> AppResult<Provider> {
    validate_input(&input)?;
    let mut input = input;
    input.name = input.name.trim().to_string();
    input.base_url = input.base_url.trim().trim_end_matches('/').to_string();
    input.headers = if input.headers.is_null() {
        serde_json::json!({})
    } else {
        input.headers
    };
    input.params = if input.params.is_null() {
        serde_json::json!({})
    } else {
        input.params
    };
    ensure_default_models(&mut input);

    let conn = state.db();
    let updating = input
        .id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .map(|id| get_provider_row(&conn, id))
        .transpose()?;
    let id = input
        .id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = now();
    let is_template = updating.as_ref().map(|p| p.is_template).unwrap_or(false);
    let created_at = updating
        .as_ref()
        .map(|p| p.created_at.clone())
        .unwrap_or_else(|| timestamp.clone());

    if updating.is_some() {
        conn.execute(
            "UPDATE providers SET name = ?2, category = ?3, protocol = ?4, base_url = ?5,
             models_json = ?6, default_chat_model = ?7, default_vision_model = ?8,
             default_image_model = ?9, headers_json = ?10, params_json = ?11,
             timeout_secs = ?12, stream = ?13, enabled = ?14, updated_at = ?15
             WHERE id = ?1",
            params![
                id,
                input.name,
                input.category,
                input.protocol,
                input.base_url,
                serde_json::to_string(&input.models)?,
                input.default_chat_model,
                input.default_vision_model,
                input.default_image_model,
                input.headers.to_string(),
                input.params.to_string(),
                input.timeout_secs as i64,
                input.stream as i64,
                input.enabled as i64,
                timestamp
            ],
        )?;
    } else {
        conn.execute(
            "INSERT INTO providers(id, name, category, protocol, base_url, models_json,
             default_chat_model, default_vision_model, default_image_model, headers_json,
             params_json, timeout_secs, stream, enabled, is_template, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                id,
                input.name,
                input.category,
                input.protocol,
                input.base_url,
                serde_json::to_string(&input.models)?,
                input.default_chat_model,
                input.default_vision_model,
                input.default_image_model,
                input.headers.to_string(),
                input.params.to_string(),
                input.timeout_secs as i64,
                input.stream as i64,
                input.enabled as i64,
                is_template as i64,
                created_at,
                timestamp
            ],
        )?;
    }

    if input.clear_api_key {
        secrets::delete_api_key(&id)?;
    } else if let Some(key) = input.api_key.as_deref() {
        if !key.trim().is_empty() {
            secrets::set_api_key(&id, key)?;
        }
    }
    let provider = get_provider_row(&conn, &id)?;
    Ok(attach_key(provider))
}

#[tauri::command]
pub fn provider_list(state: State<'_, AppState>) -> AppResult<Vec<Provider>> {
    let conn = state.db();
    Ok(list_provider_rows(&conn)?
        .into_iter()
        .map(attach_key)
        .collect())
}

#[tauri::command]
pub fn provider_get(state: State<'_, AppState>, provider_id: String) -> AppResult<Provider> {
    let conn = state.db();
    Ok(attach_key(get_provider_row(&conn, &provider_id)?))
}

#[tauri::command]
pub fn provider_create(
    state: State<'_, AppState>,
    input: ProviderInput,
) -> AppResult<Provider> {
    let mut input = input;
    input.id = None;
    save_provider(&state, input)
}

#[tauri::command]
pub fn provider_update(
    state: State<'_, AppState>,
    input: ProviderInput,
) -> AppResult<Provider> {
    if input.id.as_deref().unwrap_or("").trim().is_empty() {
        return Err(AppError::msg("更新供应商需要 id"));
    }
    save_provider(&state, input)
}

#[tauri::command]
pub fn provider_delete(state: State<'_, AppState>, provider_id: String) -> AppResult<()> {
    {
        let conn = state.db();
        get_provider_row(&conn, &provider_id)?;
        conn.execute("DELETE FROM providers WHERE id = ?1", [&provider_id])?;
    }
    let _ = secrets::delete_api_key(&provider_id);
    {
        let conn = state.db();
        let mut settings = crate::db::load_settings(&conn);
        let routes = [
            &mut settings.routing.chat,
            &mut settings.routing.agent_plan,
            &mut settings.routing.agent_execute,
            &mut settings.routing.review,
            &mut settings.routing.vision,
            &mut settings.routing.image,
            &mut settings.routing.image_edit,
        ];
        for route in routes {
            if route.provider_id == provider_id {
                route.provider_id.clear();
                route.model.clear();
            }
        }
        crate::db::save_settings(&conn, &settings)?;
    }
    Ok(())
}

fn unique_provider_name(conn: &rusqlite::Connection, base: &str) -> AppResult<String> {
    let mut candidate = base.to_string();
    let mut index = 2;
    loop {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM providers WHERE name = ?1 AND is_template = 0",
            [&candidate],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Ok(candidate);
        }
        candidate = format!("{base} {index}");
        index += 1;
        if index > 99 {
            return Ok(format!("{base} {}", Uuid::new_v4()));
        }
    }
}

#[tauri::command]
pub fn provider_duplicate(
    state: State<'_, AppState>,
    provider_id: String,
) -> AppResult<Provider> {
    let source = {
        let conn = state.db();
        get_provider_row(&conn, &provider_id)?
    };
    let new_id = Uuid::new_v4().to_string();
    let timestamp = now();
    // 从模板创建的是新供应商，不是副本；只有复制已有配置才保留“副本”语义。
    let base_name = if source.is_template {
        source.name.clone()
    } else {
        format!("{} 副本", source.name)
    };
    let name = {
        let conn = state.db();
        unique_provider_name(&conn, &base_name)?
    };
    {
        let conn = state.db();
        conn.execute(
            "INSERT INTO providers(id, name, category, protocol, base_url, models_json,
             default_chat_model, default_vision_model, default_image_model, headers_json,
             params_json, timeout_secs, stream, enabled, is_template, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0, ?15, ?15)",
            params![
                new_id,
                name,
                source.category,
                source.protocol,
                source.base_url,
                serde_json::to_string(&source.models)?,
                source.default_chat_model,
                source.default_vision_model,
                source.default_image_model,
                source.headers.to_string(),
                source.params.to_string(),
                source.timeout_secs as i64,
                source.stream as i64,
                1i64,
                timestamp
            ],
        )?;
    }
    if let Some(key) = secrets::get_api_key(&provider_id) {
        secrets::set_api_key(&new_id, &key)?;
    }
    let conn = state.db();
    Ok(attach_key(get_provider_row(&conn, &new_id)?))
}

async fn run_test(state: &AppState, provider: &Provider) -> Result<Vec<DiscoveredModel>, AppError> {
    let rt = load_runtime(state, &provider.id)?;
    if provider.category == "chat" {
        let adapter = chat_adapter(&provider.protocol)?;
        adapter.list_models(&state.http, &rt).await
    } else {
        let adapter = image_adapter(&provider.protocol)?;
        adapter.list_models(&state.http, &rt).await
    }
}

fn merge_models(provider: &Provider, discovered: Vec<DiscoveredModel>) -> Vec<ProviderModel> {
    let mut models = provider.models.clone();
    for found in discovered.into_iter().take(500) {
        if let Some(existing) = models.iter_mut().find(|model| model.id == found.id) {
            existing.name = found.name;
            if found.context_length.is_some() {
                existing.context_length = found.context_length;
            }
            existing.source = "api".into();
        } else {
            models.push(ProviderModel {
                id: found.id.clone(),
                name: found.name,
                context_length: found.context_length,
                capabilities: capability_baseline(provider, &found.id),
                source: "api".into(),
            });
        }
    }
    models
}

#[tauri::command]
pub async fn provider_test(
    state: State<'_, AppState>,
    provider_id: String,
) -> AppResult<ProviderTestResult> {
    let provider = {
        let conn = state.db();
        get_provider_row(&conn, &provider_id)?
    };
    let started = Instant::now();
    let result = run_test(&state, &provider).await;
    let latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(discovered) => {
            let models = merge_models(&provider, discovered);
            {
                let conn = state.db();
                conn.execute(
                    "UPDATE providers SET models_json = ?2, last_status = 'ok', last_error = NULL,
                     last_tested_at = ?3, updated_at = ?3 WHERE id = ?1",
                    params![
                        provider_id,
                        serde_json::to_string(&models)?,
                        now()
                    ],
                )?;
            }
            Ok(ProviderTestResult {
                ok: true,
                message: format!("连接成功，发现 {} 个模型", models.len()),
                latency_ms,
                models: Some(models),
            })
        }
        Err(err) => {
            let message = err.user_message();
            let safe: String = message.chars().take(500).collect();
            {
                let conn = state.db();
                conn.execute(
                    "UPDATE providers SET last_status = 'error', last_error = ?2,
                     last_tested_at = ?3, updated_at = ?3 WHERE id = ?1",
                    params![provider_id, redact(&safe), now()],
                )?;
            }
            state.log(
                "WARN",
                &format!("供应商测试失败 {}：{}", provider_id, redact(&safe)),
            );
            Ok(ProviderTestResult {
                ok: false,
                message: safe,
                latency_ms,
                models: None,
            })
        }
    }
}

#[tauri::command]
pub fn provider_set_enabled(
    state: State<'_, AppState>,
    provider_id: String,
    enabled: bool,
) -> AppResult<Provider> {
    {
        let conn = state.db();
        get_provider_row(&conn, &provider_id)?;
        conn.execute(
            "UPDATE providers SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
            params![provider_id, enabled as i64, now()],
        )?;
    }
    let conn = state.db();
    Ok(attach_key(get_provider_row(&conn, &provider_id)?))
}

#[tauri::command]
pub fn provider_set_capability(
    state: State<'_, AppState>,
    provider_id: String,
    model_id: String,
    capability: String,
    value: String,
) -> AppResult<Provider> {
    if !valid_state(&value) {
        return Err(AppError::msg("无效的能力状态"));
    }
    if !matches!(
        capability.as_str(),
        "chat"
            | "stream"
            | "tools"
            | "vision"
            | "imageGen"
            | "imageEdit"
            | "json"
            | "longContext"
            | "multiTool"
            | "reasoning"
    ) {
        return Err(AppError::msg("无效的能力字段"));
    }
    mark_capabilities(&state, &provider_id, &model_id, &[(&capability, &value)])?;
    let conn = state.db();
    Ok(attach_key(get_provider_row(&conn, &provider_id)?))
}

#[tauri::command]
pub fn provider_models_from_discovered(
    state: State<'_, AppState>,
    provider_id: String,
    models: Vec<ProviderModel>,
) -> AppResult<Provider> {
    for model in &models {
        validate_caps(&model.capabilities)?;
    }
    {
        let conn = state.db();
        get_provider_row(&conn, &provider_id)?;
        conn.execute(
            "UPDATE providers SET models_json = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                provider_id,
                serde_json::to_string(&models)?,
                now()
            ],
        )?;
    }
    let conn = state.db();
    Ok(attach_key(get_provider_row(&conn, &provider_id)?))
}
