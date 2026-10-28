use std::path::{Path, PathBuf};

use rusqlite::params;
use serde_json::json;
use uuid::Uuid;

use crate::db::{now, row_to_change};
use crate::error::{AppError, AppResult};
use crate::events::push_task_event;
use crate::files::{ensure_safe_write_path, hash_bytes, hash_file, path_is_within};
use crate::models::{Change, Task};
use crate::state::AppState;

pub struct PendingSnapshot {
    pub old_exists: bool,
    pub old_hash: Option<String>,
    pub snapshot_path: Option<PathBuf>,
}

pub fn prepare_snapshot(
    state: &AppState,
    task_id: &str,
    abs_path: &Path,
) -> AppResult<PendingSnapshot> {
    if !abs_path.exists() {
        return Ok(PendingSnapshot {
            old_exists: false,
            old_hash: None,
            snapshot_path: None,
        });
    }
    let meta = std::fs::metadata(abs_path)?;
    if meta.len() > 5 * 1024 * 1024 {
        return Err(AppError::msg("目标文件超过 5MB，拒绝自动修改"));
    }
    let bytes = std::fs::read(abs_path)?;
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return Err(AppError::msg("目标是二进制文件，拒绝文本写入"));
    }
    let snapshot_dir = state.data_dir.join("snapshots").join(task_id);
    std::fs::create_dir_all(&snapshot_dir)?;
    let snapshot_path = snapshot_dir.join(format!("{}.bak", Uuid::new_v4()));
    std::fs::write(&snapshot_path, &bytes)?;
    Ok(PendingSnapshot {
        old_exists: true,
        old_hash: Some(hash_bytes(&bytes)),
        snapshot_path: Some(snapshot_path),
    })
}

pub fn commit_change(
    state: &AppState,
    task: &Task,
    root: &Path,
    rel_path: &str,
    abs_path: &Path,
    snapshot: PendingSnapshot,
) -> AppResult<Option<Change>> {
    let new_hash = if abs_path.exists() {
        Some(hash_file(abs_path)?)
    } else {
        None
    };
    if snapshot.old_hash == new_hash {
        if let Some(path) = &snapshot.snapshot_path {
            let _ = std::fs::remove_file(path);
        }
        return Ok(None);
    }
    let change_id = Uuid::new_v4().to_string();
    let created_at = now();
    let normalized_rel = rel_path.replace('\\', "/");
    {
        let conn = state.db();
        conn.execute(
            "INSERT INTO changes(id, task_id, project_id, path, old_exists, new_exists, old_hash, new_hash, snapshot_path, status, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending', ?10)",
            params![
                change_id,
                task.id,
                task.project_id,
                normalized_rel,
                snapshot.old_exists as i64,
                new_hash.is_some() as i64,
                snapshot.old_hash,
                new_hash,
                snapshot
                    .snapshot_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string()),
                created_at
            ],
        )?;
    }
    if !path_is_within(root, abs_path) {
        return Err(AppError::msg("变更路径校验失败"));
    }
    let payload = json!({
        "changeId": change_id,
        "path": normalized_rel,
        "status": "pending",
        "taskId": task.id
    });
    push_task_event(state, &task.id, "FileChanged", payload.clone());
    push_task_event(state, &task.id, "DiffUpdated", payload);
    let conn = state.db();
    conn.query_row(
        "SELECT * FROM changes WHERE id = ?1",
        [change_id.as_str()],
        row_to_change,
    )
    .map(Some)
    .map_err(AppError::from)
}

pub fn write_file_tracked(
    state: &AppState,
    task: &Task,
    root: &Path,
    rel_path: &str,
    content: &str,
) -> AppResult<Option<Change>> {
    let abs_path = crate::files::resolve_in_root(root, rel_path)?;
    ensure_safe_write_path(root, &abs_path)?;
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let snapshot = prepare_snapshot(state, &task.id, &abs_path)?;
    std::fs::write(&abs_path, content.as_bytes())?;
    commit_change(state, task, root, rel_path, &abs_path, snapshot)
}

pub fn list_changes_conn(
    conn: &rusqlite::Connection,
    task_id: Option<&str>,
    project_id: Option<&str>,
) -> AppResult<Vec<Change>> {
    if let Some(task_id) = task_id {
        let mut stmt =
            conn.prepare("SELECT * FROM changes WHERE task_id = ?1 ORDER BY created_at ASC")?;
        let rows = stmt.query_map([task_id], row_to_change)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    } else if let Some(project_id) = project_id {
        let mut stmt = conn.prepare(
            "SELECT * FROM changes WHERE project_id = ?1 ORDER BY created_at DESC LIMIT 200",
        )?;
        let rows = stmt.query_map([project_id], row_to_change)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    } else {
        let mut stmt =
            conn.prepare("SELECT * FROM changes ORDER BY created_at DESC LIMIT 200")?;
        let rows = stmt.query_map([], row_to_change)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

pub fn list_changes(
    state: &AppState,
    task_id: Option<&str>,
    project_id: Option<&str>,
) -> AppResult<Vec<Change>> {
    let conn = state.db();
    list_changes_conn(&conn, task_id, project_id)
}

fn load_change_raw(state: &AppState, change_id: &str) -> AppResult<(Change, Option<String>)> {
    let conn = state.db();
    conn.query_row(
        "SELECT id, task_id, project_id, path, old_exists, new_exists, old_hash, new_hash,
         status, created_at, snapshot_path FROM changes WHERE id = ?1",
        [change_id],
        |row| Ok((row_to_change(row)?, row.get::<_, Option<String>>("snapshot_path")?)),
    )
    .map_err(|_| AppError::msg("变更记录不存在"))
}

pub fn resolve_change(
    state: &AppState,
    change_id: &str,
    action: &str,
    force: bool,
) -> AppResult<Change> {
    if !matches!(action, "accept" | "revert") {
        return Err(AppError::msg("无效的变更处理动作"));
    }
    let (change, snapshot_raw) = load_change_raw(state, change_id)?;
    let root = {
        let conn = state.db();
        let project = crate::db::get_project(&conn, &change.project_id)?;
        crate::files::canonical_root(Path::new(&project.path))?
    };
    let abs_path = crate::files::resolve_in_root(&root, &change.path)?;
    let current_hash = if abs_path.exists() {
        Some(hash_file(&abs_path)?)
    } else {
        None
    };
    let matches = current_hash == change.new_hash;
    if !matches && !force {
        {
            let conn = state.db();
            let _ = conn.execute(
                "UPDATE changes SET status = 'conflict' WHERE id = ?1",
                [change_id],
            );
        }
        return Err(AppError::msg(format!(
            "检测到冲突：{} 在变更后被修改或删除。请先审查当前内容，再选择强制处理。",
            change.path
        )));
    }

    if action == "accept" {
        let conn = state.db();
        conn.execute(
            "UPDATE changes SET status = 'accepted' WHERE id = ?1",
            [change_id],
        )?;
    } else {
        if change.old_exists {
            let snapshot = snapshot_raw
                .as_deref()
                .map(PathBuf::from)
                .ok_or_else(|| AppError::msg("缺少变更快照，无法撤销"))?;
            if !snapshot.exists() {
                return Err(AppError::msg("变更快照文件已丢失，无法撤销"));
            }
            let old_bytes = std::fs::read(&snapshot)?;
            if let Some(parent) = abs_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&abs_path, old_bytes)?;
        } else if abs_path.exists() {
            std::fs::remove_file(&abs_path)?;
        }
        let conn = state.db();
        conn.execute(
            "UPDATE changes SET status = 'reverted' WHERE id = ?1",
            [change_id],
        )?;
    }

    let payload = json!({
        "changeId": change.id,
        "path": change.path,
        "status": if action == "accept" { "accepted" } else { "reverted" },
        "taskId": change.task_id,
        "forced": force,
    });
    push_task_event(state, &change.task_id, "FileChanged", payload.clone());
    push_task_event(state, &change.task_id, "DiffUpdated", payload);
    let conn = state.db();
    conn.query_row("SELECT * FROM changes WHERE id = ?1", [change_id], row_to_change)
        .map_err(AppError::from)
}

pub fn change_diff(state: &AppState, change_id: &str) -> AppResult<crate::models::FileDiff> {
    let (change, snapshot_raw) = load_change_raw(state, change_id)?;
    let root = {
        let conn = state.db();
        let project = crate::db::get_project(&conn, &change.project_id)?;
        crate::files::canonical_root(Path::new(&project.path))?
    };
    let abs_path = crate::files::resolve_in_root(&root, &change.path)?;
    let (old_text, mut truncated, mut binary) = if change.old_exists {
        let snapshot = snapshot_raw
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(|| AppError::msg("缺少变更快照"))?;
        let (text, truncated, binary, _) = crate::files::read_text(&snapshot, 1_500_000)?;
        (text, truncated, binary)
    } else {
        (String::new(), false, false)
    };
    let (new_text, new_truncated, new_binary) = if abs_path.exists() {
        let (text, truncated, binary, _) = crate::files::read_text(&abs_path, 1_500_000)?;
        (text, truncated, binary)
    } else {
        (String::new(), false, false)
    };
    truncated = truncated || new_truncated;
    binary = binary || new_binary;
    Ok(crate::models::FileDiff {
        path: change.path,
        old_text,
        new_text,
        old_label: if change.old_exists {
            "变更前".into()
        } else {
            "(新文件)".into()
        },
        new_label: "当前内容".into(),
        binary,
        truncated,
        source: "task".into(),
    })
}
