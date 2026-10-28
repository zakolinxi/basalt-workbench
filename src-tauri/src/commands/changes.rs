use serde::Deserialize;
use tauri::State;

use crate::changes::{change_diff, list_changes, resolve_change};
use crate::error::AppResult;
use crate::models::{Change, FileDiff};
use crate::state::AppState;

#[tauri::command]
pub fn change_list(
    state: State<'_, AppState>,
    task_id: Option<String>,
    project_id: Option<String>,
) -> AppResult<Vec<Change>> {
    list_changes(
        &state,
        task_id.as_deref().filter(|id| !id.is_empty()),
        project_id.as_deref().filter(|id| !id.is_empty()),
    )
}

#[tauri::command]
pub fn change_diff_cmd(state: State<'_, AppState>, change_id: String) -> AppResult<FileDiff> {
    change_diff(&state, &change_id)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeResolveInput {
    pub change_id: String,
    pub action: String,
    #[serde(default)]
    pub force: bool,
}

#[tauri::command]
pub fn change_resolve(
    state: State<'_, AppState>,
    input: ChangeResolveInput,
) -> AppResult<Change> {
    resolve_change(&state, &input.change_id, &input.action, input.force)
}
