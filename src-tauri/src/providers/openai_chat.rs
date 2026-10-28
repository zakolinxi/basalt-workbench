use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::DiscoveredModel;

use super::http::{ensure_ok, merge_params, request, with_bearer};
use super::sse::{for_each_line, parse_data_json};
use super::{
    ChatAdapter, ChatMessage, ChatRequest, ProviderEvent, ProviderRuntime, ToolSchema,
};

pub struct OpenAiChatAdapter;

pub fn messages_to_json(messages: &[ChatMessage]) -> Value {
    let mut out = Vec::new();
    for message in messages {
        match message.role.as_str() {
            "assistant" => {
                let mut obj = json!({"role": "assistant"});
                if message.content.is_empty() {
                    obj["content"] = Value::Null;
                } else {
                    obj["content"] = Value::String(message.content.clone());
                }
                if !message.tool_calls.is_empty() {
                    let calls: Vec<Value> = message
                        .tool_calls
                        .iter()
                        .map(|call| {
                            json!({
                                "id": call.id,
                                "type": "function",
                                "function": {
                                    "name": call.name,
                                    "arguments": call.arguments.to_string()
                                }
                            })
                        })
                        .collect();
                    obj["tool_calls"] = Value::Array(calls);
                }
                out.push(obj);
            }
            "tool" => {
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": message.tool_call_id,
                    "content": message.content
                }));
            }
            role => {
                if message.images.is_empty() {
                    out.push(json!({"role": role, "content": message.content}));
                } else {
                    let mut parts = Vec::new();
                    if !message.content.is_empty() {
                        parts.push(json!({"type": "text", "text": message.content}));
                    }
                    for image in &message.images {
                        parts.push(json!({
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:{};base64,{}", image.mime, image.base64)
                            }
                        }));
                    }
                    out.push(json!({"role": role, "content": parts}));
                }
            }
        }
    }
    Value::Array(out)
}

pub fn tools_to_json(tools: &[ToolSchema]) -> Value {
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
impl ChatAdapter for OpenAiChatAdapter {
    async fn list_models(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
    ) -> AppResult<Vec<DiscoveredModel>> {
        let url = rt.endpoint("models")?;
        let builder = with_bearer(request(client, reqwest::Method::GET, &url, rt), rt);
        let response = ensure_ok(builder.send().await?).await?;
        let value: Value = response.json().await?;
        let items = value
            .get("data")
            .and_then(|v| v.as_array())
            .or_else(|| value.as_array())
            .cloned()
            .unwrap_or_default();
        let mut models = Vec::new();
        for item in items {
            let Some(id) = item.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(id)
                .to_string();
            let context_length = item
                .get("context_length")
                .or_else(|| item.get("context_window"))
                .or_else(|| item.get("max_context_length"))
                .and_then(|v| v.as_u64());
            models.push(DiscoveredModel {
                id: id.to_string(),
                name,
                context_length,
            });
        }
        models.sort_by(|a, b| a.id.cmp(&b.id));
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
        let url = rt.endpoint("chat/completions")?;
        let stream = rt.provider.stream;
        let mut body = json!({
            "model": req.model,
            "messages": messages_to_json(&req.messages),
            "stream": stream
        });
        if !req.tools.is_empty() {
            body["tools"] = tools_to_json(&req.tools);
            body["tool_choice"] = json!("auto");
        }
        if let Some(max_tokens) = req.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temperature) = req.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(effort) = req.reasoning_effort.as_deref() {
            let lower = req.model.to_ascii_lowercase();
            if lower.starts_with("deepseek") {
                match effort {
                    "off" | "minimal" => body["thinking"] = json!({ "type": "disabled" }),
                    "low" => body["reasoning_effort"] = json!("low"),
                    "medium" | "high" => body["reasoning_effort"] = json!("high"),
                    "xhigh" | "max" | "ultra" => body["reasoning_effort"] = json!("max"),
                    _ => {}
                }
            } else if lower.contains("minimax") {
                // MiniMax 新模型不支持关闭思考，低档代替。
                let mapped = match effort {
                    "off" | "minimal" | "low" => "low",
                    "medium" => "medium",
                    "high" => "high",
                    "xhigh" => "xhigh",
                    "max" | "ultra" => "max",
                    _ => "",
                };
                if !mapped.is_empty() {
                    body["reasoning_effort"] = json!(mapped);
                }
            } else if lower.contains("kimi") || lower.contains("glm") {
                // 这两类模型没有 none/minimal/medium 档，中档取 high。
                let mapped = match effort {
                    "off" | "minimal" | "low" => "low",
                    "medium" | "high" => "high",
                    "xhigh" | "max" | "ultra" => "max",
                    _ => "",
                };
                if !mapped.is_empty() {
                    body["reasoning_effort"] = json!(mapped);
                }
            } else if lower.contains("qwen") {
                let mapped = match effort {
                    "off" => "none",
                    "minimal" | "low" => "low",
                    "medium" => "medium",
                    "high" | "xhigh" | "max" | "ultra" => "xhigh",
                    _ => "",
                };
                if !mapped.is_empty() {
                    body["reasoning_effort"] = json!(mapped);
                }
            } else if lower.contains("grok") {
                let mapped = match effort {
                    "off" | "minimal" | "low" => "low",
                    "medium" => "medium",
                    "high" => "high",
                    "xhigh" | "max" | "ultra" => "xhigh",
                    _ => "",
                };
                if !mapped.is_empty() {
                    body["reasoning_effort"] = json!(mapped);
                }
            } else {
                let gpt6 = lower.starts_with("gpt-6");
                let mapped = match effort {
                    "off" => {
                        if gpt6 {
                            "low"
                        } else {
                            "none"
                        }
                    }
                    "minimal" => {
                        if gpt6 {
                            "low"
                        } else {
                            "minimal"
                        }
                    }
                    "low" => "low",
                    "medium" => "medium",
                    "high" => "high",
                    "xhigh" => "xhigh",
                    "max" => "max",
                    "ultra" => "max",
                    _ => "",
                };
                if !mapped.is_empty() {
                    body["reasoning_effort"] = json!(mapped);
                }
            }
        }
        let body = merge_params(body, rt);
        let builder = with_bearer(request(client, reqwest::Method::POST, &url, rt), rt)
            .json(&body);
        let response = ensure_ok(builder.send().await?).await?;

        if !stream {
            let value: Value = response.json().await?;
            if let Some(error) = value.get("error") {
                return Err(AppError::Msg(format!("供应商返回错误：{error}")));
            }
            let choice = value
                .get("choices")
                .and_then(|v| v.as_array())
                .and_then(|v| v.first())
                .cloned()
                .unwrap_or_else(|| json!({}));
            let message = choice.get("message").cloned().unwrap_or_else(|| json!({}));
            if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    let _ = emit.send(ProviderEvent::TextDelta(text.to_string()));
                }
            }
            if let Some(calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
                for (index, call) in calls.iter().enumerate() {
                    let function = call.get("function").cloned().unwrap_or_else(|| json!({}));
                    let name = function
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let arguments = function
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}")
                        .to_string();
                    let _ = emit.send(ProviderEvent::ToolCallDelta {
                        index,
                        id: call.get("id").and_then(|v| v.as_str()).map(str::to_string),
                        name: if name.is_empty() { None } else { Some(name) },
                        args_delta: arguments,
                    });
                }
            }
            if let Some(usage) = value.get("usage") {
                let _ = emit.send(ProviderEvent::Usage {
                    input_tokens: usage.get("prompt_tokens").and_then(|v| v.as_u64()),
                    output_tokens: usage.get("completion_tokens").and_then(|v| v.as_u64()),
                });
            }
            let _ = emit.send(ProviderEvent::Done {
                stop_reason: choice
                    .get("finish_reason")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
            return Ok(());
        }

        let mut finished = false;
        let mut stop_reason: Option<String> = None;
        let result = for_each_line(response, cancel, |line| {
            let Some(value) = parse_data_json(line) else {
                return Ok(false);
            };
            if let Some(error) = value.get("error") {
                return Err(AppError::Msg(format!("供应商流式错误：{error}")));
            }
            if let Some(choices) = value.get("choices").and_then(|v| v.as_array()) {
                if let Some(choice) = choices.first() {
                    if let Some(delta) = choice.get("delta") {
                        if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
                            if !text.is_empty() {
                                let _ = emit.send(ProviderEvent::TextDelta(text.to_string()));
                            }
                        }
                        if let Some(calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                            for call in calls {
                                let index = call
                                    .get("index")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as usize;
                                let function =
                                    call.get("function").cloned().unwrap_or_else(|| json!({}));
                                let _ = emit.send(ProviderEvent::ToolCallDelta {
                                    index,
                                    id: call
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .map(str::to_string),
                                    name: function
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .map(str::to_string),
                                    args_delta: function
                                        .get("arguments")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or_default()
                                        .to_string(),
                                });
                            }
                        }
                    }
                    if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                        stop_reason = Some(reason.to_string());
                    }
                }
            }
            if let Some(usage) = value.get("usage") {
                if !usage.is_null() {
                    let _ = emit.send(ProviderEvent::Usage {
                        input_tokens: usage.get("prompt_tokens").and_then(|v| v.as_u64()),
                        output_tokens: usage.get("completion_tokens").and_then(|v| v.as_u64()),
                    });
                }
            }
            if line.contains("[DONE]") {
                finished = true;
                return Ok(true);
            }
            Ok(false)
        })
        .await;
        let _ = finished;
        result?;
        let _ = emit.send(ProviderEvent::Done { stop_reason });
        Ok(())
    }
}
