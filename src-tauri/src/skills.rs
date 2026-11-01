use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::Row;
use serde_json::json;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::db::now;
use crate::error::{AppError, AppResult};
use crate::files::{path_is_within, read_text, resolve_in_root, truncate_chars};
use crate::models::{Skill, ToolInfo};
use crate::providers::ToolSchema;
use crate::state::AppState;

const MAX_FILES: usize = 500;
const MAX_BYTES: u64 = 24 * 1024 * 1024;
const SKIP_DIRS: [&str; 6] = [".git", "node_modules", "target", "dist", "__pycache__", ".venv"];

fn is_heavy(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(|name| SKIP_DIRS.contains(&name))
            .unwrap_or(false)
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    let value = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(value);
    value.trim().to_string()
}

/// Parses the small YAML subset used by SKILL.md frontmatter. Supports plain
/// scalars plus folded (`>`) and literal (`|`) multi-line values.
pub fn parse_skill_md(text: &str) -> (BTreeMap<String, String>, String) {
    let normalized = text.replace("\r\n", "\n");
    let mut lines = normalized.lines();
    let mut meta = BTreeMap::new();
    if lines.next().map(str::trim) != Some("---") {
        return (meta, normalized.to_string());
    }
    let mut body_start = 0usize;
    let all: Vec<&str> = normalized.lines().collect();
    let mut index = 1usize;
    let mut closed = false;
    while index < all.len() {
        let line = all[index];
        if line.trim() == "---" {
            body_start = index + 1;
            closed = true;
            break;
        }
        if let Some((key, raw_value)) = line.split_once(':') {
            let key = key.trim().to_ascii_lowercase();
            let value = raw_value.trim();
            if value == ">" || value == "|" {
                let folded = value == ">";
                let mut parts: Vec<String> = Vec::new();
                index += 1;
                while index < all.len() {
                    let next = all[index];
                    if next.trim() == "---" {
                        break;
                    }
                    if !next.starts_with(' ') && !next.starts_with('\t') && !next.trim().is_empty() {
                        break;
                    }
                    parts.push(next.trim().to_string());
                    index += 1;
                }
                let joined = if folded {
                    parts.join(" ")
                } else {
                    parts.join("\n")
                };
                meta.insert(key, joined.trim().to_string());
                continue;
            }
            if !key.is_empty() && !key.starts_with('#') {
                meta.insert(key, unquote(value));
            }
        }
        index += 1;
    }
    if !closed {
        return (BTreeMap::new(), normalized);
    }
    let body = all
        .get(body_start..)
        .map(|lines| lines.join("\n"))
        .unwrap_or_default();
    (meta, body.trim_start_matches('\n').to_string())
}

fn fallback_description(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .unwrap_or("")
        .chars()
        .take(400)
        .collect()
}

fn copy_skill(source: &Path, target: &Path) -> AppResult<()> {
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut directories = Vec::new();
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_heavy(entry))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if entry.file_type().is_symlink() {
            continue;
        }
        let rel = path
            .strip_prefix(source)
            .map_err(|_| AppError::msg("Skill 路径无效"))?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = target.join(rel);
        if entry.file_type().is_dir() {
            directories.push(dest);
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        files += 1;
        bytes += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
        if files > MAX_FILES {
            return Err(AppError::msg(format!(
                "Skill 包含超过 {MAX_FILES} 个文件，已拒绝安装"
            )));
        }
        if bytes > MAX_BYTES {
            return Err(AppError::msg("Skill 总大小超过 24MB，已拒绝安装"));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, &dest)?;
    }
    Ok(())
}

pub fn install_from_path(
    state: &AppState,
    source: &Path,
    plugin_id: Option<&str>,
) -> AppResult<Skill> {
    let source = source
        .canonicalize()
        .map_err(|e| AppError::msg(format!("无法读取 Skill 路径：{e}")))?;
    let dir = if source.is_file() {
        if source.file_name().and_then(|name| name.to_str()) != Some("SKILL.md") {
            return Err(AppError::msg("请选择包含 SKILL.md 的文件夹或 SKILL.md 文件"));
        }
        source
            .parent()
            .ok_or_else(|| AppError::msg("Skill 路径无效"))?
            .to_path_buf()
    } else {
        source.clone()
    };
    let manifest = dir.join("SKILL.md");
    if !manifest.is_file() {
        return Err(AppError::msg("该文件夹缺少 SKILL.md"));
    }
    let raw = std::fs::read_to_string(&manifest)
        .map_err(|e| AppError::msg(format!("无法读取 SKILL.md：{e}")))?;
    if raw.len() > 512 * 1024 {
        return Err(AppError::msg("SKILL.md 超过 512KB"));
    }
    let (meta, body) = parse_skill_md(&raw);
    let folder_name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skill")
        .to_string();
    let name = meta
        .get("name")
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or(folder_name);
    if name.chars().count() > 80 {
        return Err(AppError::msg("Skill 名称不能超过 80 个字符"));
    }
    if !name
        .chars()
        .all(|ch| ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' '))
    {
        return Err(AppError::msg(
            "Skill 名称只能包含字母、数字、空格、点、横线和下划线",
        ));
    }
    let description = meta
        .get("description")
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| fallback_description(&body));
    if description.trim().is_empty() {
        return Err(AppError::msg("SKILL.md 需要 description 或正文说明"));
    }

    let id = Uuid::new_v4().to_string();
    let skills_root = state.data_dir.join("skills");
    std::fs::create_dir_all(&skills_root)?;
    let target = skills_root.join(&id);
    std::fs::create_dir_all(&target)?;
    if let Err(err) = copy_skill(&dir, &target) {
        let _ = std::fs::remove_dir_all(&target);
        return Err(err);
    }
    let timestamp = now();
    {
        let conn = state.db();
        conn.execute(
            "INSERT INTO skills(id, name, description, source, path, enabled, plugin_id, created_at, updated_at)
             VALUES(?1, ?2, ?3, 'user', ?4, 1, ?5, ?6, ?6)",
            rusqlite::params![
                id,
                name,
                description.chars().take(600).collect::<String>(),
                target.join("SKILL.md").to_string_lossy(),
                plugin_id,
                timestamp
            ],
        )?;
    }
    get(state, &id)
}

pub fn row_to_skill(row: &Row<'_>) -> rusqlite::Result<Skill> {
    Ok(Skill {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        source: row.get("source")?,
        path: row.get("path")?,
        enabled: row.get::<_, i64>("enabled")? != 0,
        plugin_id: row.get("plugin_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list(state: &AppState) -> AppResult<Vec<Skill>> {
    let conn = state.db();
    let mut stmt = conn.prepare("SELECT * FROM skills ORDER BY enabled DESC, name ASC")?;
    let rows = stmt.query_map([], row_to_skill)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get(state: &AppState, id: &str) -> AppResult<Skill> {
    let conn = state.db();
    conn.query_row("SELECT * FROM skills WHERE id = ?1", [id], row_to_skill)
        .map_err(|_| AppError::msg("Skill 不存在"))
}

pub fn set_enabled(state: &AppState, id: &str, enabled: bool) -> AppResult<Skill> {
    {
        let conn = state.db();
        let changed = conn.execute(
            "UPDATE skills SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, enabled as i64, now()],
        )?;
        if changed == 0 {
            return Err(AppError::msg("Skill 不存在"));
        }
    }
    get(state, id)
}

pub fn delete(state: &AppState, id: &str) -> AppResult<()> {
    let skill = get(state, id)?;
    let skills_root = state.data_dir.join("skills");
    let path = PathBuf::from(&skill.path);
    if let Some(dir) = path.parent() {
        let canonical_root = skills_root.canonicalize().unwrap_or(skills_root);
        let canonical_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        if path_is_within(&canonical_root, &canonical_dir) && canonical_dir != canonical_root {
            std::fs::remove_dir_all(&canonical_dir)?;
        }
    }
    let conn = state.db();
    conn.execute("DELETE FROM skills WHERE id = ?1", [id])?;
    Ok(())
}

pub fn delete_for_plugin(state: &AppState, plugin_id: &str) {
    let ids: Vec<String> = {
        let conn = state.db();
        let mut stmt = conn
            .prepare("SELECT id FROM skills WHERE plugin_id = ?1")
            .ok();
        let Some(stmt) = stmt.as_mut() else {
            return;
        };
        stmt.query_map([plugin_id], |row| row.get(0))
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    };
    for id in ids {
        let _ = delete(state, &id);
    }
}

pub fn enabled_count(state: &AppState) -> usize {
    let conn = state.db();
    conn.query_row(
        "SELECT COUNT(*) FROM skills WHERE enabled = 1",
        [],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0) as usize
}

pub fn prompt_section(state: &AppState) -> String {
    let skills = {
        let conn = state.db();
        let mut stmt = match conn.prepare(
            "SELECT name, description FROM skills WHERE enabled = 1 ORDER BY name ASC LIMIT 30",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return String::new(),
        };
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        });
        rows.map(|rows| rows.filter_map(Result::ok).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n可用 Skills（用户安装的可复用工作流）：\n\
         当任务与某个 Skill 的描述匹配时，先调用 read_skill 读取它的完整说明，再按说明执行。\n\
         Skill 目录中的脚本、模板和参考资料可通过 read_skill 的 file 参数读取。\n",
    );
    for (name, description) in skills {
        out.push_str(&format!(
            "- {name}: {}\n",
            description.chars().take(300).collect::<String>()
        ));
    }
    out
}

pub fn tool_schemas(state: &AppState) -> Vec<ToolSchema> {
    if enabled_count(state) == 0 {
        return Vec::new();
    }
    vec![ToolSchema {
        name: "read_skill".into(),
        description: "读取已安装 Skill 的完整说明（SKILL.md）。匹配到相关 Skill 时必须先调用。可用 file 参数读取该 Skill 目录内的脚本、模板或参考文件。".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Skill 名称"},
                "file": {"type": "string", "description": "可选：Skill 目录内的相对文件路径，默认读取 SKILL.md"}
            },
            "required": ["name"]
        }),
    }]
}

pub fn tool_infos(state: &AppState) -> Vec<ToolInfo> {
    if enabled_count(state) == 0 {
        return Vec::new();
    }
    vec![ToolInfo {
        name: "read_skill".into(),
        description: "加载 Skill 说明与附属文件".into(),
        source: "skill".into(),
        server: None,
        readonly: true,
        requires_approval: false,
    }]
}

pub fn read(
    state: &AppState,
    name: &str,
    file: Option<&str>,
) -> AppResult<(String, String, String, bool)> {
    let skill = {
        let conn = state.db();
        conn.query_row(
            "SELECT * FROM skills WHERE enabled = 1 AND (name = ?1 COLLATE NOCASE OR id = ?1) LIMIT 1",
            [name],
            row_to_skill,
        )
        .map_err(|_| AppError::msg(format!("Skill 不存在或未启用：{name}")))?
    };
    read_skill_file(&skill, file)
}

pub fn read_skill_file(
    skill: &Skill,
    file: Option<&str>,
) -> AppResult<(String, String, String, bool)> {
    let manifest = PathBuf::from(&skill.path);
    let dir = manifest
        .parent()
        .ok_or_else(|| AppError::msg("Skill 路径无效"))?
        .to_path_buf();
    let relative = file.unwrap_or("SKILL.md").trim();
    if relative.is_empty() {
        return Err(AppError::msg("file 不能为空"));
    }
    let target = resolve_in_root(&dir, relative)?;
    if !target.is_file() {
        return Err(AppError::msg(format!("Skill 文件不存在：{relative}")));
    }
    let (content, truncated_file, binary, _) = read_text(&target, 300_000)?;
    if binary {
        return Err(AppError::msg("该 Skill 文件是二进制文件，Agent 只能读取文本内容"));
    }
    let (content, truncated_chars) = truncate_chars(&content, 120_000);
    Ok((
        skill.name.clone(),
        content,
        dir.to_string_lossy().to_string(),
        truncated_file || truncated_chars,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let raw = "---\nname: pdf-tools\ndescription: >\n  Work with PDFs\n  safely.\n---\n\n# PDF\nUse the script.\n";
        let (meta, body) = parse_skill_md(raw);
        assert_eq!(meta.get("name").unwrap(), "pdf-tools");
        assert_eq!(meta.get("description").unwrap(), "Work with PDFs safely.");
        assert!(body.contains("Use the script."));
    }

    #[test]
    fn tolerates_documents_without_frontmatter() {
        let (meta, body) = parse_skill_md("# Title\nBody");
        assert!(meta.is_empty());
        assert!(body.contains("# Title"));
    }

    #[test]
    fn strips_quotes_from_values() {
        let (meta, _) = parse_skill_md("---\nname: \"quoted\"\ndescription: 'single'\n---\n");
        assert_eq!(meta.get("name").unwrap(), "quoted");
        assert_eq!(meta.get("description").unwrap(), "single");
    }

    #[test]
    fn reads_skill_files_without_escaping_directory() {
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        std::fs::write(root.join("SKILL.md"), "# demo").unwrap();
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets").join("notes.md"), "notes").unwrap();
        let skill = Skill {
            id: "skill-1".into(),
            name: "demo".into(),
            description: String::new(),
            source: "user".into(),
            path: root.join("SKILL.md").to_string_lossy().to_string(),
            enabled: true,
            plugin_id: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let (name, content, _, _) = read_skill_file(&skill, None).unwrap();
        assert_eq!(name, "demo");
        assert_eq!(content, "# demo");
        let (_, content, _, _) = read_skill_file(&skill, Some("assets/notes.md")).unwrap();
        assert_eq!(content, "notes");
        assert!(read_skill_file(&skill, Some("../secret")).is_err());
        assert!(read_skill_file(&skill, Some("missing.md")).is_err());
    }
}
