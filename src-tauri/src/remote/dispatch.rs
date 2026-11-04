//! RPC dispatch for the remote gateway.
//!
//! Every Tauri command exposed to the desktop UI is forwarded here so a paired
//! phone can drive the exact same backend code. Tauri converts the JavaScript
//! argument object's top-level keys from camelCase to snake_case before calling
//! a command; nested objects are left untouched because the input structs
//! themselves use `#[serde(rename_all = "camelCase")]`. `p()` reproduces that
//! first half of the contract, so the frontend can send its argument object
//! through untouched.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::commands;
use crate::error::AppResult;
use crate::state::AppState;

fn p<T: DeserializeOwned>(args: &Value, keys: &[&str]) -> Result<T, String> {
    match keys.iter().find_map(|key| args.get(*key)).cloned() {
        Some(value) => serde_json::from_value(value)
            .map_err(|error| format!("参数 {} 无效：{error}", keys[0])),
        // Missing values are handed to the deserializer so `Option<T>` still
        // accepts them while required types fail with a readable message.
        None => serde_json::from_value(Value::Null).map_err(|_| format!("缺少参数：{}", keys[0])),
    }
}

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("结果序列化失败：{error}"))
}

fn wrap<T: Serialize>(result: AppResult<T>) -> Result<Value, String> {
    result.map_err(|error| error.user_message()).and_then(ok)
}

/// Entry point used by the gateway. Commands that genuinely need to await
/// network IO run on the async runtime; everything else is moved to the
/// blocking pool so SQLite and file work never stalls the HTTP workers.
pub async fn call(app: &AppHandle, cmd: &str, args: Value) -> Result<Value, String> {
    let args = if args.is_null() {
        Value::Object(Default::default())
    } else {
        args
    };
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "应用尚未初始化完成".to_string())?;

    match cmd {
        "provider_test" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            wrap(commands::providers::provider_test(state, provider_id).await)
        }
        "provider_balance" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            wrap(commands::balance::provider_balance(state, provider_id).await)
        }
        "provider_detect_model" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            let model: String = p(&args, &["model"])?;
            wrap(commands::providers::provider_detect_model(state, provider_id, model).await)
        }
        "chat_send" => {
            let input = p(&args, &["input"])?;
            wrap(commands::chat::chat_send(state, input).await)
        }
        "review_task" => {
            let task_id: String = p(&args, &["taskId", "task_id"])?;
            wrap(commands::tasks::review_task(state, task_id).await)
        }
        "image_generate" => {
            let input = p(&args, &["input"])?;
            wrap(commands::images::image_generate(state, input).await)
        }
        "chat_image_generate" => {
            let session_id: String = p(&args, &["sessionId", "session_id"])?;
            let prompt: String = p(&args, &["prompt"])?;
            wrap(commands::images::chat_image_generate(state, session_id, prompt).await)
        }
        "mcp_server_save" => {
            let input = p(&args, &["input"])?;
            wrap(commands::extensions::mcp_server_save(state, input).await)
        }
        "mcp_server_delete" => {
            let server_id: String = p(&args, &["serverId", "server_id"])?;
            wrap(commands::extensions::mcp_server_delete(state, server_id).await)
        }
        "mcp_server_set_enabled" => {
            let server_id: String = p(&args, &["serverId", "server_id"])?;
            let enabled: bool = p(&args, &["enabled"])?;
            wrap(commands::extensions::mcp_server_set_enabled(state, server_id, enabled).await)
        }
        "mcp_server_refresh" => {
            let server_id: String = p(&args, &["serverId", "server_id"])?;
            wrap(commands::extensions::mcp_server_refresh(state, server_id).await)
        }
        "mcp_server_test" => {
            let server_id: String = p(&args, &["serverId", "server_id"])?;
            wrap(commands::extensions::mcp_server_test(state, server_id).await)
        }
        "mcp_tool_call" => {
            let server_id: String = p(&args, &["serverId", "server_id"])?;
            let tool_name: String = p(&args, &["toolName", "tool_name"])?;
            let arguments: Value = p(&args, &["arguments"])?;
            wrap(
                commands::extensions::mcp_tool_call(state, server_id, tool_name, arguments).await,
            )
        }
        "plugin_install" => {
            let path: String = p(&args, &["path"])?;
            wrap(commands::extensions::plugin_install(state, path).await)
        }
        "plugin_uninstall" => {
            let plugin_id: String = p(&args, &["pluginId", "plugin_id"])?;
            wrap(commands::extensions::plugin_uninstall(state, plugin_id).await)
        }
        _ => {
            let app = app.clone();
            let cmd = cmd.to_string();
            tokio::task::spawn_blocking(move || call_sync(&app, &cmd, args))
                .await
                .map_err(|error| format!("远程调用失败：{error}"))?
        }
    }
}

/// Dispatches one synchronous remote call. Runs on the blocking pool because
/// the underlying commands touch SQLite and the file system.
fn call_sync(app: &AppHandle, cmd: &str, args: Value) -> Result<Value, String> {
    let args = if args.is_null() { Value::Object(Default::default()) } else { args };
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "应用尚未初始化完成".to_string())?;

    match cmd {
        // ---- misc -------------------------------------------------------
        "browse_directories" => {
            let path = p(&args, &["path"])?;
            wrap(commands::browse::browse_directories(path))
        }
        "bootstrap" => wrap(commands::misc::bootstrap(state)),
        "settings_get" => wrap(commands::misc::settings_get(state)),
        "settings_set" => {
            let settings = p(&args, &["settings"])?;
            wrap(commands::misc::settings_set(state, settings))
        }
        "resolve_approval" => {
            let call_id: String = p(&args, &["callId", "call_id"])?;
            let approved: bool = p(&args, &["approved"])?;
            wrap(commands::misc::resolve_approval(state, call_id, approved))
        }
        "open_path" => {
            let path: String = p(&args, &["path"])?;
            wrap(commands::misc::open_path(state, path))
        }
        "app_info" => wrap(commands::misc::app_info(state)),
        "session_message_count" => {
            let session_id: String = p(&args, &["sessionId", "session_id"])?;
            wrap(commands::misc::session_message_count(state, session_id))
        }

        // ---- providers --------------------------------------------------
        "provider_list" => wrap(commands::providers::provider_list(state)),
        "provider_get" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            wrap(commands::providers::provider_get(state, provider_id))
        }
        "provider_create" => {
            let input = p(&args, &["input"])?;
            wrap(commands::providers::provider_create(state, input))
        }
        "provider_update" => {
            let input = p(&args, &["input"])?;
            wrap(commands::providers::provider_update(state, input))
        }
        "provider_delete" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            wrap(commands::providers::provider_delete(state, provider_id))
        }
        "provider_duplicate" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            wrap(commands::providers::provider_duplicate(state, provider_id))
        }
        "provider_model_endpoints" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            wrap(commands::providers::provider_model_endpoints(state, provider_id))
        }
        "provider_set_enabled" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            let enabled: bool = p(&args, &["enabled"])?;
            wrap(commands::providers::provider_set_enabled(state, provider_id, enabled))
        }
        "provider_set_capability" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            let model_id: String = p(&args, &["modelId", "model_id"])?;
            let capability: String = p(&args, &["capability"])?;
            let value: String = p(&args, &["value"])?;
            wrap(commands::providers::provider_set_capability(
                state,
                provider_id,
                model_id,
                capability,
                value,
            ))
        }
        "provider_models_from_discovered" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            let models = p(&args, &["models"])?;
            wrap(commands::providers::provider_models_from_discovered(
                state,
                provider_id,
                models,
            ))
        }

        // ---- projects / files / git -------------------------------------
        "project_list" => wrap(commands::projects::project_list(state)),
        "project_add" => {
            let input = p(&args, &["input"])?;
            wrap(commands::projects::project_add(state, input))
        }
        "project_open" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            wrap(commands::projects::project_open(state, project_id))
        }
        "project_remove" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            wrap(commands::projects::project_remove(state, project_id))
        }
        "project_list_dir" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            let path: String = p(&args, &["path"])?;
            wrap(commands::files::project_list_dir(state, project_id, path))
        }
        "project_read_file" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            let path: String = p(&args, &["path"])?;
            wrap(commands::files::project_read_file(state, project_id, path))
        }
        "project_write_file" => {
            let input = p(&args, &["input"])?;
            wrap(commands::files::project_write_file(state, input))
        }
        "git_status" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            wrap(commands::files::git_status(state, project_id))
        }
        "git_branches" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            wrap(commands::files::git_branches(state, project_id))
        }
        "git_checkout_branch" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            let branch: String = p(&args, &["branch"])?;
            wrap(commands::files::git_checkout_branch(state, project_id, branch))
        }
        "git_create_branch" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            let branch: String = p(&args, &["branch"])?;
            wrap(commands::files::git_create_branch(state, project_id, branch))
        }
        "git_file_diff" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            let path: String = p(&args, &["path"])?;
            wrap(commands::files::git_file_diff(state, project_id, path))
        }
        "git_commit" => {
            let input = p(&args, &["input"])?;
            wrap(commands::files::git_commit(state, input))
        }
        "git_restore_file" => {
            let project_id: String = p(&args, &["projectId", "project_id"])?;
            let path: String = p(&args, &["path"])?;
            wrap(commands::files::git_restore_file(state, project_id, path))
        }

        // ---- sessions ---------------------------------------------------
        "session_list" => wrap(commands::sessions::session_list(state)),
        "session_create" => {
            let input = p(&args, &["input"])?;
            wrap(commands::sessions::session_create(state, input))
        }
        "session_update" => {
            let session_id: String = p(&args, &["sessionId", "session_id"])?;
            let title = p(&args, &["title"])?;
            let provider_id = p(&args, &["providerId", "provider_id"])?;
            let model = p(&args, &["model"])?;
            let reasoning_effort = p(&args, &["reasoningEffort", "reasoning_effort"])?;
            wrap(commands::sessions::session_update(
                state,
                session_id,
                title,
                provider_id,
                model,
                reasoning_effort,
            ))
        }
        "session_delete" => {
            let session_id: String = p(&args, &["sessionId", "session_id"])?;
            wrap(commands::sessions::session_delete(state, session_id))
        }
        "message_list" => {
            let session_id: String = p(&args, &["sessionId", "session_id"])?;
            wrap(commands::sessions::message_list(state, session_id))
        }

        // ---- tasks ------------------------------------------------------
        "task_create" => {
            let input = p(&args, &["input"])?;
            wrap(commands::tasks::task_create(state, input))
        }
        "task_list" => wrap(commands::tasks::task_list(state)),
        "task_get" => {
            let task_id: String = p(&args, &["taskId", "task_id"])?;
            wrap(commands::tasks::task_get(state, task_id))
        }
        "task_start" => {
            let task_id: String = p(&args, &["taskId", "task_id"])?;
            wrap(commands::tasks::task_start(app.clone(), state, task_id))
        }
        "task_cancel" => {
            let task_id: String = p(&args, &["taskId", "task_id"])?;
            wrap(commands::tasks::task_cancel(state, task_id))
        }
        "task_delete" => {
            let task_id: String = p(&args, &["taskId", "task_id"])?;
            wrap(commands::tasks::task_delete(state, task_id))
        }

        // ---- changes ----------------------------------------------------
        "change_list" => {
            let task_id = p(&args, &["taskId", "task_id"])?;
            let project_id = p(&args, &["projectId", "project_id"])?;
            wrap(commands::changes::change_list(state, task_id, project_id))
        }
        "change_diff_cmd" => {
            let change_id: String = p(&args, &["changeId", "change_id"])?;
            wrap(commands::changes::change_diff_cmd(state, change_id))
        }
        "change_resolve" => {
            let input = p(&args, &["input"])?;
            wrap(commands::changes::change_resolve(state, input))
        }

        // ---- chat -------------------------------------------------------
        "attachment_import" => {
            let path: String = p(&args, &["path"])?;
            wrap(commands::chat::attachment_import(state, path))
        }
        "attachment_import_bytes" => {
            let name: String = p(&args, &["name"])?;
            let mime = p(&args, &["mime"])?;
            let data_base64: String = p(&args, &["dataBase64", "data_base64"])?;
            wrap(commands::chat::attachment_import_bytes(
                state,
                name,
                mime,
                data_base64,
            ))
        }
        "read_media" => {
            let path: String = p(&args, &["path"])?;
            wrap(commands::chat::read_media(state, path))
        }
        "attachment_delete" => {
            let path: String = p(&args, &["path"])?;
            wrap(commands::chat::attachment_delete(state, path))
        }
        "chat_cancel" => {
            let request_id: String = p(&args, &["requestId", "request_id"])?;
            wrap(commands::chat::chat_cancel(state, request_id))
        }
        "context_usage" => {
            let session_id: String = p(&args, &["sessionId", "session_id"])?;
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            let model: String = p(&args, &["model"])?;
            let mode: String = p(&args, &["mode"])?;
            wrap(commands::chat::context_usage(
                state,
                session_id,
                provider_id,
                model,
                mode,
            ))
        }
        "usage_stats" => {
            let session_id: String = p(&args, &["sessionId", "session_id"])?;
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            let model: String = p(&args, &["model"])?;
            wrap(commands::chat::usage_stats(state, session_id, provider_id, model))
        }
        "run_metrics" => {
            let session_id: String = p(&args, &["sessionId", "session_id"])?;
            let task_id = p(&args, &["taskId", "task_id"])?;
            wrap(commands::chat::run_metrics(state, session_id, task_id))
        }

        // ---- terminal ---------------------------------------------------
        "terminal_create" => {
            let project_id = p(&args, &["projectId", "project_id"])?;
            let shell = p(&args, &["shell"])?;
            let cols = p(&args, &["cols"])?;
            let rows = p(&args, &["rows"])?;
            wrap(commands::terminal::terminal_create(
                state, project_id, shell, cols, rows,
            ))
        }
        "terminal_write" => {
            let terminal_id: String = p(&args, &["terminalId", "terminal_id"])?;
            let data: String = p(&args, &["data"])?;
            wrap(commands::terminal::terminal_write(state, terminal_id, data))
        }
        "terminal_resize" => {
            let terminal_id: String = p(&args, &["terminalId", "terminal_id"])?;
            let cols: u16 = p(&args, &["cols"])?;
            let rows: u16 = p(&args, &["rows"])?;
            wrap(commands::terminal::terminal_resize(
                state,
                terminal_id,
                cols,
                rows,
            ))
        }
        "terminal_kill" => {
            let terminal_id: String = p(&args, &["terminalId", "terminal_id"])?;
            wrap(commands::terminal::terminal_kill(state, terminal_id))
        }

        // ---- images -----------------------------------------------------
        "image_capabilities" => {
            let provider_id: String = p(&args, &["providerId", "provider_id"])?;
            let model: String = p(&args, &["model"])?;
            wrap(commands::images::image_capabilities(state, provider_id, model))
        }
        "image_history" => {
            let session_id = p(&args, &["sessionId", "session_id"])?;
            let limit = p(&args, &["limit"])?;
            wrap(commands::images::image_history(state, session_id, limit))
        }
        "image_delete" => {
            let generation_id: String = p(&args, &["generationId", "generation_id"])?;
            wrap(commands::images::image_delete(state, generation_id))
        }
        "image_save_as" => {
            let source_path: String = p(&args, &["sourcePath", "source_path"])?;
            let target_path: String = p(&args, &["targetPath", "target_path"])?;
            wrap(commands::images::image_save_as(state, source_path, target_path))
        }
        // ---- extensions -------------------------------------------------
        "skill_list" => wrap(commands::extensions::skill_list(state)),
        "skill_install" => {
            let path: String = p(&args, &["path"])?;
            wrap(commands::extensions::skill_install(state, path))
        }
        "skill_set_enabled" => {
            let skill_id: String = p(&args, &["skillId", "skill_id"])?;
            let enabled: bool = p(&args, &["enabled"])?;
            wrap(commands::extensions::skill_set_enabled(state, skill_id, enabled))
        }
        "skill_delete" => {
            let skill_id: String = p(&args, &["skillId", "skill_id"])?;
            wrap(commands::extensions::skill_delete(state, skill_id))
        }
        "mcp_server_list" => wrap(commands::extensions::mcp_server_list(state)),
        "mcp_tool_list" => {
            let server_id: String = p(&args, &["serverId", "server_id"])?;
            wrap(commands::extensions::mcp_tool_list(state, server_id))
        }
        "tool_registry" => wrap(commands::extensions::tool_registry(state)),
        "plugin_list" => wrap(commands::extensions::plugin_list(state)),

        other => Err(format!("未知命令：{other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Compound {
        project_id: String,
        #[serde(default)]
        expected_hash: Option<String>,
    }

    #[test]
    fn accepts_camel_case_keys() {
        let args = json!({ "projectId": "p1", "expectedHash": null });
        let value: String = p(&args, &["projectId", "project_id"]).unwrap();
        assert_eq!(value, "p1");
    }

    #[test]
    fn accepts_snake_case_keys() {
        let args = json!({ "project_id": "p2" });
        let value: String = p(&args, &["projectId", "project_id"]).unwrap();
        assert_eq!(value, "p2");
    }

    #[test]
    fn missing_required_argument_is_reported() {
        let args = json!({});
        let error = p::<String>(&args, &["callId", "call_id"]).unwrap_err();
        assert!(error.contains("callId"), "{error}");
    }

    #[test]
    fn missing_optional_argument_becomes_none() {
        let args = json!({});
        let value: Option<String> = p(&args, &["taskId", "task_id"]).unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn nested_structs_keep_camel_case() {
        let args = json!({ "input": { "projectId": "p3", "path": "a.rs", "content": "" } });
        let input: Compound = p(&args, &["input"]).unwrap();
        assert_eq!(input.project_id, "p3");
        assert!(input.expected_hash.is_none());
    }
}
