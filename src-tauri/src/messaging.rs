use std::path::Path;

use base64::Engine;

use crate::db::{insert_message, row_to_message};
use crate::error::AppResult;
use crate::models::{Attachment, Message};
use crate::providers::{ChatImage, ChatMessage};
use crate::state::AppState;

pub fn list_messages(state: &AppState, session_id: &str) -> AppResult<Vec<Message>> {
    let conn = state.db();
    let mut stmt =
        conn.prepare("SELECT * FROM messages WHERE session_id = ?1 ORDER BY created_at ASC")?;
    let rows = stmt
        .query_map([session_id], row_to_message)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn add_message(
    state: &AppState,
    id: Option<&str>,
    session_id: &str,
    role: &str,
    content: &str,
    kind: &str,
    attachments: &[Attachment],
    task_id: Option<&str>,
    model: Option<&str>,
) -> AppResult<Message> {
    let id = id
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let conn = state.db();
    insert_message(
        &conn,
        &id,
        session_id,
        role,
        content,
        kind,
        attachments,
        task_id,
        model,
    )
}

pub fn history_for_model(
    state: &AppState,
    session_id: &str,
    limit: usize,
    include_images: bool,
) -> AppResult<Vec<ChatMessage>> {
    let messages = list_messages(state, session_id)?;
    let start = messages.len().saturating_sub(limit);
    let mut out = Vec::new();
    let mut image_count = 0usize;
    for message in &messages[start..] {
        if message.kind == "image" {
            continue;
        }
        if message.role == "system" {
            continue;
        }
        let mut content = message.content.clone();
        let mut images = Vec::new();
        if message.role == "user" {
            let image_attachments: Vec<&Attachment> = message
                .attachments
                .iter()
                .filter(|a| a.kind == "image")
                .collect();
            if !image_attachments.is_empty() {
                if include_images && image_count < 6 {
                    for attachment in &image_attachments {
                        if image_count >= 6 {
                            break;
                        }
                        if let Some(image) = load_image(attachment) {
                            images.push(image);
                            image_count += 1;
                        }
                    }
                }
                let names = image_attachments
                    .iter()
                    .map(|a| a.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                if !content.contains("[图片") {
                    content.push_str(&format!("\n\n[用户附带了图片：{names}]"));
                }
            }
            let files: Vec<&Attachment> = message
                .attachments
                .iter()
                .filter(|a| a.kind != "image")
                .collect();
            if !files.is_empty() {
                content.push_str(&format!(
                    "\n\n[用户附带了文件：{}]",
                    files.iter().map(|a| a.name.clone()).collect::<Vec<_>>().join(", ")
                ));
                let mut total_chars = 0usize;
                for attachment in &files {
                    if total_chars >= 200_000 {
                        content.push_str("\n\n[其余附件内容因长度限制未包含]");
                        break;
                    }
                    if let Ok((text, truncated, binary, _)) =
                        crate::files::read_text(Path::new(&attachment.path), 200_000)
                    {
                        if binary {
                            continue;
                        }
                        let (text, cut) = crate::files::truncate_chars(&text, 100_000);
                        total_chars += text.chars().count();
                        content.push_str(&format!(
                            "\n\n--- 附件: {} ---\n~~~\n{}{}\n~~~",
                            attachment.name,
                            text,
                            if truncated || cut { "\n[附件内容已截断]" } else { "" }
                        ));
                    }
                }
            }
        }
        if content.trim().is_empty() && images.is_empty() {
            continue;
        }
        out.push(ChatMessage {
            role: message.role.clone(),
            content,
            images,
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
        });
    }
    Ok(out)
}

pub fn load_image(attachment: &Attachment) -> Option<ChatImage> {
    let path = Path::new(&attachment.path);
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 12 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    Some(ChatImage {
        mime: attachment.mime.clone(),
        base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}
