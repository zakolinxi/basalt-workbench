use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;

use crate::error::AppError;
use crate::models::RecoverySettings;
use crate::providers::{
    chat_adapter_for, ChatRequest, ProviderEvent, ProviderRuntime, ProviderToolCall, UsageTotals,
};
use crate::state::AppState;

#[derive(Default)]
struct AccCall {
    id: Option<String>,
    name: Option<String>,
    args: String,
}

#[derive(Default)]
pub struct Round {
    pub text: String,
    pub calls: Vec<ProviderToolCall>,
    pub usage: UsageTotals,
    pub duration_ms: u64,
    pub attempts: u32,
}

impl Round {
    pub fn usage(&self) -> UsageTotals {
        self.usage
    }

    pub fn add_usage(&mut self, usage: UsageTotals) {
        self.usage.input_tokens += usage.input_tokens;
        self.usage.output_tokens += usage.output_tokens;
        self.usage.cached_tokens += usage.cached_tokens;
    }
}

pub struct RoundFailure {
    pub error: AppError,
    /// True when any text delta reached the UI. Retrying after that would
    /// duplicate tokens, so callers must not transparently resend.
    pub emitted: bool,
    pub usage: UsageTotals,
    pub duration_ms: u64,
}

impl From<RoundFailure> for AppError {
    fn from(value: RoundFailure) -> Self {
        value.error
    }
}

fn max_attempts(recovery: &RecoverySettings) -> u32 {
    1 + recovery.max_retries.clamp(0, 5)
}

fn backoff(recovery: &RecoverySettings, attempt: u32) -> Duration {
    let base = recovery.backoff_ms.clamp(100, 10_000);
    let factor = 1u64 << attempt.min(4);
    Duration::from_millis((base * factor).min(15_000))
}

fn calls_from_raw(calls: BTreeMap<usize, AccCall>) -> Vec<ProviderToolCall> {
    let mut out = Vec::new();
    for acc in calls.into_values() {
        let Some(name) = acc.name else { continue };
        let id = acc
            .id
            .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4()));
        let raw = acc.args.trim();
        let arguments: Value = if raw.is_empty() {
            json!({})
        } else {
            serde_json::from_str(raw).unwrap_or_else(|_| json!({"__raw": raw}))
        };
        out.push(ProviderToolCall {
            id,
            name,
            arguments,
        });
    }
    out
}

fn merge_usage(total: &mut UsageTotals, usage: UsageTotals) {
    total.input_tokens += usage.input_tokens;
    total.output_tokens += usage.output_tokens;
    total.cached_tokens += usage.cached_tokens;
}

/// Runs one model round with automatic retry for transient failures. Deltas are
/// forwarded live; a retry happens only when the failed attempt produced no
/// text, so the UI never receives duplicate content.
pub async fn run_round(
    state: &AppState,
    rt: &ProviderRuntime,
    model: &str,
    request: &ChatRequest,
    cancel: &CancellationToken,
    recovery: &RecoverySettings,
    emit_delta: &mut (dyn FnMut(&str) + Send),
    on_retry: &mut (dyn FnMut(u32, &AppError) + Send),
) -> Result<Round, RoundFailure> {
    let attempts = max_attempts(recovery);
    let started = Instant::now();
    let mut total_usage = UsageTotals::default();
    let mut attempt = 0u32;

    loop {
        attempt += 1;
        if cancel.is_cancelled() {
            return Err(RoundFailure {
                error: AppError::Cancelled,
                emitted: false,
                usage: total_usage,
                duration_ms: started.elapsed().as_millis() as u64,
            });
        }
        let adapter = match chat_adapter_for(state, rt, model) {
            Ok(adapter) => adapter,
            Err(error) => {
                return Err(RoundFailure {
                    error,
                    emitted: false,
                    usage: total_usage,
                    duration_ms: started.elapsed().as_millis() as u64,
                })
            }
        };
        let client = state.http.clone();
        let rt_for = rt.clone();
        let req_for = request.clone();
        let token = cancel.clone();
        let (tx, mut rx) = unbounded_channel::<ProviderEvent>();
        let handle = tauri::async_runtime::spawn(async move {
            adapter.chat(&client, &rt_for, &req_for, &tx, &token).await
        });

        let mut text = String::new();
        let mut emitted = false;
        let mut calls: BTreeMap<usize, AccCall> = BTreeMap::new();
        let mut usage = UsageTotals::default();
        while let Some(event) = rx.recv().await {
            match event {
                ProviderEvent::TextDelta(delta) => {
                    text.push_str(&delta);
                    if !delta.is_empty() {
                        emitted = true;
                        emit_delta(&delta);
                    }
                }
                ProviderEvent::ToolCallDelta {
                    index,
                    id,
                    name,
                    args_delta,
                } => {
                    let acc = calls.entry(index).or_default();
                    if let Some(id) = id {
                        acc.id = Some(id);
                    }
                    if let Some(name) = name {
                        acc.name = Some(name);
                    }
                    acc.args.push_str(&args_delta);
                }
                ProviderEvent::Usage {
                    input_tokens,
                    output_tokens,
                    cached_tokens,
                } => usage.merge(input_tokens, output_tokens, cached_tokens),
                ProviderEvent::Done { .. } => {}
            }
        }
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(AppError::msg(format!("模型任务执行异常：{error}"))),
        };
        merge_usage(&mut total_usage, usage);
        if let Err(error) = result {
            if error.is_cancelled() {
                return Err(RoundFailure {
                    error,
                    emitted,
                    usage: total_usage,
                    duration_ms: started.elapsed().as_millis() as u64,
                });
            }
            let can_retry = error.is_retryable() && !emitted && attempt < attempts;
            if can_retry {
                on_retry(attempt, &error);
                state.log(
                    "WARN",
                    &format!(
                        "模型调用失败，第 {attempt}/{attempts} 次尝试后重试：{}",
                        error.user_message()
                    ),
                );
                tokio::select! {
                    _ = cancel.cancelled() => {
                        return Err(RoundFailure {
                            error: AppError::Cancelled,
                            emitted: false,
                            usage: total_usage,
                            duration_ms: started.elapsed().as_millis() as u64,
                        });
                    }
                    _ = tokio::time::sleep(backoff(recovery, attempt)) => {}
                }
                continue;
            }
            return Err(RoundFailure {
                error,
                emitted,
                usage: total_usage,
                duration_ms: started.elapsed().as_millis() as u64,
            });
        }
        if cancel.is_cancelled() {
            return Err(RoundFailure {
                error: AppError::Cancelled,
                emitted,
                usage: total_usage,
                duration_ms: started.elapsed().as_millis() as u64,
            });
        }
        return Ok(Round {
            calls: calls_from_raw(calls),
            text,
            usage: total_usage,
            duration_ms: started.elapsed().as_millis() as u64,
            attempts: attempt,
        });
    }
}

pub fn record_usage(
    state: &AppState,
    session_id: &str,
    task_id: Option<&str>,
    provider_id: &str,
    model: &str,
    duration_ms: u64,
    usage: UsageTotals,
) {
    if usage.is_empty() {
        return;
    }
    let conn = state.db();
    let _ = crate::db::record_run_metric(
        &conn,
        session_id,
        task_id,
        provider_id,
        model,
        duration_ms,
        usage.input_tokens,
        usage.cached_tokens,
        usage.output_tokens,
    );
}

pub fn fallback_route(
    state: &AppState,
    primary_provider: &str,
    primary_model: &str,
) -> Option<(ProviderRuntime, String)> {
    let recovery = {
        let conn = state.db();
        crate::db::load_settings(&conn).recovery
    };
    if recovery.fallback_provider_id.trim().is_empty() {
        return None;
    }
    if recovery.fallback_provider_id == primary_provider
        && (recovery.fallback_model.trim().is_empty() || recovery.fallback_model == primary_model)
    {
        return None;
    }
    let rt = crate::providers::load_chat_runtime(state, &recovery.fallback_provider_id).ok()?;
    let model = if recovery.fallback_model.trim().is_empty() {
        rt.provider.default_chat_model.clone()?
    } else {
        recovery.fallback_model.trim().to_string()
    };
    if model == primary_model && rt.provider.id == primary_provider {
        return None;
    }
    Some((rt, model))
}

#[allow(dead_code)]
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[allow(dead_code)]
pub fn is_retryable(error: &AppError) -> bool {
    error.is_retryable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_are_bounded_and_backoff_grows() {
        let recovery = RecoverySettings {
            max_retries: 9,
            backoff_ms: 500,
            fallback_provider_id: String::new(),
            fallback_model: String::new(),
        };
        assert_eq!(max_attempts(&recovery), 6);
        assert!(backoff(&recovery, 2) > backoff(&recovery, 1));
    }
}
