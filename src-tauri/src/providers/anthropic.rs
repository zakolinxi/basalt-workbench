use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::DiscoveredModel;

use super::http::{ensure_ok, merge_params, request};
use super::sse::{for_each_line, parse_data_json};
use super::{ChatAdapter, ChatRequest, ProviderEvent, ProviderRuntime};

pub struct AnthropicAdapter;

const ANTHROPIC_VERSION: &str = "2023-06-01";

fn headers(builder: reqwest::RequestBuilder, rt: &ProviderRuntime) -> reqwest::RequestBuilder {
    let builder = builder.header("anthropic-version", ANTHROPIC_VERSION);
    if let Some(key) = rt.api_key.as_deref() {
        builder.header("x-api-key", key)
    } else {
        builder
    }
}

fn convert_messages(messages: &[super::ChatMessage]) -> (Option<String>, Value) {
    let mut system_parts: Vec<String> = Vec::new();
    let mut converted: Vec<(String, Vec<Value>)> = Vec::new();
    for message in messages {
        if message.role == "system" {
            if !message.content.trim().is_empty() {
                system_parts.push(message.content.clone());
            }
            continue;
        }
        let (role, blocks) = match message.role.as_str() {
            "tool" => {
                let content = message.content.clone();
                (
                    "user".to_string(),
                    vec![json!({
                        "type": "tool_result",
                        "tool_use_id": message.tool_call_id,
                        "content": content
                    })],
                )
            }
            "assistant" => {
                let mut blocks = Vec::new();
                if !message.content.is_empty() {
                    blocks.push(json!({"type": "text", "text": message.content}));
                }
                for call in &message.tool_calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call.id,
                        "name": call.name,
                        "input": call.arguments
                    }));
                }
                ("assistant".to_string(), blocks)
            }
            _ => {
                let mut blocks = Vec::new();
                for image in &message.images {
                    blocks.push(json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": image.mime,
                            "data": image.base64
                        }
                    }));
                }
                if !message.content.is_empty() {
                    blocks.push(json!({"type": "text", "text": message.content}));
                }
                ("user".to_string(), blocks)
            }
        };
        if let Some((last_role, last_blocks)) = converted.last_mut() {
            if *last_role == role {
                last_blocks.extend(blocks);
                continue;
            }
        }
        converted.push((role, blocks));
    }
    let messages: Vec<Value> = converted
        .into_iter()
        .map(|(role, content)| json!({"role": role, "content": content}))
        .collect();
    let system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };
    (system, Value::Array(messages))
}

fn tools_json(tools: &[super::ToolSchema]) -> Value {
    Value::Array(
        tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.parameters
                })
            })
            .collect(),
    )
}

#[async_trait]
impl ChatAdapter for AnthropicAdapter {
    async fn list_models(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
    ) -> AppResult<Vec<DiscoveredModel>> {
        let url = rt.endpoint("models")?;
        let builder = headers(request(client, reqwest::Method::GET, &url, rt), rt);
        let response = ensure_ok(builder.send().await?).await?;
        let value: Value = response.json().await?;
        let mut models = Vec::new();
        if let Some(items) = value.get("data").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    models.push(DiscoveredModel {
                        id: id.to_string(),
                        name: item
                            .get("display_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(id)
                            .to_string(),
                        context_length: item
                            .get("context_window")
                            .and_then(|v| v.as_u64()),
                    });
                }
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
        if !rt.has_key() {
            return Err(AppError::msg("Anthropic 需要 API Key"));
        }
        let url = rt.endpoint("messages")?;
        let stream = rt.provider.stream;
        let (system, messages) = convert_messages(&req.messages);
        let mut body = json!({
            "model": req.model,
            "max_tokens": req.max_tokens.unwrap_or(4096),
            "messages": messages,
            "stream": stream
        });
        if let Some(system) = system {
            body["system"] = Value::String(system);
        }
        if !req.tools.is_empty() {
            body["tools"] = tools_json(&req.tools);
        }
        if let Some(temperature) = req.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(effort) = req.reasoning_effort.as_deref() {
            if req.model.contains("-4-5") {
                let budget = match effort {
                    "minimal" | "low" => Some(1_024u32),
                    "medium" => Some(4_096),
                    "high" => Some(16_384),
                    "xhigh" => Some(24_576),
                    "max" | "ultra" => Some(32_768),
                    _ => None,
                };
                if let Some(budget) = budget {
                    body["thinking"] =
                        json!({ "type": "enabled", "budget_tokens": budget });
                    let max_tokens = req.max_tokens.unwrap_or(4_096).max(budget + 1_024);
                    body["max_tokens"] = json!(max_tokens);
                    if let Some(obj) = body.as_object_mut() {
                        obj.remove("temperature");
                    }
                }
            } else {
                // 5.x 起自适应思考常开，用最低 effort 代替“关闭”。
                let mapped = match effort {
                    "off" | "minimal" | "low" => "low",
                    "medium" => "medium",
                    "high" => "high",
                    "xhigh" => "xhigh",
                    "max" => "max",
                    "ultra" => "max",
                    _ => "",
                };
                if !mapped.is_empty() {
                    body["output_config"] = json!({ "effort": mapped });
                    let base = req.max_tokens.unwrap_or(match mapped {
                        "medium" => 8_192,
                        "high" => 16_384,
                        "xhigh" | "max" => 32_768,
                        _ => 4_096,
                    });
                    let max_tokens = if req.max_tokens.is_none() && effort == "ultra" {
                        base + 32_768
                    } else {
                        base
                    };
                    body["max_tokens"] = json!(max_tokens);
                }
            }
        }
        let body = merge_params(body, rt);
        let builder = headers(request(client, reqwest::Method::POST, &url, rt), rt).json(&body);
        let response = ensure_ok(builder.send().await?).await?;

        if !stream {
            let value: Value = response.json().await?;
            if let Some(error) = value.get("error") {
                return Err(AppError::Msg(format!("供应商返回错误：{error}")));
            }
            if let Some(blocks) = value.get("content").and_then(|v| v.as_array()) {
                let mut tool_index = 0usize;
                for block in blocks {
                    match block.get("type").and_then(|v| v.as_str()) {
                        Some("text") => {
                            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                                let _ = emit.send(ProviderEvent::TextDelta(text.to_string()));
                            }
                        }
                        Some("tool_use") => {
                            let _ = emit.send(ProviderEvent::ToolCallDelta {
                                index: tool_index,
                                id: block.get("id").and_then(|v| v.as_str()).map(str::to_string),
                                name: block
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                args_delta: block
                                    .get("input")
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "{}".into()),
                            });
                            tool_index += 1;
                        }
                        _ => {}
                    }
                }
            }
            if let Some(usage) = value.get("usage") {
                let _ = emit.send(ProviderEvent::Usage {
                    input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()),
                    output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()),
                });
            }
            let _ = emit.send(ProviderEvent::Done {
                stop_reason: value
                    .get("stop_reason")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
            return Ok(());
        }

        let mut stop_reason: Option<String> = None;
        let result = for_each_line(response, cancel, |line| {
            let Some(value) = parse_data_json(line) else {
                return Ok(false);
            };
            let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            match event_type {
                "message_start" => {
                    if let Some(usage) = value
                        .get("message")
                        .and_then(|m| m.get("usage"))
                    {
                        let _ = emit.send(ProviderEvent::Usage {
                            input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()),
                            output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()),
                        });
                    }
                }
                "content_block_start" => {
                    if let Some(block) = value.get("content_block") {
                        if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            let index =
                                value.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                            let _ = emit.send(ProviderEvent::ToolCallDelta {
                                index,
                                id: block.get("id").and_then(|v| v.as_str()).map(str::to_string),
                                name: block
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                args_delta: String::new(),
                            });
                        }
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = value.get("delta") {
                        let index =
                            value.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        match delta.get("type").and_then(|v| v.as_str()) {
                            Some("text_delta") => {
                                if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                    let _ = emit.send(ProviderEvent::TextDelta(text.to_string()));
                                }
                            }
                            Some("input_json_delta") => {
                                if let Some(partial) =
                                    delta.get("partial_json").and_then(|v| v.as_str())
                                {
                                    let _ = emit.send(ProviderEvent::ToolCallDelta {
                                        index,
                                        id: None,
                                        name: None,
                                        args_delta: partial.to_string(),
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "message_delta" => {
                    if let Some(delta) = value.get("delta") {
                        stop_reason = delta
                            .get("stop_reason")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                    }
                    if let Some(usage) = value.get("usage") {
                        let _ = emit.send(ProviderEvent::Usage {
                            input_tokens: None,
                            output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()),
                        });
                    }
                }
                "message_stop" => return Ok(true),
                "error" => {
                    return Err(AppError::Msg(format!(
                        "Anthropic 流式错误：{}",
                        value.get("error").unwrap_or(&value)
                    )));
                }
                _ => {}
            }
            Ok(false)
        })
        .await;
        result?;
        let _ = emit.send(ProviderEvent::Done { stop_reason });
        Ok(())
    }
}
