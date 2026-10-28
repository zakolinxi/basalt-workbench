use std::path::Path;

use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::db::{get_project, now, row_to_project};
use crate::error::{AppError, AppResult};
use crate::files::canonical_root;
use crate::models::Project;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAddInput {
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[tauri::command]
pub fn project_list(state: State<'_, AppState>) -> AppResult<Vec<Project>> {
    let conn = state.db();
    let mut stmt = conn.prepare("SELECT * FROM projects ORDER BY last_opened_at DESC")?;
    let projects = stmt
        .query_map([], row_to_project)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(projects)
}

#[tauri::command]
pub fn project_add(state: State<'_, AppState>, input: ProjectAddInput) -> AppResult<Project> {
    let root = canonical_root(Path::new(input.path.trim()))?;
    let path_string = root.to_string_lossy().to_string();
    let name = input
        .name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .or_else(|| {
            root.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "未命名项目".into());
    let conn = state.db();
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM projects WHERE path = ?1",
            [&path_string],
            |row| row.get(0),
        )
        .ok();
    if let Some(existing) = existing {
        conn.execute(
            "UPDATE projects SET last_opened_at = ?2 WHERE id = ?1",
            params![existing, now()],
        )?;
        return Ok(get_project(&conn, &existing)?);
    }
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    conn.execute(
        "INSERT INTO projects(id, name, path, created_at, last_opened_at) VALUES(?1, ?2, ?3, ?4, ?4)",
        params![id, name, path_string, timestamp],
    )?;
    Ok(get_project(&conn, &id)?)
}

#[tauri::command]
pub fn project_open(state: State<'_, AppState>, project_id: String) -> AppResult<Project> {
    let conn = state.db();
    get_project(&conn, &project_id)?;
    conn.execute(
        "UPDATE projects SET last_opened_at = ?2 WHERE id = ?1",
        params![project_id, now()],
    )?;
    Ok(get_project(&conn, &project_id)?)
}

#[tauri::command]
pub fn project_remove(state: State<'_, AppState>, project_id: String) -> AppResult<()> {
    {
        let conn = state.db();
        let running: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND status IN ('running', 'waitingApproval')",
            [&project_id],
            |row| row.get(0),
        )?;
        if running > 0 {
            return Err(AppError::msg("该项目仍有运行中的任务，请先取消任务"));
        }
        conn.execute("DELETE FROM projects WHERE id = ?1", [&project_id])?;
    }
    Ok(())
}

