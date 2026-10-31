pub mod agent;
pub mod changes;
pub mod commands;
pub mod db;
pub mod error;
pub mod events;
pub mod files;
pub mod git;
pub mod logging;
pub mod messaging;
pub mod models;
pub mod policy;
pub mod providers;
pub mod secrets;
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::terminal::terminal_create,
            commands::terminal::terminal_write,
            commands::terminal::terminal_resize,
            commands::terminal::terminal_kill,
            commands::images::image_generate,
            commands::images::image_capabilities,
            commands::images::image_history,
            commands::images::image_delete,
            commands::images::image_save_as,
            commands::images::chat_image_generate
        ])
        .build(tauri::generate_context!())
        .expect("Basalt 无法启动 Tauri 应用");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
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
                events::mark_clean_exit(&state);
                state.log("INFO", "Basalt Workbench 正常退出");
            }
        }
    });
}
