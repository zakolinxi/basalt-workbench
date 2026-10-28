use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::db::{get_project, load_settings, now, row_to_session};
use crate::error::{AppError, AppResult};
use crate::messaging;
use crate::models::{Message, Session};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateInput {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[tauri::command]
pub fn session_list(state: State<'_, AppState>) -> AppResult<Vec<Session>> {
    let conn = state.db();
    let mut stmt = conn.prepare("SELECT * FROM sessions ORDER BY updated_at DESC LIMIT 200")?;
    let rows = stmt.query_map([], row_to_session)?;
    let collected = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(collected)
}

#[tauri::command]
pub fn session_create(
    state: State<'_, AppState>,
    input: SessionCreateInput,
) -> AppResult<Session> {
    let conn = state.db();
    if let Some(project_id) = input.project_id.as_deref().filter(|id| !id.is_empty()) {
        get_project(&conn, project_id)?;
    }
    let settings = load_settings(&conn);
    let provider_id = input
        .provider_id
        .filter(|id| !id.is_empty())
        .or_else(|| Some(settings.routing.chat.provider_id))
        .filter(|id| !id.is_empty());
    let model = input
        .model
        .filter(|m| !m.is_empty())
        .or_else(|| Some(settings.routing.chat.model))
        .filter(|m| !m.is_empty());
    let title = input
        .title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "新会话".into());
    let mode = input.mode.unwrap_or_else(|| "agent".into());
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    conn.execute(
        "INSERT INTO sessions(id, project_id, title, provider_id, model, mode, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            id,
            input.project_id.filter(|p| !p.is_empty()),
            title,
            provider_id,
            model,
            mode,
            timestamp
        ],
    )?;
    Ok(conn.query_row("SELECT * FROM sessions WHERE id = ?1", [&id], row_to_session)?)
}

#[tauri::command]
pub fn session_update(
    state: State<'_, AppState>,
    session_id: String,
    title: Option<String>,
    provider_id: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<String>,
) -> AppResult<Session> {
    let conn = state.db();
    let current = conn
        .query_row("SELECT * FROM sessions WHERE id = ?1", [&session_id], row_to_session)
        .map_err(|_| AppError::msg("会话不存在"))?;
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| current.title.clone());
    let next_provider = provider_id
        .filter(|p| !p.is_empty())
        .or_else(|| current.provider_id.clone());
    let next_model = model
        .filter(|m| !m.is_empty())
        .or_else(|| current.model.clone());
    let next_effort = match reasoning_effort {
        Some(value) => crate::models::normalize_reasoning_effort(Some(&value)),
        None => current.reasoning_effort.clone(),
    };
    conn.execute(
        "UPDATE sessions SET title = ?2, provider_id = ?3, model = ?4,
         reasoning_effort = ?5, updated_at = ?6 WHERE id = ?1",
        params![session_id, title, next_provider, next_model, next_effort, now()],
    )?;
    Ok(conn.query_row(
        "SELECT * FROM sessions WHERE id = ?1",
        [&session_id],
        row_to_session,
    )?)
}

#[tauri::command]
pub fn session_delete(state: State<'_, AppState>, session_id: String) -> AppResult<()> {
    let conn = state.db();
    let running: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE session_id = ?1 AND status IN ('running', 'waitingApproval')",
        [&session_id],
        |row| row.get(0),
    )?;
    if running > 0 {
        return Err(AppError::msg("该会话仍有运行中的任务，请先取消任务"));
    }
    conn.execute(
        "DELETE FROM task_events WHERE task_id IN (SELECT id FROM tasks WHERE session_id = ?1)",
        [&session_id],
    )?;
    conn.execute(
        "DELETE FROM tool_calls WHERE task_id IN (SELECT id FROM tasks WHERE session_id = ?1)",
        [&session_id],
    )?;
    conn.execute(
        "DELETE FROM changes WHERE task_id IN (SELECT id FROM tasks WHERE session_id = ?1)",
        [&session_id],
    )?;
    conn.execute("DELETE FROM tasks WHERE session_id = ?1", [&session_id])?;
    conn.execute("DELETE FROM messages WHERE session_id = ?1", [&session_id])?;
    conn.execute("DELETE FROM sessions WHERE id = ?1", [&session_id])?;
    Ok(())
}

#[tauri::command]
pub fn message_list(state: State<'_, AppState>, session_id: String) -> AppResult<Vec<Message>> {
    messaging::list_messages(&state, &session_id)
}
