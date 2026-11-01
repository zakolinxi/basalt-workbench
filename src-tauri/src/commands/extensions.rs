use std::path::Path;

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::models::{InstalledPlugin, McpServer, McpServerInput, McpTool, Skill, ToolInfo};
use crate::state::AppState;

#[tauri::command]
pub fn skill_list(state: State<'_, AppState>) -> AppResult<Vec<Skill>> {
    crate::skills::list(&state)
}

#[tauri::command]
pub fn skill_install(state: State<'_, AppState>, path: String) -> AppResult<Skill> {
    crate::skills::install_from_path(&state, Path::new(&path), None)
}

#[tauri::command]
pub fn skill_set_enabled(
    state: State<'_, AppState>,
    skill_id: String,
    enabled: bool,
) -> AppResult<Skill> {
    crate::skills::set_enabled(&state, &skill_id, enabled)
}

#[tauri::command]
pub fn skill_delete(state: State<'_, AppState>, skill_id: String) -> AppResult<()> {
    crate::skills::delete(&state, &skill_id)
}

#[tauri::command]
pub fn mcp_server_list(state: State<'_, AppState>) -> AppResult<Vec<McpServer>> {
    crate::mcp::list_servers(&state)
}

#[tauri::command]
pub async fn mcp_server_save(
    state: State<'_, AppState>,
    input: McpServerInput,
) -> AppResult<McpServer> {
    let server = crate::mcp::save_server(&state, &input, None)?;
    crate::mcp::disconnect(&state, &server.id).await;
    Ok(server)
}

#[tauri::command]
pub async fn mcp_server_delete(
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<()> {
    crate::mcp::disconnect(&state, &server_id).await;
    crate::mcp::delete_server(&state, &server_id)
}

#[tauri::command]
pub async fn mcp_server_set_enabled(
    state: State<'_, AppState>,
    server_id: String,
    enabled: bool,
) -> AppResult<McpServer> {
    let server = crate::mcp::set_enabled(&state, &server_id, enabled)?;
    if !enabled {
        crate::mcp::disconnect(&state, &server_id).await;
    }
    Ok(server)
}

#[tauri::command]
pub async fn mcp_server_refresh(
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<McpTool>> {
    crate::mcp::refresh_tools(&state, &server_id).await
}

#[tauri::command]
pub fn mcp_tool_list(
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<McpTool>> {
    crate::mcp::list_tools(&state, &server_id)
}

#[tauri::command]
pub async fn mcp_server_test(
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<McpTool>> {
    crate::mcp::refresh_tools(&state, &server_id).await
}

#[tauri::command]
pub async fn mcp_tool_call(
    state: State<'_, AppState>,
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> AppResult<serde_json::Value> {
    let server = crate::mcp::get_server(&state, &server_id)?;
    if !server.enabled {
        return Err(AppError::msg("请先启用 MCP 服务器"));
    }
    let full_name = crate::mcp::tool_full_name(&server.id, &tool_name);
    let cancel = tokio_util::sync::CancellationToken::new();
    let (data, summary) = crate::mcp::call_tool(&state, &full_name, &arguments, &cancel).await?;
    Ok(serde_json::json!({"data": data, "summary": summary}))
}

#[tauri::command]
pub fn tool_registry(state: State<'_, AppState>) -> AppResult<Vec<ToolInfo>> {
    let mut tools = crate::agent::tools::tool_infos();
    tools.extend(crate::skills::tool_infos(&state));
    tools.extend(crate::mcp::tool_infos(&state));
    Ok(tools)
}

#[tauri::command]
pub fn plugin_list(state: State<'_, AppState>) -> AppResult<Vec<InstalledPlugin>> {
    crate::plugins::list(&state)
}

#[tauri::command]
pub async fn plugin_install(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<InstalledPlugin> {
    if !Path::new(&path).exists() {
        return Err(AppError::msg("插件路径不存在"));
    }
    let plugin = crate::plugins::install(&state, Path::new(&path))?;
    let servers = crate::mcp::list_servers(&state)?
        .into_iter()
        .filter(|server| server.plugin_id.as_deref() == Some(plugin.id.as_str()))
        .collect::<Vec<_>>();
    for server in servers {
        let _ = crate::mcp::refresh_tools(&state, &server.id).await;
    }
    Ok(plugin)
}

#[tauri::command]
pub async fn plugin_uninstall(
    state: State<'_, AppState>,
    plugin_id: String,
) -> AppResult<()> {
    let servers = crate::plugins::uninstall(&state, &plugin_id)?;
    for server_id in servers {
        crate::mcp::disconnect(&state, &server_id).await;
    }
    Ok(())
}
