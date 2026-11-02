pub mod agent;
pub mod changes;
pub mod commands;
pub mod db;
pub mod error;
pub mod events;
pub mod files;
pub mod git;
pub mod logging;
pub mod mcp;
pub mod messaging;
pub mod model_interfaces;
pub mod models;
pub mod plugins;
pub mod policy;
pub mod providers;
pub mod remote;
pub mod secrets;
pub mod skills;
pub mod state;
pub mod terminal;

use tauri::Manager;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = AppState::new(app.handle()).expect("Basalt 初始化失败：无法创建应用状态");
            state.log("INFO", "Basalt Workbench 启动");
            app.manage(state);
            // The LAN gateway is opt-in; when the user enabled it last time it
            // comes back up here. A busy port must not stop the app from
            // starting, so failures are only logged.
            let handle = app.handle().clone();
            let state = app.state::<AppState>();
            remote::install_event_forwarder(&handle);
            let _ = remote::start_from_settings(&handle, &state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::browse::browse_directories,
            commands::misc::bootstrap,
            commands::misc::settings_get,
            commands::misc::settings_set,
            commands::misc::resolve_approval,
            commands::misc::open_path,
            commands::misc::app_info,
            commands::misc::session_message_count,
            commands::providers::provider_list,
            commands::providers::provider_get,
            commands::providers::provider_create,
            commands::providers::provider_update,
            commands::providers::provider_delete,
            commands::providers::provider_duplicate,
            commands::providers::provider_test,
            commands::providers::provider_model_endpoints,
            commands::providers::provider_detect_model,
            commands::providers::provider_set_enabled,
            commands::providers::provider_set_capability,
            commands::providers::provider_models_from_discovered,
            commands::projects::project_list,
            commands::projects::project_add,
            commands::projects::project_open,
            commands::projects::project_remove,
            commands::files::project_list_dir,
            commands::files::project_read_file,
            commands::files::project_write_file,
            commands::files::git_status,
            commands::files::git_branches,
            commands::files::git_checkout_branch,
            commands::files::git_create_branch,
            commands::files::git_file_diff,
            commands::files::git_commit,
            commands::files::git_restore_file,
            commands::sessions::session_list,
            commands::sessions::session_create,
            commands::sessions::session_update,
            commands::sessions::session_delete,
            commands::sessions::message_list,
            commands::tasks::task_create,
            commands::tasks::task_list,
            commands::tasks::task_get,
            commands::tasks::task_start,
            commands::tasks::task_cancel,
            commands::tasks::task_delete,
            commands::tasks::review_task,
            commands::changes::change_list,
            commands::changes::change_diff_cmd,
            commands::changes::change_resolve,
            commands::chat::attachment_import,
            commands::chat::attachment_import_bytes,
            commands::chat::read_media,
            commands::chat::attachment_delete,
            commands::chat::chat_send,
            commands::chat::chat_cancel,
            commands::chat::context_usage,
            commands::chat::usage_stats,
            commands::chat::run_metrics,
            commands::terminal::terminal_create,
            commands::terminal::terminal_write,
            commands::terminal::terminal_resize,
            commands::terminal::terminal_kill,
            commands::images::image_generate,
            commands::images::image_capabilities,
            commands::images::image_history,
            commands::images::image_delete,
            commands::images::image_save_as,
            commands::images::chat_image_generate,
            commands::extensions::skill_list,
            commands::extensions::skill_install,
            commands::extensions::skill_set_enabled,
            commands::extensions::skill_delete,
            commands::extensions::mcp_server_list,
            commands::extensions::mcp_server_save,
            commands::extensions::mcp_server_delete,
            commands::extensions::mcp_server_set_enabled,
            commands::extensions::mcp_server_refresh,
            commands::extensions::mcp_server_test,
            commands::extensions::mcp_tool_list,
            commands::extensions::mcp_tool_call,
            commands::extensions::tool_registry,
            commands::extensions::plugin_list,
            commands::extensions::plugin_install,
            commands::extensions::plugin_uninstall,
            commands::remote::remote_status,
            commands::remote::remote_set_enabled,
            commands::remote::remote_rotate_code
        ])
        .build(tauri::generate_context!())
        .expect("Basalt 无法启动 Tauri 应用");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            remote::stop();
            terminal::kill_all_terminals(app_handle);
            if let Some(state) = app_handle.try_state::<AppState>() {
                let cancels = state
                    .task_cancels
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                for (_, token) in cancels.iter() {
                    token.cancel();
                }
                let chats = state
                    .chat_cancels
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                for (_, token) in chats.iter() {
                    token.cancel();
                }
                drop(cancels);
                drop(chats);
                let handle = app_handle.clone();
                let _ = std::thread::spawn(move || {
                    if let Some(state) = handle.try_state::<AppState>() {
                        let _ = tauri::async_runtime::block_on(async {
                            tokio::time::timeout(
                                std::time::Duration::from_secs(3),
                                mcp::disconnect_all(&state),
                            )
                            .await
                        });
                    }
                })
                .join();
                events::mark_clean_exit(&state);
                state.log("INFO", "Basalt Workbench 正常退出");
            }
        }
    });
}
