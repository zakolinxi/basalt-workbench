use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rusqlite::{params, Row};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::db::now;
use crate::error::{http_status_error, AppError, AppResult};
use crate::files::truncate_chars;
use crate::models::{McpServer, McpServerInput, McpTool, ToolInfo};
use crate::policy::PolicyDecision;
use crate::providers::ToolSchema;
use crate::state::AppState;

const PROTOCOL_VERSION: &str = "2025-06-18";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_TOOLS: usize = 300;
const MAX_PAGES: usize = 20;

pub type SharedSession = Arc<Mutex<McpSession>>;

enum Transport {
    Stdio {
        child: Child,
        stdin: BufWriter<ChildStdin>,
        stdout: BufReader<ChildStdout>,
    },
    Http {
        session_id: Option<String>,
    },
}

pub struct McpSession {
    server_id: String,
    url: String,
    headers: HashMap<String, String>,
    next_id: u64,
    transport: Transport,
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let Transport::Stdio { child, .. } = &mut self.transport else {
            return;
        };
        #[cfg(windows)]
        {
            if let Some(pid) = child.id() {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .creation_flags(0x08000000)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
        let _ = child.start_kill();
    }
}

fn string_map(value: &Value) -> HashMap<String, String> {
    value
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn validate_input(input: &McpServerInput) -> AppResult<()> {
    if input.name.trim().is_empty() {
        return Err(AppError::msg("MCP 服务器名称不能为空"));
    }
    if input.name.chars().count() > 80 {
        return Err(AppError::msg("MCP 服务器名称不能超过 80 个字符"));
    }
    if input.args.len() > 64 {
        return Err(AppError::msg("MCP 参数过多"));
    }
    if !matches!(input.transport.as_str(), "stdio" | "http") {
        return Err(AppError::msg("MCP 传输方式必须是 stdio 或 http"));
    }
    if input.transport == "stdio" {
        if input.command.trim().is_empty() {
            return Err(AppError::msg("stdio 模式需要启动命令"));
        }
        if input.command.chars().count() > 1000 {
            return Err(AppError::msg("启动命令过长"));
        }
    } else {
        let parsed = url::Url::parse(input.url.trim())
            .map_err(|e| AppError::msg(format!("MCP URL 无效：{e}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::msg("MCP URL 必须使用 http 或 https"));
        }
    }
    for value in [&input.env, &input.headers] {
        if !value.is_object() {
            return Err(AppError::msg("环境变量与请求头必须是 JSON 对象"));
        }
        if value.as_object().is_some_and(|map| {
            map.values().any(|item| !item.is_string()) || map.len() > 64
        }) {
            return Err(AppError::msg("环境变量与请求头的值必须是字符串"));
        }
    }
    Ok(())
}

pub fn row_to_server(row: &Row<'_>) -> rusqlite::Result<McpServer> {
    let args_raw: String = row.get("args_json")?;
    let env_raw: String = row.get("env_json")?;
    let headers_raw: String = row.get("headers_json")?;
    Ok(McpServer {
        id: row.get("id")?,
        name: row.get("name")?,
        transport: row.get("transport")?,
        command: row.get("command")?,
        args: serde_json::from_str(&args_raw).unwrap_or_default(),
        env: serde_json::from_str(&env_raw).unwrap_or_else(|_| json!({})),
        url: row.get("url")?,
        headers: serde_json::from_str(&headers_raw).unwrap_or_else(|_| json!({})),
        enabled: row.get::<_, i64>("enabled")? != 0,
        auto_approve: row.get::<_, i64>("auto_approve")? != 0,
        status: row.get("status")?,
        last_error: row.get("last_error")?,
        last_refreshed_at: row.get("last_refreshed_at")?,
        plugin_id: row.get("plugin_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_servers(state: &AppState) -> AppResult<Vec<McpServer>> {
    let conn = state.db();
    let mut stmt = conn.prepare("SELECT * FROM mcp_servers ORDER BY name ASC")?;
    let rows = stmt.query_map([], row_to_server)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_server(state: &AppState, id: &str) -> AppResult<McpServer> {
    let conn = state.db();
    conn.query_row("SELECT * FROM mcp_servers WHERE id = ?1", [id], row_to_server)
        .map_err(|_| AppError::msg("MCP 服务器不存在"))
}

pub fn save_server(
    state: &AppState,
    input: &McpServerInput,
    plugin_id: Option<&str>,
) -> AppResult<McpServer> {
    validate_input(input)?;
    let id = input
        .id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = now();
    let args_json = serde_json::to_string(&input.args)?;
    let env = if input.env.is_object() {
        input.env.clone()
    } else {
        json!({})
    };
    let headers = if input.headers.is_object() {
        input.headers.clone()
    } else {
        json!({})
    };
    {
        let conn = state.db();
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM mcp_servers WHERE id = ?1",
            [&id],
            |row| row.get(0),
        )?;
        if exists > 0 {
            conn.execute(
                "UPDATE mcp_servers SET name = ?2, transport = ?3, command = ?4, args_json = ?5,
                 env_json = ?6, url = ?7, headers_json = ?8, enabled = ?9, auto_approve = ?10,
                 status = 'idle', last_error = NULL, updated_at = ?11 WHERE id = ?1",
                params![
                    id,
                    input.name.trim(),
                    input.transport,
                    input.command.trim(),
                    args_json,
                    serde_json::to_string(&env)?,
                    input.url.trim(),
                    serde_json::to_string(&headers)?,
                    input.enabled as i64,
                    input.auto_approve as i64,
                    timestamp
                ],
            )?;
            conn.execute("DELETE FROM mcp_tools WHERE server_id = ?1", [&id])?;
        } else {
            conn.execute(
                "INSERT INTO mcp_servers(id, name, transport, command, args_json, env_json, url,
                 headers_json, enabled, auto_approve, status, plugin_id, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'idle', ?11, ?12, ?12)",
                params![
                    id,
                    input.name.trim(),
                    input.transport,
                    input.command.trim(),
                    args_json,
                    serde_json::to_string(&env)?,
                    input.url.trim(),
                    serde_json::to_string(&headers)?,
                    input.enabled as i64,
                    input.auto_approve as i64,
                    plugin_id,
                    timestamp
                ],
            )?;
        }
    }
    get_server(state, &id)
}

pub fn set_enabled(state: &AppState, id: &str, enabled: bool) -> AppResult<McpServer> {
    {
        let conn = state.db();
        let changed = conn.execute(
            "UPDATE mcp_servers SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, enabled as i64, now()],
        )?;
        if changed == 0 {
            return Err(AppError::msg("MCP 服务器不存在"));
        }
    }
    get_server(state, id)
}

pub fn delete_server(state: &AppState, id: &str) -> AppResult<()> {
    let conn = state.db();
    conn.execute("DELETE FROM mcp_tools WHERE server_id = ?1", [id])?;
    let changed = conn.execute("DELETE FROM mcp_servers WHERE id = ?1", [id])?;
    if changed == 0 {
        return Err(AppError::msg("MCP 服务器不存在"));
    }
    Ok(())
}

pub fn delete_for_plugin(state: &AppState, plugin_id: &str) -> Vec<String> {
    let ids: Vec<String> = {
        let conn = state.db();
        let mut stmt = conn
            .prepare("SELECT id FROM mcp_servers WHERE plugin_id = ?1")
            .ok();
        let Some(stmt) = stmt.as_mut() else {
            return Vec::new();
        };
        stmt.query_map([plugin_id], |row| row.get(0))
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    };
    for id in &ids {
        let _ = delete_server(state, id);
    }
    ids
}

pub fn row_to_tool(row: &Row<'_>, server_name: &str) -> rusqlite::Result<McpTool> {
    let schema_raw: String = row.get("input_schema")?;
    let original: String = row.get("name")?;
    let server_id: String = row.get("server_id")?;
    Ok(McpTool {
        name: tool_full_name(&server_id, &original),
        tool_name: original,
        server_id,
        server_name: server_name.to_string(),
        description: row.get("description")?,
        input_schema: serde_json::from_str(&schema_raw).unwrap_or_else(|_| json!({})),
        readonly: row.get::<_, i64>("readonly")? != 0,
        refreshed_at: row.get("refreshed_at")?,
    })
}

pub fn list_tools(state: &AppState, server_id: &str) -> AppResult<Vec<McpTool>> {
    let server = get_server(state, server_id)?;
    let conn = state.db();
    let mut stmt = conn.prepare(
        "SELECT server_id, name, description, input_schema, readonly, refreshed_at
         FROM mcp_tools WHERE server_id = ?1 ORDER BY name ASC",
    )?;
    let rows = stmt.query_map([server_id], |row| row_to_tool(row, &server.name))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn all_tools(state: &AppState) -> Vec<McpTool> {
    let servers = list_servers(state).unwrap_or_default();
    let mut out = Vec::new();
    for server in servers.iter().filter(|server| server.enabled) {
        if let Ok(tools) = list_tools(state, &server.id) {
            out.extend(tools);
        }
    }
    out
}

fn sanitize_tool_name(value: &str) -> String {
    let mut out: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        out.push_str("tool");
    }
    out
}

pub fn tool_full_name(server_id: &str, tool: &str) -> String {
    let short: String = server_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(6)
        .collect();
    let mut name = format!("mcp_{}_{}", short, sanitize_tool_name(tool));
    if name.chars().count() > 64 {
        name = name.chars().take(64).collect();
    }
    name
}

pub fn is_mcp_tool(name: &str) -> bool {
    name.starts_with("mcp_")
}

pub fn find_tool(state: &AppState, full_name: &str) -> Option<(McpServer, McpTool)> {
    for server in list_servers(state).ok()?.into_iter().filter(|s| s.enabled) {
        if let Ok(tools) = list_tools(state, &server.id) {
            if let Some(tool) = tools.into_iter().find(|tool| tool.name == full_name) {
                return Some((server, tool));
            }
        }
    }
    None
}

pub fn tool_schemas(state: &AppState) -> Vec<ToolSchema> {
    all_tools(state)
        .into_iter()
        .map(|tool| {
            let description = format!(
                "[MCP · {}] {}",
                tool.server_name,
                if tool.description.trim().is_empty() {
                    &tool.tool_name
                } else {
                    &tool.description
                }
            );
            let parameters = if tool.input_schema.is_object() {
                tool.input_schema
            } else {
                json!({"type": "object", "properties": {}})
            };
            ToolSchema {
                name: tool.name,
                description,
                parameters,
            }
        })
        .collect()
}

pub fn tool_infos(state: &AppState) -> Vec<ToolInfo> {
    all_tools(state)
        .into_iter()
        .map(|tool| {
            let server = list_servers(state)
                .unwrap_or_default()
                .into_iter()
                .find(|server| server.id == tool.server_id);
            ToolInfo {
                name: tool.name,
                description: tool.description,
                source: "mcp".into(),
                server: Some(tool.server_name),
                readonly: tool.readonly,
                requires_approval: server.map(|server| !server.auto_approve).unwrap_or(true),
            }
        })
        .collect()
}

pub fn decision(state: &AppState, full_name: &str) -> PolicyDecision {
    let Some((server, tool)) = find_tool(state, full_name) else {
        return PolicyDecision::Deny(format!("MCP 工具不存在或服务器未启用：{full_name}"));
    };
    if server.auto_approve {
        PolicyDecision::Allow
    } else {
        PolicyDecision::NeedsApproval(format!(
            "将调用 MCP 工具：{} / {}",
            server.name, tool.tool_name
        ))
    }
}

fn command_line(server: &McpServer) -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        let mut all = Vec::with_capacity(server.args.len() + 2);
        all.push("/C".to_string());
        all.push(server.command.clone());
        all.extend(server.args.iter().cloned());
        ("cmd".to_string(), all)
    }
    #[cfg(not(windows))]
    {
        (server.command.clone(), server.args.clone())
    }
}

fn rpc_error(value: &Value) -> Option<AppError> {
    let error = value.get("error")?;
    let code = error
        .get("code")
        .and_then(|value| value.as_i64())
        .unwrap_or_default();
    let message = error
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("未知错误");
    Some(AppError::msg(format!("MCP 错误 {code}：{message}")))
}

pub fn parse_sse_events(body: &str) -> Vec<Value> {
    let mut events = Vec::new();
    let mut data = String::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            if !data.trim().is_empty() {
                if let Ok(value) = serde_json::from_str::<Value>(data.trim()) {
                    events.push(value);
                }
                data.clear();
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data.push_str(rest.trim_start());
            data.push('\n');
        }
    }
    if !data.trim().is_empty() {
        if let Ok(value) = serde_json::from_str::<Value>(data.trim()) {
            events.push(value);
        }
    }
    events
}

async fn http_post(
    state: &AppState,
    server: &McpServer,
    payload: &Value,
    session_id: Option<&str>,
) -> AppResult<reqwest::Response> {
    let mut builder = state
        .http
        .post(server.url.trim())
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", PROTOCOL_VERSION)
        .json(payload);
    if let Some(session_id) = session_id {
        builder = builder.header("mcp-session-id", session_id);
    }
    for (key, value) in string_map(&server.headers) {
        builder = builder.header(key, value);
    }
    builder.send().await.map_err(AppError::from)
}

async fn parse_http_result(
    response: reqwest::Response,
    expected_id: Option<u64>,
) -> AppResult<Value> {
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(http_status_error(status, &body));
    }
    let values = if content_type.contains("text/event-stream") {
        parse_sse_events(&body)
    } else if body.trim().is_empty() {
        Vec::new()
    } else {
        vec![serde_json::from_str::<Value>(&body)
            .map_err(|e| AppError::msg(format!("MCP 返回了无法解析的 JSON：{e}")))?]
    };
    if let Some(expected_id) = expected_id {
        for value in &values {
            let matches = value
                .get("id")
                .and_then(|id| id.as_u64())
                .map(|id| id == expected_id)
                .unwrap_or(false);
            if matches {
                if let Some(error) = rpc_error(value) {
                    return Err(error);
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
        }
        return Err(AppError::msg("MCP 响应中没有匹配的请求 ID"));
    }
    Ok(Value::Null)
}

impl McpSession {
    async fn send_notification(
        &mut self,
        state: &AppState,
        method: &str,
        params: Value,
    ) -> AppResult<()> {
        let payload = json!({"jsonrpc": "2.0", "method": method, "params": params});
        match &mut self.transport {
            Transport::Stdio { stdin, .. } => {
                let mut line = serde_json::to_vec(&payload)?;
                line.push(b'\n');
                stdin.write_all(&line).await?;
                stdin.flush().await?;
                Ok(())
            }
            Transport::Http { session_id } => {
                let server = synthetic_server(&self.server_id, &self.url, &self.headers);
                let response = http_post(state, &server, &payload, session_id.as_deref()).await?;
                let status = response.status();
                if !status.is_success() {
                    let body = response.text().await.unwrap_or_default();
                    return Err(http_status_error(status, &body));
                }
                Ok(())
            }
        }
    }

    async fn request(
        &mut self,
        state: &AppState,
        method: &str,
        params: Value,
        cancel: &CancellationToken,
    ) -> AppResult<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let payload = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        match &mut self.transport {
            Transport::Stdio { stdin, stdout, .. } => {
                let mut line = serde_json::to_vec(&payload)?;
                line.push(b'\n');
                stdin.write_all(&line).await?;
                stdin.flush().await?;
                loop {
                    let mut response_line = String::new();
                    let read = tokio::select! {
                        _ = cancel.cancelled() => return Err(AppError::Cancelled),
                        result = tokio::time::timeout(REQUEST_TIMEOUT, stdout.read_line(&mut response_line)) => result,
                    };
                    let read = read.map_err(|_| {
                        AppError::msg("MCP 服务器响应超时")
                    })??;
                    if read == 0 {
                        return Err(AppError::Io(std::io::Error::other(
                            "MCP 服务器已关闭连接",
                        )));
                    }
                    if response_line.len() > 16 * 1024 * 1024 {
                        return Err(AppError::msg("MCP 响应过大，已中止"));
                    }
                    let Ok(value) = serde_json::from_str::<Value>(response_line.trim()) else {
                        continue;
                    };
                    let matches = value
                        .get("id")
                        .and_then(|value| value.as_u64())
                        .map(|value| value == id)
                        .unwrap_or(false);
                    if !matches {
                        continue;
                    }
                    if let Some(error) = rpc_error(&value) {
                        return Err(error);
                    }
                    return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                }
            }
            Transport::Http { session_id } => {
                let mut server = synthetic_server(&self.server_id, &self.url, &self.headers);
                server.transport = "http".into();
                let response = tokio::select! {
                    _ = cancel.cancelled() => return Err(AppError::Cancelled),
                    result = http_post(state, &server, &payload, session_id.as_deref()) => result?,
                };
                if let Some(value) = response.headers().get("mcp-session-id") {
                    if let Ok(value) = value.to_str() {
                        *session_id = Some(value.to_string());
                    }
                }
                parse_http_result(response, Some(id)).await
            }
        }
    }
}

fn synthetic_server(id: &str, url: &str, headers: &HashMap<String, String>) -> McpServer {
    McpServer {
        id: id.to_string(),
        name: id.to_string(),
        transport: "http".into(),
        command: String::new(),
        args: Vec::new(),
        env: json!({}),
        url: url.to_string(),
        headers: json!(headers),
        enabled: true,
        auto_approve: false,
        status: String::new(),
        last_error: None,
        last_refreshed_at: None,
        plugin_id: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

async fn connect(state: &AppState, server: &McpServer) -> AppResult<McpSession> {
    let params = json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {},
        "clientInfo": {
            "name": "Basalt",
            "version": env!("CARGO_PKG_VERSION")
        }
    });
    let mut session = if server.transport == "stdio" {
        let (program, args) = command_line(server);
        let mut command = tokio::process::Command::new(&program);
        command.args(&args);
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        for (key, value) in string_map(&server.env) {
            command.env(key, value);
        }
        if let Ok(cwd) = std::env::current_dir() {
            command.current_dir(cwd);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.as_std_mut().creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| AppError::msg(format!("无法启动 MCP 服务器：{e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::msg("无法连接 MCP 标准输入"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::msg("无法连接 MCP 标准输出"))?;
        if let Some(stderr) = child.stderr.take() {
            drain_stderr(stderr, state.data_dir.clone(), server.name.clone());
        }
        McpSession {
            server_id: server.id.clone(),
            url: String::new(),
            headers: HashMap::new(),
            next_id: 0,
            transport: Transport::Stdio {
                child,
                stdin: BufWriter::new(stdin),
                stdout: BufReader::new(stdout),
            },
        }
    } else {
        McpSession {
            server_id: server.id.clone(),
            url: server.url.trim().to_string(),
            headers: string_map(&server.headers),
            next_id: 0,
            transport: Transport::Http { session_id: None },
        }
    };

    // For HTTP the initialize response may carry the session id. Reuse the
    // generic request path and capture it in the transport state.
    let cancel = CancellationToken::new();
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        session.request(state, "initialize", params, &cancel),
    )
    .await
    .map_err(|_| AppError::msg("MCP 初始化超时"))??;
    if result.get("protocolVersion").is_none() && !result.is_null() {
        // Some servers return a minimal result; a non-error response is enough.
    }
    let _ = session
        .send_notification(state, "notifications/initialized", json!({}))
        .await;
    Ok(session)
}

fn drain_stderr(
    stderr: tokio::process::ChildStderr,
    data_dir: PathBuf,
    server_name: String,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let redacted = crate::error::redact(&line);
            let preview: String = redacted.chars().take(500).collect();
            crate::logging::log_line(
                &data_dir,
                "MCP",
                &format!("[{server_name}] {preview}"),
            );
        }
    });
}

pub async fn ensure_session(state: &AppState, server: &McpServer) -> AppResult<SharedSession> {
    if !server.enabled {
        return Err(AppError::msg("MCP 服务器已禁用"));
    }
    {
        let sessions = state.mcp.lock().await;
        if let Some(session) = sessions.get(&server.id) {
            return Ok(session.clone());
        }
    }
    let session = Arc::new(Mutex::new(connect(state, server).await?));
    let mut sessions = state.mcp.lock().await;
    let entry = sessions
        .entry(server.id.clone())
        .or_insert_with(|| session.clone())
        .clone();
    Ok(entry)
}

pub async fn disconnect(state: &AppState, server_id: &str) {
    let removed = {
        let mut sessions = state.mcp.lock().await;
        sessions.remove(server_id)
    };
    drop(removed);
}

pub async fn disconnect_all(state: &AppState) {
    let removed = {
        let mut sessions = state.mcp.lock().await;
        std::mem::take(&mut *sessions)
    };
    drop(removed);
}

pub async fn refresh_tools(state: &AppState, server_id: &str) -> AppResult<Vec<McpTool>> {
    let server = get_server(state, server_id)?;
    if !server.enabled {
        return Err(AppError::msg("请先启用 MCP 服务器"));
    }
    {
        let conn = state.db();
        let _ = conn.execute(
            "UPDATE mcp_servers SET status = 'connecting', updated_at = ?2 WHERE id = ?1",
            params![server_id, now()],
        );
    }
    let result = refresh_inner(state, &server).await;
    match result {
        Ok(tools) => {
            {
                let conn = state.db();
                let tx = conn.unchecked_transaction()?;
                tx.execute("DELETE FROM mcp_tools WHERE server_id = ?1", [server_id])?;
                let timestamp = now();
                for tool in &tools {
                    tx.execute(
                        "INSERT OR REPLACE INTO mcp_tools(server_id, name, description, input_schema,
                         readonly, refreshed_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            server_id,
                            tool.tool_name,
                            tool.description,
                            tool.input_schema.to_string(),
                            tool.readonly as i64,
                            timestamp
                        ],
                    )?;
                }
                tx.execute(
                    "UPDATE mcp_servers SET status = 'ready', last_error = NULL,
                     last_refreshed_at = ?2, updated_at = ?2 WHERE id = ?1",
                    params![server_id, timestamp],
                )?;
                tx.commit()?;
            }
            list_tools(state, server_id)
        }
        Err(err) => {
            let message = err.user_message();
            {
                let conn = state.db();
                let _ = conn.execute(
                    "UPDATE mcp_servers SET status = 'error', last_error = ?2, updated_at = ?3 WHERE id = ?1",
                    params![server_id, message.chars().take(1000).collect::<String>(), now()],
                );
            }
            Err(err)
        }
    }
}

async fn refresh_inner(state: &AppState, server: &McpServer) -> AppResult<Vec<McpTool>> {
    let session = ensure_session(state, server).await?;
    let mut guard = session.lock().await;
    let cancel = CancellationToken::new();
    let mut tools = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let params = cursor
            .as_ref()
            .map(|cursor| json!({"cursor": cursor}))
            .unwrap_or_else(|| json!({}));
        let result = guard.request(state, "tools/list", params, &cancel).await?;
        let Some(items) = result.get("tools").and_then(|value| value.as_array()) else {
            break;
        };
        for tool in items {
            let Some(name) = tool.get("name").and_then(|value| value.as_str()) else {
                continue;
            };
            let description = tool
                .get("description")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .chars()
                .take(4000)
                .collect::<String>();
            let input_schema = tool
                .get("inputSchema")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
            let readonly = tool
                .get("annotations")
                .and_then(|value| value.get("readOnlyHint"))
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            tools.push(McpTool {
                server_id: server.id.clone(),
                server_name: server.name.clone(),
                name: tool_full_name(&server.id, name),
                tool_name: name.to_string(),
                description,
                input_schema,
                readonly,
                refreshed_at: now(),
            });
            if tools.len() >= MAX_TOOLS {
                break;
            }
        }
        cursor = result
            .get("nextCursor")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .filter(|value| !value.is_empty());
        if cursor.is_none() || tools.len() >= MAX_TOOLS {
            break;
        }
    }
    Ok(tools)
}

pub async fn call_tool(
    state: &AppState,
    full_name: &str,
    arguments: &Value,
    cancel: &CancellationToken,
) -> AppResult<(Value, String)> {
    let Some((server, tool)) = find_tool(state, full_name) else {
        return Err(AppError::msg(format!("MCP 工具不存在：{full_name}")));
    };
    let session = ensure_session(state, &server).await?;
    let result = {
        let mut guard = session.lock().await;
        guard
            .request(
                state,
                "tools/call",
                json!({"name": tool.tool_name, "arguments": arguments}),
                cancel,
            )
            .await
    };
    let result = match result {
        Ok(result) => result,
        Err(err) => {
            if matches!(err, AppError::Io(_) | AppError::Http(_)) {
                disconnect(state, &server.id).await;
            }
            return Err(err);
        }
    };
    let is_error = result
        .get("isError")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let mut content = Vec::new();
    let mut text = String::new();
    if let Some(items) = result.get("content").and_then(|value| value.as_array()) {
        for item in items.iter().take(80) {
            match item.get("type").and_then(|value| value.as_str()) {
                Some("text") => {
                    let value = item
                        .get("text")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(value);
                    content.push(json!({"type": "text", "text": value}));
                }
                Some("image") => {
                    let mime = item
                        .get("mimeType")
                        .and_then(|value| value.as_str())
                        .unwrap_or("image");
                    content.push(json!({"type": "image", "mimeType": mime, "note": "图片内容未注入上下文"}));
                }
                Some("audio") => {
                    content.push(json!({"type": "audio", "note": "音频内容未注入上下文"}));
                }
                Some("resource") => {
                    let resource = item.get("resource").cloned().unwrap_or(Value::Null);
                    if let Some(value) = resource.get("text").and_then(|value| value.as_str()) {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(value);
                    }
                    content.push(json!({"type": "resource", "resource": resource}));
                }
                _ => content.push(item.clone()),
            }
        }
    }
    let (text, truncated) = truncate_chars(&text, 60_000);
    let data = json!({
        "server": server.name,
        "tool": tool.tool_name,
        "isError": is_error,
        "text": text,
        "content": content,
        "structuredContent": result.get("structuredContent").cloned().unwrap_or(Value::Null),
        "truncated": truncated
    });
    let summary = if is_error {
        format!("MCP 工具返回错误：{} / {}", server.name, tool.tool_name)
    } else {
        format!("MCP 完成：{} / {}", server.name, tool.tool_name)
    };
    Ok((data, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_and_prefixes_tool_names() {
        assert_eq!(
            tool_full_name("12345678-abcd", "read/file name"),
            "mcp_123456_read_file_name"
        );
        let long = tool_full_name("abcdef", &"x".repeat(120));
        assert_eq!(long.chars().count(), 64);
    }

    #[test]
    fn parses_sse_messages() {
        let body = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n\n";
        let events = parse_sse_events(body);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["result"]["ok"], true);
    }

    #[test]
    fn rejects_invalid_server_input() {
        let mut input = McpServerInput {
            id: None,
            name: "x".into(),
            transport: "stdio".into(),
            command: String::new(),
            args: Vec::new(),
            env: json!({}),
            url: String::new(),
            headers: json!({}),
            enabled: true,
            auto_approve: false,
        };
        assert!(validate_input(&input).is_err());
        input.command = "npx".into();
        assert!(validate_input(&input).is_ok());
        input.transport = "http".into();
        input.url = "ftp://x".into();
        assert!(validate_input(&input).is_err());
    }
}
