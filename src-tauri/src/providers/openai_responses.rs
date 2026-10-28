use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::DiscoveredModel;

use super::http::{ensure_ok, merge_params, request, with_bearer};
use super::sse::{for_each_line, parse_data_json};
use super::{ChatAdapter, ChatRequest, ProviderEvent, ProviderRuntime};

pub struct OpenAiResponsesAdapter;

fn input_items(messages: &[super::ChatMessage]) -> Value {
    let mut out = Vec::new();
    for message in messages {
        match message.role.as_str() {
            "tool" => {
                out.push(json!({
                    "type": "function_call_output",
                    "call_id": message.tool_call_id,
                    "output": message.content
                }));
            }
            "assistant" => {
                if !message.content.is_empty() {
                    out.push(json!({
                        "role": "assistant",
                        "content": [{"type": "input_text", "text": message.content}]
                    }));
                }
                for call in &message.tool_calls {
                    out.push(json!({
                        "type": "function_call",
                        "call_id": call.id,
                        "name": call.name,
                        "arguments": call.arguments.to_string()
                    }));
                }
            }
            role => {
                let mut content = Vec::new();
                if !message.content.is_empty() {
                    content.push(json!({"type": "input_text", "text": message.content}));
                }
                for image in &message.images {
                    content.push(json!({
                        "type": "input_image",
                        "image_url": format!("data:{};base64,{}", image.mime, image.base64)
                    }));
                }
                out.push(json!({"role": role, "content": content}));
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
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters
                })
            })
            .collect(),
    )
}

#[async_trait]
impl ChatAdapter for OpenAiResponsesAdapter {
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
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                models.push(DiscoveredModel {
                    id: id.to_string(),
                    name: item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(id)
                        .to_string(),
                    context_length: item.get("context_length").and_then(|v| v.as_u64()),
                });
            }
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
        let url = rt.endpoint("responses")?;
        let stream = rt.provider.stream;
        let mut body = json!({
            "model": req.model,
            "input": input_items(&req.messages),
            "stream": stream,
            "store": false
        });
        if !req.tools.is_empty() {
            body["tools"] = tools_json(&req.tools);
            body["tool_choice"] = json!("auto");
        }
        if let Some(max_tokens) = req.max_tokens {
            body["max_output_tokens"] = json!(max_tokens);
        }
        if let Some(temperature) = req.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(effort) = req.reasoning_effort.as_deref() {
            let lower = req.model.to_ascii_lowercase();
            let gpt6 = lower.starts_with("gpt-6");
            let mapped = if lower.contains("minimax") {
                match effort {
                    "off" | "minimal" | "low" => "low",
                    "medium" => "medium",
                    "high" => "high",
                    "xhigh" => "xhigh",
                    "max" | "ultra" => "max",
                    _ => "",
                }
            } else {
                match effort {
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
                }
            };
            if !mapped.is_empty() {
                body["reasoning"] = json!({ "effort": mapped });
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
            let empty = Vec::new();
            let output = value
                .get("output")
                .and_then(|v| v.as_array())
                .unwrap_or(&empty);
            let mut tool_index = 0usize;
            for item in output {
                match item.get("type").and_then(|v| v.as_str()) {
                    Some("message") => {
                        if let Some(content) = item.get("content").and_then(|v| v.as_array()) {
                            for part in content {
                                if part.get("type").and_then(|v| v.as_str()) == Some("output_text")
                                {
                                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                        let _ = emit.send(ProviderEvent::TextDelta(text.to_string()));
                                    }
                                }
                            }
                        }
                    }
                    Some("function_call") => {
                        let _ = emit.send(ProviderEvent::ToolCallDelta {
                            index: tool_index,
                            id: item
                                .get("call_id")
                                .and_then(|v| v.as_str())
                                .map(str::to_string),
                            name: item
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(str::to_string),
                            args_delta: item
                                .get("arguments")
                                .and_then(|v| v.as_str())
                                .unwrap_or("{}")
                                .to_string(),
                        });
                        tool_index += 1;
                    }
                    _ => {}
                }
            }
            if let Some(usage) = value.get("usage") {
                let _ = emit.send(ProviderEvent::Usage {
                    input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()),
                    output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()),
                });
            }
            let status = value
                .get("status")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let _ = emit.send(ProviderEvent::Done { stop_reason: status });
            return Ok(());
        }

        let mut stop_reason: Option<String> = None;
        let result = for_each_line(response, cancel, |line| {
            let Some(value) = parse_data_json(line) else {
                return Ok(false);
            };
            let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            match event_type {
                "response.output_text.delta" => {
                    if let Some(delta) = value.get("delta").and_then(|v| v.as_str()) {
                        if !delta.is_empty() {
                            let _ = emit.send(ProviderEvent::TextDelta(delta.to_string()));
                        }
                    }
                }
                "response.output_item.added" => {
                    if let Some(item) = value.get("item") {
                        if item.get("type").and_then(|v| v.as_str()) == Some("function_call") {
                            let index =
                                value.get("output_index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                            let _ = emit.send(ProviderEvent::ToolCallDelta {
                                index,
                                id: item
                                    .get("call_id")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                name: item
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                args_delta: String::new(),
                            });
                        }
                    }
                }
                "response.function_call_arguments.delta" => {
                    let index =
                        value.get("output_index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    if let Some(delta) = value.get("delta").and_then(|v| v.as_str()) {
                        let _ = emit.send(ProviderEvent::ToolCallDelta {
                            index,
                            id: None,
                            name: None,
                            args_delta: delta.to_string(),
                        });
                    }
                }
                "response.completed" => {
                    if let Some(response) = value.get("response") {
                        if let Some(usage) = response.get("usage") {
                            let _ = emit.send(ProviderEvent::Usage {
                                input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()),
                                output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()),
                            });
                        }
                        stop_reason = response
                            .get("status")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                    }
                    return Ok(true);
                }
                "response.incomplete" | "response.failed" => {
                    let reason = value
                        .get("response")
                        .and_then(|r| r.get("incomplete_details"))
                        .and_then(|d| d.get("reason"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(event_type);
                    stop_reason = Some(reason.to_string());
                    if event_type == "response.failed" {
                        return Err(AppError::Msg(format!("Responses 执行失败：{reason}")));
                    }
                    return Ok(true);
                }
                "error" => {
                    return Err(AppError::Msg(format!(
                        "供应商流式错误：{}",
                        value.get("message").unwrap_or(&value)
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
