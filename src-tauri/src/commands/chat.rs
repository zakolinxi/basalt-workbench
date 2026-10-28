use std::path::Path;

use base64::Engine;
use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::db::{get_project, insert_message, now, row_to_session};
use crate::error::{AppError, AppResult};
use crate::files::path_is_within;
use crate::messaging;
use crate::models::{
    Attachment, ChatEvent, Message, Session, CAP_UNSUPPORTED, CAP_VERIFIED,
};
use crate::providers::{
    capabilities_for, chat_adapter, load_chat_runtime, mark_capabilities, ChatMessage, ChatRequest,
    ProviderEvent,
};
use crate::state::AppState;

const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_TEXT_BYTES: u64 = 1024 * 1024;
const MAX_ATTACHMENTS: usize = 8;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendInput {
    pub request_id: String,
    pub session_id: String,
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub reuse_last_user: bool,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
}

fn text_extension_allowed(ext: &str) -> bool {
    matches!(
        ext,
        "txt"
            | "md"
            | "mdx"
            | "json"
            | "jsonc"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "ts"
            | "tsx"
            | "rs"
            | "py"
            | "go"
            | "java"
            | "kt"
            | "c"
            | "h"
            | "cpp"
            | "cc"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "swift"
            | "toml"
            | "yaml"
            | "yml"
            | "xml"
            | "html"
            | "htm"
            | "css"
            | "scss"
            | "less"
            | "sql"
            | "sh"
            | "bash"
            | "ps1"
            | "bat"
            | "cmd"
            | "ini"
            | "cfg"
            | "conf"
            | "log"
            | "csv"
            | "tsv"
            | "graphql"
            | "proto"
            | "dockerfile"
    )
}

fn secret_like_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == ".env"
        || lower.starts_with(".env.")
        || matches!(
            lower.as_str(),
            ".npmrc"
                | ".pypirc"
                | ".git-credentials"
                | ".netrc"
                | "id_rsa"
                | "id_ed25519"
                | "credentials.json"
        )
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".pfx")
        || lower.ends_with(".p12")
        || lower.ends_with(".ppk")
}

fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn sanitize_name(raw: &str) -> String {
    let name = Path::new(raw)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "attachment".into());
    let cleaned: String = name
        .chars()
        .filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
        .take(120)
        .collect();
    if cleaned.trim().is_empty() {
        "attachment".into()
    } else {
        cleaned
    }
}

fn store_attachment(
    state: &AppState,
    name: &str,
    bytes: &[u8],
    declared_mime: Option<&str>,
) -> AppResult<Attachment> {
    let name = sanitize_name(name);
    if secret_like_name(&name) {
        return Err(AppError::msg(
            "出于安全考虑，不允许将疑似密钥或凭据文件作为附件上传",
        ));
    }
    let ext = Path::new(&name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let detected_image = sniff_image_mime(bytes);
    let is_image = detected_image.is_some()
        || declared_mime
            .map(|m| m.starts_with("image/"))
            .unwrap_or(false)
        || matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif");
    let (kind, mime, save_ext) = if is_image {
        let Some(mime) = detected_image else {
            return Err(AppError::msg("图片文件格式无效，仅支持 PNG/JPEG/GIF/WebP"));
        };
        if bytes.len() as u64 > MAX_IMAGE_BYTES {
            return Err(AppError::msg("图片超过 10MB 限制"));
        }
        let save_ext = match mime {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            _ => "webp",
        };
        ("image", mime.to_string(), save_ext.to_string())
    } else {
        if bytes.len() as u64 > MAX_TEXT_BYTES {
            return Err(AppError::msg("文本附件超过 1MB 限制"));
        }
        if !text_extension_allowed(&ext) {
            return Err(AppError::msg(format!(
                "不支持的附件类型 .{ext}。仅支持常见文本/代码文件和图片。"
            )));
        }
        if std::str::from_utf8(bytes).is_err() {
            return Err(AppError::msg("文本附件必须是 UTF-8 编码"));
        }
        let mime = mime_guess::from_path(&name)
            .first_raw()
            .unwrap_or("text/plain")
            .to_string();
        ("file", mime, if ext.is_empty() { "txt".into() } else { ext })
    };
    let dir = state.data_dir.join("attachments");
    std::fs::create_dir_all(&dir)?;
    let stored_name = format!("{}.{}", Uuid::new_v4(), save_ext);
    let path = dir.join(stored_name);
    std::fs::write(&path, bytes)?;
    Ok(Attachment {
        name,
        mime,
        size: bytes.len() as u64,
        kind: kind.to_string(),
        path: path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn attachment_import(state: State<'_, AppState>, path: String) -> AppResult<Attachment> {
    let source = Path::new(&path)
        .canonicalize()
        .map_err(|_| AppError::msg("文件不存在"))?;
    if !source.is_file() {
        return Err(AppError::msg("只能导入文件"));
    }
    let meta = std::fs::metadata(&source)?;
    let limit = if source
        .extension()
        .map(|e| {
            matches!(
                e.to_string_lossy().to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif"
            )
        })
        .unwrap_or(false)
    {
        MAX_IMAGE_BYTES
    } else {
        MAX_TEXT_BYTES
    };
    if meta.len() > limit {
        return Err(AppError::msg("文件超过允许的大小限制"));
    }
    let bytes = std::fs::read(&source)?;
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "attachment".into());
    store_attachment(&state, &name, &bytes, None)
}

#[tauri::command]
pub fn attachment_import_bytes(
    state: State<'_, AppState>,
    name: String,
    mime: Option<String>,
    data_base64: String,
) -> AppResult<Attachment> {
    if data_base64.len() > 16 * 1024 * 1024 {
        return Err(AppError::msg("附件数据过大"));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|_| AppError::msg("附件数据不是有效的 Base64"))?;
    store_attachment(&state, &name, &bytes, mime.as_deref())
}

fn ensure_attachment_allowed(state: &AppState, attachment: &Attachment) -> AppResult<()> {
    let path = Path::new(&attachment.path)
        .canonicalize()
        .map_err(|_| AppError::msg("附件文件不存在"))?;
    let allowed_root = state
        .data_dir
        .join("attachments")
        .canonicalize()
        .map_err(|e| AppError::msg(format!("附件目录不可用：{e}")))?;
    if !path_is_within(&allowed_root, &path) || !path.is_file() {
        return Err(AppError::msg("附件路径不在工作台附件目录中"));
    }
    if path.metadata()?.len() > MAX_IMAGE_BYTES.max(MAX_TEXT_BYTES) {
        return Err(AppError::msg("附件超过大小限制"));
    }
    Ok(())
}

#[tauri::command]
pub fn read_media(state: State<'_, AppState>, path: String) -> AppResult<String> {
    let canon = Path::new(&path)
        .canonicalize()
        .map_err(|_| AppError::msg("文件不存在"))?;
    let mut allowed = false;
    for sub in ["attachments", "images"] {
        if let Ok(root) = state.data_dir.join(sub).canonicalize() {
            if path_is_within(&root, &canon) {
                allowed = true;
                break;
            }
        }
    }
    if !allowed {
        return Err(AppError::msg("拒绝读取工作台媒体目录以外的文件"));
    }
    let meta = std::fs::metadata(&canon)?;
    if meta.len() > 25 * 1024 * 1024 {
        return Err(AppError::msg("媒体文件超过 25MB 限制"));
    }
    let bytes = std::fs::read(&canon)?;
    let mime = mime_guess::from_path(&canon)
        .first_raw()
        .unwrap_or("application/octet-stream");
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[tauri::command]
pub fn attachment_delete(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let canon = Path::new(&path)
        .canonicalize()
        .map_err(|_| AppError::msg("附件不存在"))?;
    let root = state.data_dir.join("attachments").canonicalize()?;
    if !path_is_within(&root, &canon) {
        return Err(AppError::msg("拒绝删除附件目录以外的文件"));
    }
    std::fs::remove_file(&canon)?;
    Ok(())
}

fn chat_system_prompt(state: &AppState, session: &Session) -> String {
    let project_note = session
        .project_id
        .as_deref()
        .and_then(|id| {
            let conn = state.db();
            get_project(&conn, id).ok()
        })
        .map(|project| {
            format!(
                "用户当前工作区项目：{}（{}）。你只能基于用户提供的对话与附件回答；普通对话模式没有文件系统或命令执行工具。需要实际修改文件时，提示用户切换到 Agent 任务模式。",
                project.name, project.path
            )
        })
        .unwrap_or_else(|| "当前没有关联项目。".into());
    format!(
        "你是 Basalt，一个运行在 Windows AI 工程工作台中的严谨助手。使用中文回答，使用清晰的 Markdown。\
         不要编造已执行的操作或已读取的文件。{project_note}"
    )
}

async fn chat_send_inner(
    state: &AppState,
    input: ChatSendInput,
    token: &CancellationToken,
) -> AppResult<Message> {
    if input.text.trim().is_empty() && input.attachments.is_empty() {
        return Err(AppError::msg("消息内容不能为空"));
    }
    if input.attachments.len() > MAX_ATTACHMENTS {
        return Err(AppError::msg(format!("单条消息最多 {MAX_ATTACHMENTS} 个附件")));
    }
    let session = {
        let conn = state.db();
        conn.query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [&input.session_id],
            row_to_session,
        )
        .map_err(|_| AppError::msg("会话不存在，请先创建会话"))?
    };
    let rt = load_chat_runtime(state, &input.provider_id)?;
    let model = if input.model.trim().is_empty() {
        rt.provider
            .default_chat_model
            .clone()
            .ok_or_else(|| AppError::msg("未选择模型"))?
    } else {
        input.model.trim().to_string()
    };
    let caps = capabilities_for(&rt, &model);
    let reasoning_effort = if caps
        .as_ref()
        .is_some_and(|caps| caps.reasoning == crate::models::CAP_UNSUPPORTED)
    {
        None
    } else {
        crate::models::normalize_reasoning_effort(
            input
                .reasoning_effort
                .as_deref()
                .or(session.reasoning_effort.as_deref()),
        )
    };
    let has_images = input
        .attachments
        .iter()
        .any(|attachment| attachment.kind == "image");
    if has_images {
        if let Some(caps) = &caps {
            if caps.vision == CAP_UNSUPPORTED {
                return Err(AppError::msg(
                    "所选模型的视觉能力被标记为不支持。请切换支持图片输入的模型，或移除图片附件。",
                ));
            }
        }
    }
    let total_size: u64 = input.attachments.iter().map(|a| a.size).sum();
    if total_size > 25 * 1024 * 1024 {
        return Err(AppError::msg("附件总大小超过 25MB 限制"));
    }
    for attachment in &input.attachments {
        ensure_attachment_allowed(state, attachment)?;
    }

    if !input.reuse_last_user {
        let user_message_id = Uuid::new_v4().to_string();
        {
            let conn = state.db();
            let user_message = insert_message(
                &conn,
                &user_message_id,
                &session.id,
                "user",
                input.text.trim(),
                "text",
                &input.attachments,
                None,
                Some(&model),
            )?;
            drop(conn);
            crate::events::emit(
                &state.app,
                crate::events::EVENT_CHAT,
                ChatEvent {
                    request_id: input.request_id.clone(),
                    session_id: session.id.clone(),
                    task_id: None,
                    kind: "userMessage".into(),
                    text: None,
                    message_id: Some(user_message_id.clone()),
                    message: Some(user_message),
                    error: None,
                },
            );
        }
        {
            let conn = state.db();
            if session.title == "新会话" || session.title.trim().is_empty() {
                let title_source = if input.text.trim().is_empty() {
                    input
                        .attachments
                        .first()
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| "附件对话".into())
                } else {
                    input.text.trim().to_string()
                };
                let title: String = title_source.lines().next().unwrap_or("新会话").chars().take(48).collect();
                conn.execute(
                    "UPDATE sessions SET title = ?2, provider_id = ?3, model = ?4, updated_at = ?5 WHERE id = ?1",
                    params![session.id, title, input.provider_id, model, now()],
                )?;
            } else {
                conn.execute(
                    "UPDATE sessions SET provider_id = ?2, model = ?3, updated_at = ?4 WHERE id = ?1",
                    params![session.id, input.provider_id, model, now()],
                )?;
            }
        }
    } else {
        let exists: i64 = {
            let conn = state.db();
            conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = ?1 AND role = 'user'",
                [&session.id],
                |row| row.get(0),
            )?
        };
        if exists == 0 {
            return Err(AppError::msg("没有可重试的用户消息"));
        }
    }

    let include_images = caps
        .as_ref()
        .map(|caps| caps.vision != CAP_UNSUPPORTED)
        .unwrap_or(true);
    let mut request_messages = vec![ChatMessage::text(
        "system",
        chat_system_prompt(state, &session),
    )];
    request_messages.extend(messaging::history_for_model(
        state,
        &session.id,
        40,
        include_images,
    )?);
    if request_messages.len() < 2 {
        return Err(AppError::msg("会话中没有可用消息"));
    }

    let assistant_message_id = Uuid::new_v4().to_string();
    let adapter = chat_adapter(&rt.provider.protocol)?;
    let client = state.http.clone();
    let rt_for = rt.clone();
    let request = ChatRequest {
        model: model.clone(),
        messages: request_messages,
        tools: Vec::new(),
        max_tokens: None,
        temperature: None,
        reasoning_effort,
    };
    let (tx, mut rx) = unbounded_channel::<ProviderEvent>();
    let token_for = token.clone();
    let handle = tauri::async_runtime::spawn(async move {
        adapter
            .chat(&client, &rt_for, &request, &tx, &token_for)
            .await
    });
    let mut text = String::new();
    while let Some(event) = rx.recv().await {
        if let ProviderEvent::TextDelta(delta) = event {
            text.push_str(&delta);
            crate::events::emit(
                &state.app,
                crate::events::EVENT_CHAT,
                ChatEvent {
                    request_id: input.request_id.clone(),
                    session_id: session.id.clone(),
                    task_id: None,
                    kind: "delta".into(),
                    text: Some(delta),
                    message_id: Some(assistant_message_id.clone()),
                    message: None,
                    error: None,
                },
            );
        }
    }
    let provider_result = handle
        .await
        .map_err(|e| AppError::msg(format!("对话任务异常：{e}")))?;

    match provider_result {
        Ok(()) => {
            if token.is_cancelled() {
                if !text.is_empty() {
                    let message = messaging::add_message(
                        state,
                        Some(&assistant_message_id),
                        &session.id,
                        "assistant",
                        &text,
                        "text",
                        &[],
                        None,
                        Some(&model),
                    )?;
                    emit_final(state, &input.request_id, &session.id, &assistant_message_id, message);
                }
                return Err(AppError::Cancelled);
            }
            if text.trim().is_empty() {
                let err = AppError::msg("模型没有返回文本内容，请重试或检查供应商配置");
                emit_error(state, &input.request_id, &session.id, &err.user_message());
                return Err(err);
            }
            let message = messaging::add_message(
                state,
                Some(&assistant_message_id),
                &session.id,
                "assistant",
                &text,
                "text",
                &[],
                None,
                Some(&model),
            )?;
            emit_final(state, &input.request_id, &session.id, &assistant_message_id, message.clone());
            let mut updates = vec![("chat", CAP_VERIFIED)];
            if rt.provider.stream {
                updates.push(("stream", CAP_VERIFIED));
            }
            if has_images {
                updates.push(("vision", CAP_VERIFIED));
            }
            let _ = mark_capabilities(state, &input.provider_id, &model, &updates);
            Ok(message)
        }
        Err(err) if err.is_cancelled() => {
            if !text.is_empty() {
                let message = messaging::add_message(
                    state,
                    Some(&assistant_message_id),
                    &session.id,
                    "assistant",
                    &text,
                    "text",
                    &[],
                    None,
                    Some(&model),
                )?;
                emit_final(state, &input.request_id, &session.id, &assistant_message_id, message);
            }
            crate::events::emit(
                &state.app,
                crate::events::EVENT_CHAT,
                ChatEvent {
                    request_id: input.request_id.clone(),
                    session_id: session.id.clone(),
                    task_id: None,
                    kind: "cancelled".into(),
                    text: None,
                    message_id: Some(assistant_message_id),
                    message: None,
                    error: Some("已停止生成".into()),
                },
            );
            Err(AppError::Cancelled)
        }
        Err(err) => {
            emit_error(state, &input.request_id, &session.id, &err.user_message());
            Err(err)
        }
    }
}

fn emit_final(
    state: &AppState,
    request_id: &str,
    session_id: &str,
    message_id: &str,
    message: Message,
) {
    crate::events::emit(
        &state.app,
        crate::events::EVENT_CHAT,
        ChatEvent {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            task_id: None,
            kind: "message".into(),
            text: None,
            message_id: Some(message_id.to_string()),
            message: Some(message),
            error: None,
        },
    );
}

fn emit_error(state: &AppState, request_id: &str, session_id: &str, error: &str) {
    crate::events::emit(
        &state.app,
        crate::events::EVENT_CHAT,
        ChatEvent {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            task_id: None,
            kind: "error".into(),
            text: None,
            message_id: None,
            message: None,
            error: Some(error.to_string()),
        },
    );
}

#[tauri::command]
pub async fn chat_send(
    state: State<'_, AppState>,
    input: ChatSendInput,
) -> AppResult<Message> {
    if input.request_id.trim().is_empty() {
        return Err(AppError::msg("缺少 requestId"));
    }
    let token = CancellationToken::new();
    {
        let mut cancels = state
            .chat_cancels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if cancels.contains_key(&input.request_id) {
            return Err(AppError::msg("该请求已在执行中"));
        }
        cancels.insert(input.request_id.clone(), token.clone());
    }
    let result = chat_send_inner(&state, input.clone(), &token).await;
    {
        let mut cancels = state
            .chat_cancels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cancels.remove(&input.request_id);
    }
    result
}

#[tauri::command]
pub fn chat_cancel(state: State<'_, AppState>, request_id: String) -> AppResult<bool> {
    let token = {
        let cancels = state
            .chat_cancels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cancels.get(&request_id).cloned()
    };
    if let Some(token) = token {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}
