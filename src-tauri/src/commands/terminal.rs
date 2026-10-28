use std::path::Path;

use tauri::State;

use crate::db::{get_project, load_settings};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[tauri::command]
pub fn terminal_create(
    state: State<'_, AppState>,
    project_id: Option<String>,
    shell: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> AppResult<String> {
    let settings = {
        let conn = state.db();
        load_settings(&conn)
    };
    let shell = shell.unwrap_or(settings.terminal_shell);
    if !matches!(shell.as_str(), "powershell" | "cmd" | "wsl") {
        return Err(AppError::msg("不支持的终端类型"));
    }
    let cwd = if let Some(project_id) = project_id.filter(|id| !id.is_empty()) {
        let project = {
            let conn = state.db();
            get_project(&conn, &project_id)?
        };
        crate::files::canonical_root(Path::new(&project.path))?
    } else {
        state.data_dir.clone()
    };
    crate::terminal::create_terminal(&state, &cwd, &shell, cols.unwrap_or(100), rows.unwrap_or(30))
}

#[tauri::command]
pub fn terminal_write(
    state: State<'_, AppState>,
    terminal_id: String,
    data: String,
) -> AppResult<()> {
    crate::terminal::write_terminal(&state, &terminal_id, &data)
}

#[tauri::command]
pub fn terminal_resize(
    state: State<'_, AppState>,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> AppResult<()> {
    crate::terminal::resize_terminal(&state, &terminal_id, cols, rows)
}

#[tauri::command]
pub fn terminal_kill(state: State<'_, AppState>, terminal_id: String) -> AppResult<()> {
    crate::terminal::kill_terminal(&state, &terminal_id);
    Ok(())
}
