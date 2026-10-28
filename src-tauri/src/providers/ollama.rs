use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::DiscoveredModel;

use super::http::{ensure_ok, join_url, merge_params, request};
use super::sse::for_each_line;
use super::{ChatAdapter, ChatRequest, ProviderEvent, ProviderRuntime};

pub struct OllamaAdapter;

fn endpoint(rt: &ProviderRuntime, suffix: &str) -> AppResult<String> {
    let base = rt.provider.base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(AppError::msg("未配置 Ollama Base URL"));
    }
    let base = if base.ends_with("/api") {
        base.to_string()
    } else {
        format!("{base}/api")
    };
    join_url(&base, suffix)
}

fn messages_json(messages: &[super::ChatMessage]) -> Value {
    let mut out = Vec::new();
    for message in messages {
        match message.role.as_str() {
            "assistant" => {
                let mut obj = json!({"role": "assistant", "content": message.content});
                if !message.tool_calls.is_empty() {
                    obj["tool_calls"] = Value::Array(
                        message
                            .tool_calls
                            .iter()
                            .map(|call| {
                                json!({
                                    "function": {
                                        "name": call.name,
                                        "arguments": call.arguments
                                    }
                                })
                            })
                            .collect(),
                    );
                }
                out.push(obj);
            }
            "tool" => {
                out.push(json!({
                    "role": "tool",
                    "content": message.content,
                    "tool_name": message.tool_name
                }));
            }
            role => {
                let mut obj = json!({"role": role, "content": message.content});
                if !message.images.is_empty() {
                    obj["images"] = Value::Array(
                        message
                            .images
                            .iter()
                            .map(|image| Value::String(image.base64.clone()))
                            .collect(),
                    );
                }
                out.push(obj);
            }
        }
    }
    Value::Array(out)
}

fn tools_json(tools: &[super::ToolSchema]) -> Value {
    Value::Array(
        tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters
                    }
                })
            })
            .collect(),
    )
}

#[async_trait]
impl ChatAdapter for OllamaAdapter {
    async fn list_models(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
    ) -> AppResult<Vec<DiscoveredModel>> {
        let url = endpoint(rt, "tags")?;
        let response = ensure_ok(request(client, reqwest::Method::GET, &url, rt).send().await?).await?;
        let value: Value = response.json().await?;
        let mut models = Vec::new();
        if let Some(items) = value.get("models").and_then(|v| v.as_array()) {
            for item in items {
                let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                models.push(DiscoveredModel {
                    id: name.to_string(),
                    name: name.to_string(),
                    context_length: None,
                });
            }
        }
        Ok(models)
    }

    async fn chat(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
        req: &ChatRequest,
        emit: &UnboundedSender<ProviderEvent>,
        cancel: &CancellationToken,
    ) -> AppResult<()> {
        let url = endpoint(rt, "chat")?;
        let stream = rt.provider.stream;
        let mut body = json!({
            "model": req.model,
            "messages": messages_json(&req.messages),
            "stream": stream
        });
        if !req.tools.is_empty() {
            body["tools"] = tools_json(&req.tools);
        }
        if let Some(effort) = req.reasoning_effort.as_deref() {
            body["think"] = match effort {
                "off" => json!(false),
                "minimal" | "low" => json!("low"),
                "medium" => json!("medium"),
                "high" => json!("high"),
                "xhigh" | "max" | "ultra" => json!("max"),
                _ => json!(true),
            };
        }
        let body = merge_params(body, rt);
        let builder = request(client, reqwest::Method::POST, &url, rt).json(&body);
        let response = ensure_ok(builder.send().await?).await?;

        if !stream {
            let value: Value = response.json().await?;
            if let Some(error) = value.get("error") {
                return Err(AppError::Msg(format!("Ollama 返回错误：{error}")));
            }
            if let Some(message) = value.get("message") {
                if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        let _ = emit.send(ProviderEvent::TextDelta(text.to_string()));
                    }
                }
                emit_tool_calls(message, emit);
            }
            emit_usage(&value, emit);
            let _ = emit.send(ProviderEvent::Done {
                stop_reason: value
                    .get("done_reason")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
            return Ok(());
        }

        let mut stop_reason: Option<String> = None;
        let result = for_each_line(response, cancel, |line| {
            if line.trim().is_empty() {
                return Ok(false);
            }
            let value: Value = match serde_json::from_str(line) {
                Ok(value) => value,
                Err(_) => return Ok(false),
            };
            if let Some(error) = value.get("error") {
                return Err(AppError::Msg(format!("Ollama 流式错误：{error}")));
            }
            if let Some(message) = value.get("message") {
                if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        let _ = emit.send(ProviderEvent::TextDelta(text.to_string()));
                    }
                }
                emit_tool_calls(message, emit);
            }
            emit_usage(&value, emit);
            if let Some(reason) = value.get("done_reason").and_then(|v| v.as_str()) {
                stop_reason = Some(reason.to_string());
            }
            if value.get("done").and_then(|v| v.as_bool()) == Some(true) {
                return Ok(true);
            }
            Ok(false)
        })
        .await;
        result?;
        let _ = emit.send(ProviderEvent::Done { stop_reason });
        Ok(())
    }
}

fn emit_tool_calls(message: &Value, emit: &UnboundedSender<ProviderEvent>) {
    if let Some(calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        for (index, call) in calls.iter().enumerate() {
            let function = call.get("function").cloned().unwrap_or_else(|| json!({}));
            let name = function
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("tool")
                .to_string();
            let arguments = function.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let args = if let Some(text) = arguments.as_str() {
                text.to_string()
            } else {
                arguments.to_string()
            };
            let id = call
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("ollama-{}-{}", index, &Uuid::new_v4().to_string()[..8]));
            let _ = emit.send(ProviderEvent::ToolCallDelta {
                index,
                id: Some(id),
                name: Some(name),
                args_delta: args,
            });
        }
    }
}

fn emit_usage(value: &Value, emit: &UnboundedSender<ProviderEvent>) {
    if value.get("done").and_then(|v| v.as_bool()) == Some(true) {
        let _ = emit.send(ProviderEvent::Usage {
            input_tokens: value.get("prompt_eval_count").and_then(|v| v.as_u64()),
            output_tokens: value.get("eval_count").and_then(|v| v.as_u64()),
        });
    }
}
