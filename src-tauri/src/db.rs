use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde_json::json;

use crate::error::AppResult;
use crate::models::*;

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn open_db(data_dir: &Path) -> AppResult<Connection> {
    let conn = Connection::open(data_dir.join("basalt.db"))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))?;
    conn.execute_batch(SCHEMA)?;
    migrate(&conn)?;
    seed(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> AppResult<()> {
    for (table, column, ddl) in [
        ("sessions", "reasoning_effort", "ALTER TABLE sessions ADD COLUMN reasoning_effort TEXT"),
        ("tasks", "reasoning_effort", "ALTER TABLE tasks ADD COLUMN reasoning_effort TEXT"),
    ] {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
            params![table, column],
            |row| row.get(0),
        )?;
        if exists == 0 {
            conn.execute(ddl, [])?;
        }
    }
    Ok(())
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  path TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  last_opened_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS providers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  category TEXT NOT NULL,
  protocol TEXT NOT NULL,
  base_url TEXT NOT NULL DEFAULT '',
  models_json TEXT NOT NULL DEFAULT '[]',
  default_chat_model TEXT,
  default_vision_model TEXT,
  default_image_model TEXT,
  headers_json TEXT NOT NULL DEFAULT '{}',
  params_json TEXT NOT NULL DEFAULT '{}',
  timeout_secs INTEGER NOT NULL DEFAULT 120,
  stream INTEGER NOT NULL DEFAULT 1,
  enabled INTEGER NOT NULL DEFAULT 1,
  is_template INTEGER NOT NULL DEFAULT 0,
  last_status TEXT,
  last_error TEXT,
  last_tested_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  project_id TEXT,
  title TEXT NOT NULL,
  provider_id TEXT,
  model TEXT,
  mode TEXT NOT NULL DEFAULT 'chat',
  reasoning_effort TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'text',
  attachments_json TEXT NOT NULL DEFAULT '[]',
  task_id TEXT,
  model TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, created_at);
CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  title TEXT NOT NULL,
  goal TEXT NOT NULL,
  status TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  model TEXT NOT NULL,
  reasoning_effort TEXT,
  plan_json TEXT NOT NULL DEFAULT '[]',
  report TEXT,
  error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS task_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_task_events_task ON task_events(task_id, id);
CREATE TABLE IF NOT EXISTS tool_calls (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  name TEXT NOT NULL,
  args_json TEXT NOT NULL,
  result_json TEXT,
  status TEXT NOT NULL,
  requires_approval INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  completed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_tool_calls_task ON tool_calls(task_id, created_at);
CREATE TABLE IF NOT EXISTS changes (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  project_id TEXT NOT NULL,
  path TEXT NOT NULL,
  old_exists INTEGER NOT NULL,
  new_exists INTEGER NOT NULL,
  old_hash TEXT,
  new_hash TEXT,
  snapshot_path TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_changes_task ON changes(task_id, created_at);
CREATE TABLE IF NOT EXISTS image_generations (
  id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,
  provider_name TEXT NOT NULL,
  model TEXT NOT NULL,
  mode TEXT NOT NULL,
  prompt TEXT NOT NULL,
  params_json TEXT NOT NULL DEFAULT '{}',
  files_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL,
  error TEXT,
  session_id TEXT,
  created_at TEXT NOT NULL
);
"#;

fn seed(conn: &Connection) -> AppResult<()> {
    let seeded: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = 'seeded'", [], |r| r.get(0))
        .ok();
    if seeded.is_none() {
        let settings = Settings::default();
        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES('app', ?1)",
            [serde_json::to_string(&settings)?],
        )?;

        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES('seeded', '1')",
            [],
        )?;
    }
    purge_mock_data(conn)?;
    seed_templates(conn)?;
    Ok(())
}

fn purge_mock_data(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT id FROM providers WHERE id LIKE 'builtin-mock-%' OR protocol IN ('mock', 'mock_images')",
    )?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(Result::ok)
        .collect();
    if !ids.is_empty() {
        conn.execute(
            "DELETE FROM providers WHERE id LIKE 'builtin-mock-%' OR protocol IN ('mock', 'mock_images')",
            [],
        )?;
    }
    let mut settings = load_settings(conn);
    let mut changed = false;
    let routes = [
        &mut settings.routing.chat,
        &mut settings.routing.agent_plan,
        &mut settings.routing.agent_execute,
        &mut settings.routing.review,
        &mut settings.routing.vision,
        &mut settings.routing.image,
        &mut settings.routing.image_edit,
    ];
    for route in routes {
        if route.provider_id.is_empty() {
            continue;
        }
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM providers WHERE id = ?1",
                [&route.provider_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if ids.iter().any(|id| id == &route.provider_id)
            || route.provider_id.starts_with("builtin-mock-")
            || exists == 0
        {
            route.provider_id.clear();
            route.model.clear();
            changed = true;
        }
    }
    if changed {
        save_settings(conn, &settings)?;
    }
    Ok(())
}

const TEMPLATES: [(&str, &str, &str, &str, &str); 17] = [
    ("tpl-openai-chat", "OpenAI Chat Completions", "chat", "openai_chat", "https://api.openai.com/v1"),
    ("tpl-openai-responses", "OpenAI Responses", "chat", "openai_responses", "https://api.openai.com/v1"),
    ("tpl-anthropic", "Anthropic Claude", "chat", "anthropic", "https://api.anthropic.com/v1"),
    ("tpl-gemini", "Google Gemini", "chat", "gemini", "https://generativelanguage.googleapis.com/v1beta"),
    ("tpl-deepseek", "DeepSeek", "chat", "openai_chat", "https://api.deepseek.com/v1"),
    ("tpl-minimax", "MiniMax", "chat", "openai_chat", "https://api.minimaxi.com/v1"),
    ("tpl-moonshot", "Moonshot Kimi", "chat", "openai_chat", "https://api.moonshot.cn/v1"),
    ("tpl-qwen", "阿里云百炼 Qwen", "chat", "openai_chat", "https://dashscope.aliyuncs.com/compatible-mode/v1"),
    ("tpl-glm", "智谱 GLM", "chat", "openai_chat", "https://open.bigmodel.cn/api/paas/v4"),
    ("tpl-grok", "xAI Grok", "chat", "openai_chat", "https://api.x.ai/v1"),
    ("tpl-openai-compat", "OpenAI 兼容接口", "chat", "openai_chat", ""),
    ("tpl-gateway", "自建模型网关", "chat", "openai_chat", ""),
    ("tpl-ollama", "Ollama 本地模型", "chat", "ollama", "http://127.0.0.1:11434"),
    ("tpl-openai-images", "OpenAI 图像生成", "image", "openai_images", "https://api.openai.com/v1"),
    ("tpl-gemini-images", "Gemini 图像生成", "image", "gemini_images", "https://generativelanguage.googleapis.com/v1beta"),
    ("tpl-openai-compat-images", "OpenAI 兼容图像接口", "image", "openai_images", ""),
    ("tpl-gateway-images", "自建图像网关", "image", "openai_images", ""),
];

fn preset_model(
    id: &str,
    name: &str,
    context_length: u64,
    vision: bool,
    tools: bool,
    reasoning: bool,
) -> ProviderModel {
    let mut capabilities = ModelCapabilities::chat_baseline(true);
    capabilities.stream = CAP_UNVERIFIED.into();
    capabilities.tools = if tools { CAP_UNVERIFIED } else { CAP_UNSUPPORTED }.into();
    capabilities.vision = if vision { CAP_UNVERIFIED } else { CAP_UNSUPPORTED }.into();
    capabilities.reasoning = if reasoning { CAP_UNVERIFIED } else { CAP_UNSUPPORTED }.into();
    capabilities.multi_tool = capabilities.tools.clone();
    ProviderModel {
        id: id.into(),
        name: name.into(),
        context_length: Some(context_length),
        capabilities,
        source: "preset".into(),
    }
}

fn template_models(template_id: &str) -> Vec<ProviderModel> {
    match template_id {
        "tpl-openai-chat" => vec![
            // GPT-6.1 Sol 的工具调用只在 Responses API 提供。
            preset_model("gpt-6.1-sol", "GPT-6.1 Sol", 1_050_000, true, false, true),
            preset_model("gpt-6-astra", "GPT-6 Astra", 1_050_000, true, true, true),
            preset_model("gpt-5.6-sol", "GPT-5.6 Sol", 1_050_000, true, true, true),
            preset_model("gpt-4.1", "GPT-4.1", 1_047_576, true, true, false),
        ],
        "tpl-openai-responses" => vec![
            preset_model("gpt-6.1-sol", "GPT-6.1 Sol", 1_050_000, true, true, true),
            preset_model("gpt-6-astra", "GPT-6 Astra", 1_050_000, true, true, true),
            preset_model("gpt-5.6-sol", "GPT-5.6 Sol", 1_050_000, true, true, true),
            preset_model("gpt-4.1", "GPT-4.1", 1_047_576, true, true, false),
        ],
        "tpl-anthropic" => vec![
            preset_model("claude-opus-5-5", "Claude Opus 5.5", 1_000_000, true, true, true),
            preset_model("claude-sonnet-5-5", "Claude Sonnet 5.5", 1_000_000, true, true, true),
            preset_model("claude-haiku-5-5", "Claude Haiku 5.5", 1_000_000, true, true, true),
            preset_model("claude-fable-5-1", "Claude Fable 5.1", 1_000_000, true, true, true),
        ],
        "tpl-gemini" => vec![
            preset_model("gemini-4-argon", "Gemini 4 Argon", 1_048_576, true, true, true),
            preset_model("gemini-3.8-flash", "Gemini 3.8 Flash", 1_048_576, true, true, true),
            preset_model("gemini-3.1-pro-preview", "Gemini 3.1 Pro", 1_000_000, true, true, true),
        ],
        "tpl-deepseek" => vec![
            preset_model("deepseek-v4-pro", "DeepSeek V4 Pro", 1_000_000, false, true, true),
            preset_model("deepseek-flash", "DeepSeek V4.1 Flash", 1_000_000, false, true, true),
        ],
        "tpl-minimax" => vec![
            preset_model("MiniMax-M3.1-Flash-Preview", "MiniMax M3.1 Flash", 1_000_000, true, true, true),
            preset_model("MiniMax-M3", "MiniMax M3", 1_000_000, true, true, true),
            preset_model("MiniMax-M2.7", "MiniMax M2.7", 204_800, false, true, true),
        ],
        "tpl-moonshot" => vec![
            preset_model("kimi-k3.1-pro", "Kimi K3.1 Pro", 1_048_576, true, true, true),
            preset_model("kimi-k3.1-pro-highspeed", "Kimi K3.1 Pro 高速版", 1_048_576, true, true, true),
        ],
        "tpl-qwen" => vec![
            preset_model("qwen3.8-max", "Qwen3.8 Max", 1_000_000, true, true, true),
            preset_model("qwen3.8-flash", "Qwen3.8 Flash", 1_000_000, true, true, true),
        ],
        "tpl-glm" => vec![
            preset_model("glm-5.3", "GLM-5.3", 1_000_000, false, true, true),
            preset_model("glm-5.3-flash", "GLM-5.3 Flash", 1_000_000, true, true, true),
        ],
        "tpl-grok" => vec![
            preset_model("grok-4.7", "Grok 4.7", 500_000, true, true, true),
            preset_model("grok-4.6", "Grok 4.6", 500_000, true, true, true),
        ],
        "tpl-ollama" => vec![
            preset_model("qwen3.8:27b", "Qwen3.8 27B（本地）", 262_144, true, true, true),
            preset_model("deepseek-v4-flash", "DeepSeek V4 Flash（本地）", 1_000_000, false, true, true),
        ],
        _ => Vec::new(),
    }
}

fn seed_templates(conn: &Connection) -> AppResult<()> {
    let ts = now();
    for (id, name, category, protocol, base_url) in TEMPLATES {
        conn.execute(
            "INSERT OR IGNORE INTO providers(id, name, category, protocol, base_url, models_json,
             default_chat_model, headers_json, params_json, timeout_secs, stream, enabled,
             is_template, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, '[]', NULL, '{}', '{}', 180, 1, 0, 1, ?6, ?6)",
            params![id, name, category, protocol, base_url, ts],
        )?;
        let models = template_models(id);
        if models.is_empty() {
            continue;
        }
        let models_json = serde_json::to_string(&models)?;
        let current: Option<String> = conn
            .query_row(
                "SELECT models_json FROM providers WHERE id = ?1 AND is_template = 1",
                [id],
                |row| row.get(0),
            )
            .ok();
        if current.as_deref() == Some(models_json.as_str()) {
            continue;
        }
        let default_chat = if category == "chat" {
            models.first().map(|model| model.id.clone())
        } else {
            None
        };
        conn.execute(
            "UPDATE providers SET models_json = ?2, default_chat_model = ?3, updated_at = ?4
             WHERE id = ?1 AND is_template = 1",
            params![id, models_json, default_chat, ts],
        )?;
    }
    Ok(())
}

pub fn mark_interrupted_tasks(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE status IN ('running', 'waitingApproval', 'queued')",
        params![TASK_INTERRUPTED, now()],
    )?;
    conn.execute(
        "UPDATE tool_calls SET status = 'cancelled', completed_at = ?1
         WHERE status IN ('pending', 'waiting_approval', 'running')",
        params![now()],
    )?;
    Ok(())
}

pub fn load_settings(conn: &Connection) -> Settings {
    conn.query_row("SELECT value FROM settings WHERE key = 'app'", [], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_settings(conn: &Connection, settings: &Settings) -> AppResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings(key, value) VALUES('app', ?1)",
        [serde_json::to_string(settings)?],
    )?;
    Ok(())
}

pub fn set_setting_raw(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings(key, value) VALUES(?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub fn row_to_provider(row: &Row<'_>) -> rusqlite::Result<Provider> {
    let models_raw: String = row.get("models_json")?;
    let headers_raw: String = row.get("headers_json")?;
    let params_raw: String = row.get("params_json")?;
    Ok(Provider {
        id: row.get("id")?,
        name: row.get("name")?,
        category: row.get("category")?,
        protocol: row.get("protocol")?,
        base_url: row.get("base_url")?,
        has_key: false,
        models: serde_json::from_str(&models_raw).unwrap_or_default(),
        default_chat_model: row.get("default_chat_model")?,
        default_vision_model: row.get("default_vision_model")?,
        default_image_model: row.get("default_image_model")?,
        headers: serde_json::from_str(&headers_raw).unwrap_or_else(|_| json!({})),
        params: serde_json::from_str(&params_raw).unwrap_or_else(|_| json!({})),
        timeout_secs: row.get::<_, i64>("timeout_secs")? as u64,
        stream: row.get::<_, i64>("stream")? != 0,
        enabled: row.get::<_, i64>("enabled")? != 0,
        is_template: row.get::<_, i64>("is_template")? != 0,
        last_status: row.get("last_status")?,
        last_error: row.get("last_error")?,
        last_tested_at: row.get("last_tested_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_provider_rows(conn: &Connection) -> AppResult<Vec<Provider>> {
    let mut stmt = conn.prepare("SELECT * FROM providers ORDER BY is_template ASC, category ASC, name ASC")?;
    let rows = stmt.query_map([], row_to_provider)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get_provider_row(conn: &Connection, id: &str) -> AppResult<Provider> {
    conn.query_row("SELECT * FROM providers WHERE id = ?1", [id], row_to_provider)
        .map_err(|_| crate::error::AppError::msg(format!("供应商不存在：{id}")))
}

pub fn row_to_project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        name: row.get("name")?,
        path: row.get("path")?,
        created_at: row.get("created_at")?,
        last_opened_at: row.get("last_opened_at")?,
    })
}

pub fn row_to_session(row: &Row<'_>) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        title: row.get("title")?,
        provider_id: row.get("provider_id")?,
        model: row.get("model")?,
        mode: row.get("mode")?,
        reasoning_effort: row.get("reasoning_effort")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn row_to_message(row: &Row<'_>) -> rusqlite::Result<Message> {
    let attachments_raw: String = row.get("attachments_json")?;
    Ok(Message {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        role: row.get("role")?,
        content: row.get("content")?,
        kind: row.get("kind")?,
        attachments: serde_json::from_str(&attachments_raw).unwrap_or_default(),
        task_id: row.get("task_id")?,
        model: row.get("model")?,
        created_at: row.get("created_at")?,
    })
}

pub fn row_to_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    let plan_raw: String = row.get("plan_json")?;
    Ok(Task {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        session_id: row.get("session_id")?,
        title: row.get("title")?,
        goal: row.get("goal")?,
        status: row.get("status")?,
        provider_id: row.get("provider_id")?,
        model: row.get("model")?,
        reasoning_effort: row.get("reasoning_effort")?,
        plan: serde_json::from_str(&plan_raw).unwrap_or_default(),
        report: row.get("report")?,
        error: row.get("error")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn row_to_change(row: &Row<'_>) -> rusqlite::Result<Change> {
    Ok(Change {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        project_id: row.get("project_id")?,
        path: row.get("path")?,
        old_exists: row.get::<_, i64>("old_exists")? != 0,
        new_exists: row.get::<_, i64>("new_exists")? != 0,
        old_hash: row.get("old_hash")?,
        new_hash: row.get("new_hash")?,
        status: row.get("status")?,
        created_at: row.get("created_at")?,
    })
}

pub fn row_to_tool_call(row: &Row<'_>) -> rusqlite::Result<ToolCall> {
    let args_raw: String = row.get("args_json")?;
    let result_raw: Option<String> = row.get("result_json")?;
    Ok(ToolCall {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        name: row.get("name")?,
        args: serde_json::from_str(&args_raw).unwrap_or(json!({})),
        result: result_raw.and_then(|r| serde_json::from_str(&r).ok()),
        status: row.get("status")?,
        requires_approval: row.get::<_, i64>("requires_approval")? != 0,
        created_at: row.get("created_at")?,
        completed_at: row.get("completed_at")?,
    })
}

pub fn row_to_image_generation(row: &Row<'_>) -> rusqlite::Result<ImageGeneration> {
    let params_raw: String = row.get("params_json")?;
    let files_raw: String = row.get("files_json")?;
    Ok(ImageGeneration {
        id: row.get("id")?,
        provider_id: row.get("provider_id")?,
        provider_name: row.get("provider_name")?,
        model: row.get("model")?,
        mode: row.get("mode")?,
        prompt: row.get("prompt")?,
        params: serde_json::from_str(&params_raw).unwrap_or(json!({})),
        files: serde_json::from_str(&files_raw).unwrap_or_default(),
        status: row.get("status")?,
        error: row.get("error")?,
        session_id: row.get("session_id")?,
        created_at: row.get("created_at")?,
    })
}

pub fn insert_message(
    conn: &Connection,
    id: &str,
    session_id: &str,
    role: &str,
    content: &str,
    kind: &str,
    attachments: &[Attachment],
    task_id: Option<&str>,
    model: Option<&str>,
) -> AppResult<Message> {
    let created_at = now();
    conn.execute(
        "INSERT INTO messages(id, session_id, role, content, kind, attachments_json, task_id, model, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            session_id,
            role,
            content,
            kind,
            serde_json::to_string(attachments)?,
            task_id,
            model,
            created_at
        ],
    )?;
    conn.execute(
        "UPDATE sessions SET updated_at = ?2 WHERE id = ?1",
        params![session_id, created_at],
    )?;
    Ok(Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        kind: kind.to_string(),
        attachments: attachments.to_vec(),
        task_id: task_id.map(str::to_string),
        model: model.map(str::to_string),
        created_at,
    })
}

pub fn get_task(conn: &Connection, id: &str) -> AppResult<Task> {
    conn.query_row("SELECT * FROM tasks WHERE id = ?1", [id], row_to_task)
        .map_err(|_| crate::error::AppError::msg(format!("任务不存在：{id}")))
}

pub fn get_project(conn: &Connection, id: &str) -> AppResult<Project> {
    conn.query_row("SELECT * FROM projects WHERE id = ?1", [id], row_to_project)
        .map_err(|_| crate::error::AppError::msg(format!("项目不存在：{id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_templates_without_mock() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        let providers = list_provider_rows(&conn).unwrap();
        assert!(!providers
            .iter()
            .any(|p| p.id.starts_with("builtin-mock-") || p.protocol.starts_with("mock")));
        assert!(providers.iter().any(|p| p.is_template));
        let settings = load_settings(&conn);
        assert_eq!(settings.agent_max_iterations, 20);
        assert!(settings.routing.chat.provider_id.is_empty());
        assert!(settings.routing.image.provider_id.is_empty());
    }

    #[test]
    fn seeds_template_model_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        let providers = list_provider_rows(&conn).unwrap();
        let responses = providers
            .iter()
            .find(|p| p.id == "tpl-openai-responses")
            .unwrap();
        let sol = responses
            .models
            .iter()
            .find(|m| m.id == "gpt-6.1-sol")
            .unwrap();
        assert_eq!(sol.context_length, Some(1_050_000));
        assert_eq!(sol.capabilities.reasoning, CAP_UNVERIFIED);
        let minimax = providers
            .iter()
            .find(|p| p.id == "tpl-minimax")
            .unwrap();
        assert!(minimax
            .models
            .iter()
            .any(|m| m.id == "MiniMax-M3.1-Flash-Preview"));
        let deepseek = providers
            .iter()
            .find(|p| p.id == "tpl-deepseek")
            .unwrap();
        assert!(deepseek
            .models
            .iter()
            .all(|m| m.capabilities.vision == CAP_UNSUPPORTED));
    }

    #[test]
    fn purges_mock_providers_and_routes() {
        let dir = tempfile::tempdir().unwrap();
        {
            let conn = open_db(dir.path()).unwrap();
            conn.execute(
                "INSERT INTO providers(id, name, category, protocol, base_url, created_at, updated_at)
                 VALUES('old-mock', 'Old Mock', 'chat', 'mock', '', ?1, ?1)",
                [now()],
            )
            .unwrap();
            let mut settings = load_settings(&conn);
            settings.routing.chat = Route {
                provider_id: "old-mock".into(),
                model: "mock-chat".into(),
            };
            save_settings(&conn, &settings).unwrap();
        }
        let conn = open_db(dir.path()).unwrap();
        assert!(!list_provider_rows(&conn)
            .unwrap()
            .iter()
            .any(|p| p.id == "old-mock"));
        let settings = load_settings(&conn);
        assert!(settings.routing.chat.provider_id.is_empty());
        assert!(settings.routing.chat.model.is_empty());
    }

    #[test]
    fn persists_settings_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        let mut settings = load_settings(&conn);
        settings.auto_approve_writes = true;
        settings.agent_max_iterations = 33;
        save_settings(&conn, &settings).unwrap();
        let loaded = load_settings(&conn);
        assert!(loaded.auto_approve_writes);
        assert_eq!(loaded.agent_max_iterations, 33);
    }
}
