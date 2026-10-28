pub mod anthropic;
pub mod gemini;
pub mod http;
pub mod images;
pub mod ollama;
pub mod openai_chat;
pub mod openai_responses;
pub mod sse;

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::{DiscoveredModel, ImageCapabilities, Provider};
use crate::secrets;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatImage {
    pub mime: String,
    pub base64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub images: Vec<ChatImage>,
    #[serde(default)]
    pub tool_calls: Vec<ProviderToolCall>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
}

impl ChatMessage {
    pub fn text(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            images: Vec::new(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Clone, Debug, Default)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSchema>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub reasoning_effort: Option<String>,
}

#[derive(Clone, Debug)]
pub enum ProviderEvent {
    TextDelta(String),
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        args_delta: String,
    },
    Usage {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    Done {
        stop_reason: Option<String>,
    },
}

#[derive(Clone)]
pub struct ProviderRuntime {
    pub provider: Provider,
    pub api_key: Option<String>,
    pub user_agent: String,
}

impl ProviderRuntime {
    pub fn endpoint(&self, suffix: &str) -> AppResult<String> {
        http::join_url(&self.provider.base_url, suffix)
    }

    pub fn has_key(&self) -> bool {
        self.api_key.as_deref().is_some_and(|k| !k.trim().is_empty())
    }
}

pub fn capabilities_for(
    rt: &ProviderRuntime,
    model: &str,
) -> Option<crate::models::ModelCapabilities> {
    rt.provider
        .models
        .iter()
        .find(|entry| entry.id == model)
        .map(|entry| entry.capabilities.clone())
}

pub fn capability_baseline(
    provider: &crate::models::Provider,
    _model_id: &str,
) -> crate::models::ModelCapabilities {
    if provider.category == "image" {
        return crate::models::ModelCapabilities::image_baseline(true, true);
    }
    crate::models::ModelCapabilities::chat_baseline(provider.stream)
}

pub fn mark_capabilities(
    state: &AppState,
    provider_id: &str,
    model_id: &str,
    updates: &[(&str, &str)],
) -> AppResult<()> {
    let conn = state.db();
    let mut provider = crate::db::get_provider_row(&conn, provider_id)?;
    let baseline = capability_baseline(&provider, model_id);
    if !provider.models.iter().any(|model| model.id == model_id) {
        provider.models.push(crate::models::ProviderModel {
            id: model_id.to_string(),
            name: model_id.to_string(),
            context_length: None,
            capabilities: baseline,
            source: "runtime".into(),
        });
    }
    let Some(model) = provider
        .models
        .iter_mut()
        .find(|model| model.id == model_id)
    else {
        return Ok(());
    };
    for (field, value) in updates {
        match *field {
            "chat" => model.capabilities.chat = (*value).to_string(),
            "stream" => model.capabilities.stream = (*value).to_string(),
            "tools" => model.capabilities.tools = (*value).to_string(),
            "vision" => model.capabilities.vision = (*value).to_string(),
            "imageGen" => model.capabilities.image_gen = (*value).to_string(),
            "imageEdit" => model.capabilities.image_edit = (*value).to_string(),
            "json" => model.capabilities.json = (*value).to_string(),
            "longContext" => model.capabilities.long_context = (*value).to_string(),
            "multiTool" => model.capabilities.multi_tool = (*value).to_string(),
            "reasoning" => model.capabilities.reasoning = (*value).to_string(),
            _ => {}
        }
    }
    conn.execute(
        "UPDATE providers SET models_json = ?2, updated_at = ?3 WHERE id = ?1",
        rusqlite::params![
            provider_id,
            serde_json::to_string(&provider.models)?,
            crate::db::now()
        ],
    )?;
    Ok(())
}

pub fn load_runtime(state: &AppState, provider_id: &str) -> AppResult<ProviderRuntime> {
    let (provider, user_agent) = {
        let conn = state.db();
        let provider = crate::db::get_provider_row(&conn, provider_id)?;
        let user_agent = crate::db::load_settings(&conn).user_agent;
        (provider, user_agent)
    };
    let api_key = secrets::get_api_key(provider_id);
    Ok(ProviderRuntime {
        provider,
        api_key,
        user_agent,
    })
}

pub fn load_chat_runtime(state: &AppState, provider_id: &str) -> AppResult<ProviderRuntime> {
    let rt = load_runtime(state, provider_id)?;
    if rt.provider.category != "chat" {
        return Err(AppError::msg("所选供应商不是对话供应商"));
    }
    if rt.provider.is_template {
        return Err(AppError::msg("模板不能直接用于对话，请先创建供应商配置"));
    }
    if !rt.provider.enabled {
        return Err(AppError::msg("供应商已被禁用"));
    }
    Ok(rt)
}

pub fn load_image_runtime(state: &AppState, provider_id: &str) -> AppResult<ProviderRuntime> {
    let rt = load_runtime(state, provider_id)?;
    if rt.provider.category != "image" {
        return Err(AppError::msg("所选供应商不是图像供应商"));
    }
    if rt.provider.is_template {
        return Err(AppError::msg("模板不能直接用于生成，请先创建供应商配置"));
    }
    if !rt.provider.enabled {
        return Err(AppError::msg("图像供应商已被禁用"));
    }
    Ok(rt)
}

pub fn chat_adapter(protocol: &str) -> AppResult<Box<dyn ChatAdapter>> {
    match protocol {
        "openai_chat" => Ok(Box::new(openai_chat::OpenAiChatAdapter)),
        "openai_responses" => Ok(Box::new(openai_responses::OpenAiResponsesAdapter)),
        "anthropic" => Ok(Box::new(anthropic::AnthropicAdapter)),
        "gemini" => Ok(Box::new(gemini::GeminiAdapter)),
        "ollama" => Ok(Box::new(ollama::OllamaAdapter)),
        other => Err(AppError::msg(format!("不支持的对话协议：{other}"))),
    }
}

pub fn image_adapter(protocol: &str) -> AppResult<Box<dyn ImageAdapter>> {
    match protocol {
        "openai_images" => Ok(Box::new(images::OpenAiImageAdapter)),
        "gemini_images" => Ok(Box::new(images::GeminiImageAdapter)),
        other => Err(AppError::msg(format!("不支持的图像协议：{other}"))),
    }
}

#[async_trait]
pub trait ChatAdapter: Send + Sync {
    async fn list_models(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
    ) -> AppResult<Vec<DiscoveredModel>>;

    async fn chat(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
        req: &ChatRequest,
        emit: &UnboundedSender<ProviderEvent>,
        cancel: &CancellationToken,
    ) -> AppResult<()>;
}

#[derive(Clone, Debug)]
pub struct ImageRequest {
    pub model: String,
    pub prompt: String,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub count: u32,
    pub background: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ImageEditRequest {
    pub model: String,
    pub prompt: String,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub count: u32,
    pub image: PathBuf,
    pub mask: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct ImageOutput {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mock: bool,
}

#[async_trait]
pub trait ImageAdapter: Send + Sync {
    fn capabilities(&self, rt: &ProviderRuntime, model: &str) -> ImageCapabilities;

    async fn list_models(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
    ) -> AppResult<Vec<DiscoveredModel>>;

    async fn generate(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
        req: &ImageRequest,
    ) -> AppResult<Vec<ImageOutput>>;

    async fn edit(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
        req: &ImageEditRequest,
    ) -> AppResult<Vec<ImageOutput>>;
}
