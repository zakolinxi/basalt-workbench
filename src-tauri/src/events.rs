use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::db::{now, set_setting_raw};
use crate::error::AppResult;
use crate::models::{TaskEventEnvelope, TASK_INTERRUPTED};
use crate::state::AppState;

pub const EVENT_TASK: &str = "basalt:task-event";
pub const EVENT_CHAT: &str = "basalt:chat-event";
pub const EVENT_TERMINAL: &str = "basalt:terminal-output";
pub const EVENT_TERMINAL_EXIT: &str = "basalt:terminal-exit";
pub const EVENT_IMAGE: &str = "basalt:image-event";

pub fn emit<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: T) {
    let _ = app.emit(event, payload);
}

pub fn push_task_event(state: &AppState, task_id: &str, event_type: &str, payload: Value) {
    let at = now();
    {
        let conn = state.db();
        let _ = conn.execute(
            "INSERT INTO task_events(task_id, event_type, payload_json, created_at) VALUES(?1, ?2, ?3, ?4)",
            rusqlite::params![task_id, event_type, payload.to_string(), at],
        );
        let _ = conn.execute(
            "UPDATE tasks SET updated_at = ?2 WHERE id = ?1",
            rusqlite::params![task_id, at],
        );
    }
    emit(
        &state.app,
        EVENT_TASK,
        TaskEventEnvelope {
            task_id: task_id.to_string(),
            event_type: event_type.to_string(),
            payload,
            at,
        },
    );
}

pub fn set_task_status(state: &AppState, task_id: &str, status: &str) -> AppResult<()> {
    let conn = state.db();
    conn.execute(
        "UPDATE tasks SET status = ?2, updated_at = ?3 WHERE id = ?1",
        rusqlite::params![task_id, status, now()],
    )?;
    Ok(())
}

pub fn finish_task(
    state: &AppState,
    task_id: &str,
    status: &str,
    report: Option<&str>,
    error: Option<&str>,
) {
    {
        let conn = state.db();
        let _ = conn.execute(
            "UPDATE tasks SET status = ?2, report = COALESCE(?3, report), error = ?4, updated_at = ?5 WHERE id = ?1",
            rusqlite::params![task_id, status, report, error, now()],
        );
    }
    let mut payload = serde_json::json!({ "status": status });
    if let Some(report) = report {
        payload["report"] = Value::String(report.to_string());
    }
    if let Some(error) = error {
        payload["error"] = Value::String(error.to_string());
    }
    let event_type = match status {
        "completed" => "TaskCompleted",
        "cancelled" => "TaskCancelled",
        "interrupted" => "TaskInterrupted",
        _ => "TaskFailed",
    };
    push_task_event(state, task_id, event_type, payload);
}

pub fn recover_stale_tasks(state: &AppState) {
    let conn = state.db();
    let _ = conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE status IN ('running', 'waitingApproval', 'queued')",
        rusqlite::params![TASK_INTERRUPTED, now()],
    );
    let _ = set_setting_raw(&conn, "lastCleanExit", "0");
}

pub fn mark_clean_exit(state: &AppState) {
    let conn = state.db();
    let _ = set_setting_raw(&conn, "lastCleanExit", "1");
}
