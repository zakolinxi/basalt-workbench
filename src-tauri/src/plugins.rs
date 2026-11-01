use std::path::{Path, PathBuf};

use rusqlite::Row;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::now;
use crate::error::{AppError, AppResult};
use crate::files::path_is_within;
use crate::models::{InstalledPlugin, McpServerInput};
use crate::state::AppState;

const MAX_FILES: usize = 2000;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const SKIP_DIRS: [&str; 6] = [".git", "node_modules", "target", "dist", "__pycache__", ".venv"];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    mcp_servers: std::collections::BTreeMap<String, ManifestMcp>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestMcp {
    #[serde(default = "default_transport")]
    transport: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: serde_json::Value,
    #[serde(default)]
    url: String,
    #[serde(default)]
    headers: serde_json::Value,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    auto_approve: bool,
}

fn default_transport() -> String {
    "stdio".into()
}

fn default_enabled() -> bool {
    true
}

fn is_heavy(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(|name| SKIP_DIRS.contains(&name))
            .unwrap_or(false)
}

fn copy_tree(source: &Path, target: &Path) -> AppResult<()> {
    let mut files = 0usize;
    let mut bytes = 0u64;
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_heavy(entry))
        .filter_map(Result::ok)
    {
        if entry.file_type().is_symlink() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(source)
            .map_err(|_| AppError::msg("插件路径无效"))?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = target.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        files += 1;
        bytes += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
        if files > MAX_FILES {
            return Err(AppError::msg(format!(
                "插件包含超过 {MAX_FILES} 个文件，已拒绝安装"
            )));
        }
        if bytes > MAX_BYTES {
            return Err(AppError::msg("插件总大小超过 64MB，已拒绝安装"));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(entry.path(), &dest)?;
    }
    Ok(())
}

fn safe_join(base: &Path, relative: &str) -> AppResult<PathBuf> {
    let rel = Path::new(relative);
    if rel.is_absolute() {
        return Err(AppError::msg("插件内的 Skill 路径必须是相对路径"));
    }
    let joined = base.join(rel);
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let canonical = joined
        .canonicalize()
        .map_err(|_| AppError::msg(format!("插件缺少 Skill 路径：{relative}")))?;
    if !path_is_within(&canonical_base, &canonical) {
        return Err(AppError::msg("插件包含越界路径"));
    }
    Ok(canonical)
}

fn manifest_path(source: &Path) -> AppResult<(PathBuf, PathBuf)> {
    let source = source
        .canonicalize()
        .map_err(|e| AppError::msg(format!("无法读取插件路径：{e}")))?;
    let base = if source.is_file() {
        source
            .parent()
            .ok_or_else(|| AppError::msg("插件路径无效"))?
            .to_path_buf()
    } else {
        source.clone()
    };
    for name in ["plugin.json", "basalt-plugin.json"] {
        let candidate = base.join(name);
        if candidate.is_file() {
            return Ok((base, candidate));
        }
    }
    Err(AppError::msg(
        "插件文件夹需要包含 plugin.json（或 basalt-plugin.json）",
    ))
}

fn row_to_plugin(row: &Row<'_>) -> rusqlite::Result<InstalledPlugin> {
    Ok(InstalledPlugin {
        id: row.get("id")?,
        name: row.get("name")?,
        version: row.get("version")?,
        description: row.get("description")?,
        path: row.get("path")?,
        skills: row.get::<_, i64>(6)?.max(0) as u64,
        mcp_servers: row.get::<_, i64>(7)?.max(0) as u64,
        created_at: row.get("created_at")?,
    })
}

pub fn list(state: &AppState) -> AppResult<Vec<InstalledPlugin>> {
    let conn = state.db();
    let mut stmt = conn.prepare(
        "SELECT p.*,
                (SELECT COUNT(*) FROM skills s WHERE s.plugin_id = p.id) AS skill_count,
                (SELECT COUNT(*) FROM mcp_servers m WHERE m.plugin_id = p.id) AS mcp_count
         FROM plugins p ORDER BY p.created_at DESC",
    )?;
    let rows = stmt.query_map([], row_to_plugin)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn install(state: &AppState, source: &Path) -> AppResult<InstalledPlugin> {
    let (base, manifest_file) = manifest_path(source)?;
    let raw = std::fs::read_to_string(&manifest_file)
        .map_err(|e| AppError::msg(format!("无法读取插件清单：{e}")))?;
    if raw.len() > 512 * 1024 {
        return Err(AppError::msg("插件清单超过 512KB"));
    }
    let manifest: Manifest = serde_json::from_str(&raw)
        .map_err(|e| AppError::msg(format!("插件清单 JSON 无效：{e}")))?;
    if manifest.skills.is_empty() && manifest.mcp_servers.is_empty() {
        return Err(AppError::msg("插件至少需要提供 Skills 或 MCP 服务器"));
    }
    let id = Uuid::new_v4().to_string();
    let name = if manifest.name.trim().is_empty() {
        base.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin")
            .to_string()
    } else {
        manifest.name.trim().to_string()
    };
    let plugins_root = state.data_dir.join("plugins");
    std::fs::create_dir_all(&plugins_root)?;
    let target = plugins_root.join(&id);
    copy_tree(&base, &target)?;

    let mut installed_skills = Vec::new();
    let mut installed_servers = Vec::new();
    let result = (|| -> AppResult<()> {
        for relative in &manifest.skills {
            let dir = safe_join(&target, relative)?;
            let skill = crate::skills::install_from_path(state, &dir, Some(&id))?;
            installed_skills.push(skill.id);
        }
        for (server_name, config) in &manifest.mcp_servers {
            // Plugin manifests are copied out of their download folder, so any
            // relative script paths must be anchored to the copied plugin root.
            let command = {
                let candidate = target.join(&config.command);
                if Path::new(&config.command).is_relative() && candidate.is_file() {
                    candidate.to_string_lossy().to_string()
                } else {
                    config.command.clone()
                }
            };
            let args = config
                .args
                .iter()
                .map(|arg| {
                    if arg.starts_with('-') || Path::new(arg).is_absolute() {
                        return arg.clone();
                    }
                    let candidate = target.join(arg);
                    if candidate.exists() {
                        candidate.to_string_lossy().to_string()
                    } else {
                        arg.clone()
                    }
                })
                .collect::<Vec<_>>();
            let input = McpServerInput {
                id: None,
                name: server_name.clone(),
                transport: config.transport.clone(),
                command,
                args,
                env: if config.env.is_object() {
                    config.env.clone()
                } else {
                    json!({})
                },
                url: config.url.clone(),
                headers: if config.headers.is_object() {
                    config.headers.clone()
                } else {
                    json!({})
                },
                enabled: config.enabled,
                auto_approve: config.auto_approve,
            };
            let server = crate::mcp::save_server(state, &input, Some(&id))?;
            installed_servers.push(server.id);
        }
        let timestamp = now();
        {
            let conn = state.db();
            conn.execute(
                "INSERT INTO plugins(id, name, version, description, path, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    id,
                    name,
                    manifest.version,
                    manifest.description.chars().take(1000).collect::<String>(),
                    target.to_string_lossy(),
                    timestamp
                ],
            )?;
        }
        Ok(())
    })();

    if let Err(err) = result {
        for skill_id in &installed_skills {
            let _ = crate::skills::delete(state, skill_id);
        }
        for server_id in &installed_servers {
            let _ = crate::mcp::delete_server(state, server_id);
        }
        let _ = std::fs::remove_dir_all(&target);
        return Err(err);
    }
    let conn = state.db();
    conn.query_row(
        "SELECT p.*,
                (SELECT COUNT(*) FROM skills s WHERE s.plugin_id = p.id),
                (SELECT COUNT(*) FROM mcp_servers m WHERE m.plugin_id = p.id)
         FROM plugins p WHERE p.id = ?1",
        [&id],
        row_to_plugin,
    )
    .map_err(Into::into)
}

pub fn uninstall(state: &AppState, id: &str) -> AppResult<Vec<String>> {
    let path: Option<String> = {
        let conn = state.db();
        conn.query_row("SELECT path FROM plugins WHERE id = ?1", [id], |row| row.get(0))
            .ok()
    };
    let Some(path) = path else {
        return Err(AppError::msg("插件不存在"));
    };
    crate::skills::delete_for_plugin(state, id);
    let servers = crate::mcp::delete_for_plugin(state, id);
    {
        let conn = state.db();
        conn.execute("DELETE FROM plugins WHERE id = ?1", [id])?;
    }
    let plugins_root = state.data_dir.join("plugins");
    let dir = PathBuf::from(path);
    let canonical_root = plugins_root.canonicalize().unwrap_or(plugins_root);
    let canonical_dir = dir.canonicalize().unwrap_or_else(|_| dir.clone());
    if path_is_within(&canonical_root, &canonical_dir) && canonical_dir != canonical_root {
        let _ = std::fs::remove_dir_all(&canonical_dir);
    }
    Ok(servers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escaping_skill_paths() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        assert!(safe_join(&base, "../outside").is_err());
        assert!(safe_join(&base, "missing").is_err());
    }
}
