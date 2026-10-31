use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::State;

use crate::db::get_project;
use crate::error::{AppError, AppResult};
use crate::files::{
    ensure_safe_write_path, hash_file, hash_text, language_for, read_text, resolve_in_root,
};
use crate::models::{DirEntry, FileContent, FileDiff, GitStatus, Project};
use crate::state::AppState;

fn project_root(state: &AppState, project_id: &str) -> AppResult<(Project, PathBuf)> {
    let project = {
        let conn = state.db();
        get_project(&conn, project_id)?
    };
    let root = crate::files::canonical_root(Path::new(&project.path))?;
    Ok((project, root))
}

#[tauri::command]
pub fn project_list_dir(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> AppResult<Vec<DirEntry>> {
    let (_, root) = project_root(&state, &project_id)?;
    let dir = resolve_in_root(&root, &path)?;
    if !dir.is_dir() {
        return Err(AppError::msg("目标不是目录"));
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&dir)?.take(3000) {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        let is_dir = meta.is_dir();
        if let Ok(canon) = path.canonicalize() {
            if !crate::files::path_is_within(&root, &canon) {
                continue;
            }
        }
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        entries.push(DirEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: rel,
            is_dir,
            size: if is_dir { 0 } else { meta.len() },
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

#[tauri::command]
pub fn project_read_file(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> AppResult<FileContent> {
    let (_, root) = project_root(&state, &project_id)?;
    let abs = resolve_in_root(&root, &path)?;
    if !abs.is_file() {
        return Err(AppError::msg("文件不存在"));
    }
    let (content, truncated, is_binary, size) = read_text(&abs, 2 * 1024 * 1024)?;
    let hash = hash_file(&abs)?;
    Ok(FileContent {
        path: path.replace('\\', "/"),
        content: if is_binary {
            String::new()
        } else {
            content
        },
        size,
        is_binary,
        truncated,
        language: language_for(&path),
        hash,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWriteInput {
    pub project_id: String,
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub expected_hash: Option<String>,
}

#[tauri::command]
pub fn project_write_file(
    state: State<'_, AppState>,
    input: FileWriteInput,
) -> AppResult<FileContent> {
    let (_, root) = project_root(&state, &input.project_id)?;
    let abs = resolve_in_root(&root, &input.path)?;
    ensure_safe_write_path(&root, &abs)?;
    if input.content.len() > 2 * 1024 * 1024 {
        return Err(AppError::msg("文件内容超过 2MB 限制"));
    }
    if abs.is_dir() {
        return Err(AppError::msg("目标是目录"));
    }
    if abs.exists() {
        if let Some(expected) = input.expected_hash.as_deref().filter(|h| !h.is_empty()) {
            let current = hash_file(&abs)?;
            if current != expected {
                return Err(AppError::msg(
                    "文件已在外部被修改，已阻止覆盖。请重新加载文件后再编辑。",
                ));
            }
        }
        let (_, _, binary, _) = read_text(&abs, 2 * 1024 * 1024)?;
        if binary {
            return Err(AppError::msg("拒绝覆盖二进制文件"));
        }
    }
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&abs, input.content.as_bytes())?;
    let hash = hash_text(&input.content);
    Ok(FileContent {
        path: input.path.replace('\\', "/"),
        language: language_for(&input.path),
        content: input.content,
        size: std::fs::metadata(&abs)?.len(),
        is_binary: false,
        truncated: false,
        hash,
    })
}

#[tauri::command]
pub fn git_status(state: State<'_, AppState>, project_id: String) -> AppResult<GitStatus> {
    let (_, root) = project_root(&state, &project_id)?;
    Ok(crate::git::status(&root))
}

#[tauri::command]
pub fn git_branches(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<String>> {
    let (_, root) = project_root(&state, &project_id)?;
    crate::git::branches(&root)
}

#[tauri::command]
pub fn git_checkout_branch(
    state: State<'_, AppState>,
    project_id: String,
    branch: String,
) -> AppResult<GitStatus> {
    let (_, root) = project_root(&state, &project_id)?;
    crate::git::checkout_branch(&root, &branch)?;
    Ok(crate::git::status(&root))
}

#[tauri::command]
pub fn git_create_branch(
    state: State<'_, AppState>,
    project_id: String,
    branch: String,
) -> AppResult<GitStatus> {
    let (_, root) = project_root(&state, &project_id)?;
    crate::git::create_branch(&root, &branch)?;
    Ok(crate::git::status(&root))
}

#[tauri::command]
pub fn git_file_diff(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> AppResult<FileDiff> {
    let (project, root) = project_root(&state, &project_id)?;
    let clean = path.split(" -> ").last().unwrap_or(&path).to_string();
    let label = project.name.clone();
    crate::git::file_diff(&root, &clean, &label)
}
