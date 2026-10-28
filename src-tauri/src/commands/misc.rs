use std::path::Path;
use std::process::Command;

use rusqlite::params;
use tauri::State;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::{Bootstrap, Provider, Settings, MAX_AGENT_MAX_ITERATIONS};
use crate::secrets;
use crate::state::AppState;

fn attach_key(mut provider: Provider) -> Provider {
    provider.has_key = secrets::has_api_key(&provider.id);
    provider
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> AppResult<Bootstrap> {
    let conn = state.db();
    let settings = db::load_settings(&conn);
    let providers = db::list_provider_rows(&conn)?
        .into_iter()
        .map(attach_key)
        .collect();
    let projects = {
        let mut stmt = conn.prepare("SELECT * FROM projects ORDER BY last_opened_at DESC")?;
        let rows = stmt.query_map([], db::row_to_project)?;
        let collected = rows.collect::<Result<Vec<_>, _>>()?;
        collected
    };
    let sessions = {
        let mut stmt =
            conn.prepare("SELECT * FROM sessions ORDER BY updated_at DESC LIMIT 200")?;
        let rows = stmt.query_map([], db::row_to_session)?;
        let collected = rows.collect::<Result<Vec<_>, _>>()?;
        collected
    };
    let tasks = {
        let mut stmt = conn.prepare("SELECT * FROM tasks ORDER BY created_at DESC LIMIT 300")?;
        let rows = stmt.query_map([], db::row_to_task)?;
        let collected = rows.collect::<Result<Vec<_>, _>>()?;
        collected
    };
    Ok(Bootstrap {
        settings,
        providers,
        projects,
        sessions,
        tasks,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: state.data_dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> AppResult<Settings> {
    let conn = state.db();
    Ok(db::load_settings(&conn))
}

#[tauri::command]
pub fn settings_set(state: State<'_, AppState>, mut settings: Settings) -> AppResult<Settings> {
    settings.agent_max_iterations = settings
        .agent_max_iterations
        .clamp(1, MAX_AGENT_MAX_ITERATIONS);
    if !matches!(settings.theme.as_str(), "dark" | "light" | "system") {
        settings.theme = "dark".into();
    }
    if !matches!(settings.terminal_shell.as_str(), "powershell" | "cmd" | "wsl") {
        settings.terminal_shell = "powershell".into();
    }
    settings.editor_font_size = settings.editor_font_size.clamp(10, 24);
    settings.chat_font_size = settings.chat_font_size.clamp(11, 22);
    let conn = state.db();
    db::save_settings(&conn, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn resolve_approval(
    state: State<'_, AppState>,
    call_id: String,
    approved: bool,
) -> AppResult<bool> {
    let sender = {
        let mut approvals = state
            .approvals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        approvals.remove(&call_id)
    };
    if let Some(sender) = sender {
        let _ = sender.send(approved);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn open_path(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let path = Path::new(&path);
    if !path.exists() {
        return Err(AppError::msg("路径不存在"));
    }
    let mut command = Command::new("explorer.exe");
    if path.is_dir() {
        command.arg(path);
    } else {
        command.arg(format!("/select,{}", path.to_string_lossy()));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .map_err(|e| AppError::msg(format!("无法打开资源管理器：{e}")))?;
    let _ = state;
    Ok(())
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    Ok(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "dataDir": state.data_dir.to_string_lossy(),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH
    }))
}

#[tauri::command]
pub fn session_message_count(state: State<'_, AppState>, session_id: String) -> AppResult<i64> {
    let conn = state.db();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
        params![session_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn reveal_data_dir(state: &AppState) -> AppResult<()> {
    let mut command = Command::new("explorer.exe");
    command.arg(&state.data_dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .map_err(|e| AppError::msg(format!("无法打开数据目录：{e}")))?;
    Ok(())
}
