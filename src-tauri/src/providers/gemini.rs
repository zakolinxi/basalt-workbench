use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::DiscoveredModel;

use super::http::{ensure_ok, merge_params, request};
use super::sse::{for_each_line, parse_data_json};
use super::{
    ChatAdapter, ChatMessage, ChatRequest, ProviderEvent, ProviderRuntime, ToolSchema,
};

pub struct GeminiAdapter;

fn headers(builder: reqwest::RequestBuilder, rt: &ProviderRuntime) -> AppResult<reqwest::RequestBuilder> {
    if !rt.has_key() {
        return Err(AppError::msg("Gemini 需要 API Key"));
    }
    Ok(builder.header(
        "x-goog-api-key",
        rt.api_key.as_deref().unwrap_or_default(),
    ))
}

fn model_path(model: &str) -> AppResult<String> {
    let model = model.trim().trim_start_matches("models/");
    if model.is_empty()
        || !model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(AppError::msg("模型 ID 包含无效字符"));
    }
    Ok(model.to_string())
}

fn sanitize_schema(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, val) in map {
                if matches!(
                    key.as_str(),
                    "$schema"
                        | "additionalProperties"
                        | "patternProperties"
                        | "$id"
                        | "title"
                        | "default"
                        | "examples"
                ) {
                    continue;
                }
                out.insert(key.clone(), sanitize_schema(val));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sanitize_schema).collect()),
        other => other.clone(),
    }
}

fn convert_messages(messages: &[ChatMessage]) -> (Option<Value>, Value) {
    let mut system_parts: Vec<String> = Vec::new();
    let mut contents: Vec<(String, Vec<Value>)> = Vec::new();
    for message in messages {
        if message.role == "system" {
            if !message.content.trim().is_empty() {
                system_parts.push(message.content.clone());
            }
            continue;
        }
        let (role, parts) = match message.role.as_str() {
            "assistant" => {
                let mut parts = Vec::new();
                if !message.content.is_empty() {
                    parts.push(json!({"text": message.content}));
                }
                for call in &message.tool_calls {
                    parts.push(json!({
                        "functionCall": {
                            "name": call.name,
                            "args": call.arguments
                        }
                    }));
                }
                ("model".to_string(), parts)
            }
            "tool" => {
                let response = serde_json::from_str::<Value>(&message.content)
                    .ok()
                    .filter(|v| v.is_object())
                    .unwrap_or_else(|| json!({"result": message.content}));
                (
                    "user".to_string(),
                    vec![json!({
                        "functionResponse": {
                            "name": message.tool_name.clone().unwrap_or_else(|| "tool".into()),
                            "response": response
                        }
                    })],
                )
            }
            _ => {
                let mut parts = Vec::new();
                if !message.content.is_empty() {
                    parts.push(json!({"text": message.content}));
                }
                for image in &message.images {
                    parts.push(json!({
                        "inlineData": {
                            "mimeType": image.mime,
                            "data": image.base64
                        }
                    }));
                }
                ("user".to_string(), parts)
            }
        };
        if let Some((last_role, last_parts)) = contents.last_mut() {
            if *last_role == role {
                last_parts.extend(parts);
                continue;
            }
        }
        contents.push((role, parts));
    }
    let contents: Vec<Value> = contents
        .into_iter()
        .map(|(role, parts)| json!({"role": role, "parts": parts}))
        .collect();
    let system = if system_parts.is_empty() {
        None
    } else {
        Some(json!({"parts": [{"text": system_parts.join("\n\n")}]}))
    };
    (system, Value::Array(contents))
}

fn tools_json(tools: &[ToolSchema]) -> Value {
    json!([{
        "functionDeclarations": tools
            .iter()
            .map(|tool| json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": sanitize_schema(&tool.parameters)
            }))
            .collect::<Vec<_>>()
    }])
}

struct PartState {
    tool_counter: usize,
}

fn emit_parts(
    parts: &[Value],
    emit: &UnboundedSender<ProviderEvent>,
    state: &mut PartState,
) {
    for part in parts {
        if part.get("thought").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }
        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                let _ = emit.send(ProviderEvent::TextDelta(text.to_string()));
            }
        }
        if let Some(call) = part.get("functionCall") {
            let name = call
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("tool")
                .to_string();
            let args = call
                .get("args")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let id = format!(
                "gemini-{}-{}",
                state.tool_counter,
                &Uuid::new_v4().to_string()[..8]
            );
            let _ = emit.send(ProviderEvent::ToolCallDelta {
                index: state.tool_counter,
                id: Some(id),
                name: Some(name),
                args_delta: args.to_string(),
            });
            state.tool_counter += 1;
        }
    }
}

#[async_trait]
impl ChatAdapter for GeminiAdapter {
    async fn list_models(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
    ) -> AppResult<Vec<DiscoveredModel>> {
        let url = rt.endpoint("models")?;
        let builder = headers(request(client, reqwest::Method::GET, &url, rt), rt)?;
        let response = ensure_ok(builder.send().await?).await?;
        let value: Value = response.json().await?;
        let mut models = Vec::new();
        if let Some(items) = value.get("models").and_then(|v| v.as_array()) {
            for item in items {
                let Some(full_name) = item.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                let id = full_name.trim_start_matches("models/").to_string();
                models.push(DiscoveredModel {
                    id: id.clone(),
                    name: item
                        .get("displayName")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&id)
                        .to_string(),
                    context_length: item.get("inputTokenLimit").and_then(|v| v.as_u64()),
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
        let model = model_path(&req.model)?;
        let stream = rt.provider.stream;
        let method = if stream {
            format!("models/{model}:streamGenerateContent?alt=sse")
        } else {
            format!("models/{model}:generateContent")
        };
        let url = rt.endpoint(&method)?;
        let (system, contents) = convert_messages(&req.messages);
        let mut generation_config = json!({});
        if let Some(max_tokens) = req.max_tokens {
            generation_config["maxOutputTokens"] = json!(max_tokens);
        }
        if let Some(temperature) = req.temperature {
            generation_config["temperature"] = json!(temperature);
        }
        if let Some(effort) = req.reasoning_effort.as_deref() {
            if model.starts_with("gemini-2.5") {
                let budget = match effort {
                    "off" => {
                        if model.contains("pro") {
                            128
                        } else {
                            0
                        }
                    }
                    "minimal" => 512,
                    "low" => 1_024,
                    "medium" => 8_192,
                    "high" => 24_576,
                    "xhigh" | "max" | "ultra" => 32_768,
                    _ => 0,
                };
                generation_config["thinkingConfig"] = json!({ "thinkingBudget": budget });
            } else {
                // Gemini 3/4 只能选择档位，不能真正关闭思考。
                let level = match effort {
                    "off" | "minimal" | "low" => "low",
                    "medium" => "medium",
                    "high" | "xhigh" | "max" | "ultra" => "high",
                    _ => "",
                };
                if !level.is_empty() {
                    generation_config["thinkingConfig"] = json!({ "thinkingLevel": level });
                }
            }
        }
        let mut body = json!({
            "contents": contents,
            "generationConfig": generation_config
        });
        if let Some(system) = system {
            body["systemInstruction"] = system;
        }
        if !req.tools.is_empty() {
            body["tools"] = tools_json(&req.tools);
        }
        let body = merge_params(body, rt);
        let builder = headers(request(client, reqwest::Method::POST, &url, rt), rt)?;
        let response = ensure_ok(builder.json(&body).send().await?).await?;
        let mut state = PartState { tool_counter: 0 };

        if !stream {
            let value: Value = response.json().await?;
            if let Some(error) = value.get("error") {
                return Err(AppError::Msg(format!("Gemini 返回错误：{error}")));
            }
            let candidate = value
                .get("candidates")
                .and_then(|v| v.as_array())
                .and_then(|v| v.first())
                .cloned()
                .unwrap_or_else(|| json!({}));
            if let Some(parts) = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|v| v.as_array())
            {
                emit_parts(parts, emit, &mut state);
            }
            if let Some(usage) = value.get("usageMetadata") {
                let _ = emit.send(ProviderEvent::Usage {
                    input_tokens: usage.get("promptTokenCount").and_then(|v| v.as_u64()),
                    output_tokens: usage.get("candidatesTokenCount").and_then(|v| v.as_u64()),
                });
            }
            let _ = emit.send(ProviderEvent::Done {
                stop_reason: candidate
                    .get("finishReason")
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
            if let Some(error) = value.get("error") {
                return Err(AppError::Msg(format!("Gemini 流式错误：{error}")));
            }
            if let Some(candidate) = value
                .get("candidates")
                .and_then(|v| v.as_array())
                .and_then(|v| v.first())
            {
                if let Some(parts) = candidate
                    .get("content")
                    .and_then(|c| c.get("parts"))
                    .and_then(|v| v.as_array())
                {
                    emit_parts(parts, emit, &mut state);
                }
                if let Some(reason) = candidate.get("finishReason").and_then(|v| v.as_str()) {
                    stop_reason = Some(reason.to_string());
                }
            }
            if let Some(usage) = value.get("usageMetadata") {
                let _ = emit.send(ProviderEvent::Usage {
                    input_tokens: usage.get("promptTokenCount").and_then(|v| v.as_u64()),
                    output_tokens: usage.get("candidatesTokenCount").and_then(|v| v.as_u64()),
                });
            }
            Ok(false)
        })
        .await;
        result?;
        let _ = emit.send(ProviderEvent::Done { stop_reason });
        Ok(())
    }
}
