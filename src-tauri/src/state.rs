use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::Connection;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::terminal::TerminalSession;

pub struct AppState {
    pub app: AppHandle,
    pub data_dir: PathBuf,
    pub db: Mutex<Connection>,
    pub http: reqwest::Client,
    pub chat_cancels: Mutex<HashMap<String, CancellationToken>>,
    pub task_cancels: Mutex<HashMap<String, CancellationToken>>,
    pub approvals: Mutex<HashMap<String, oneshot::Sender<bool>>>,
    pub terminals: Mutex<HashMap<String, TerminalSession>>,
    pub agent_active: Mutex<bool>,
}

impl AppState {
    pub fn new(app: &AppHandle) -> AppResult<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::msg(format!("无法获取应用数据目录：{e}")))?;
        for sub in ["logs", "images", "attachments", "snapshots"] {
            std::fs::create_dir_all(data_dir.join(sub))?;
        }
        let conn = db::open_db(&data_dir)?;
        db::mark_interrupted_tasks(&conn)?;
        let http = reqwest::Client::builder()
            .user_agent(crate::models::default_user_agent())
            .connect_timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self {
            app: app.clone(),
            data_dir,
            db: Mutex::new(conn),
            http,
            chat_cancels: Mutex::new(HashMap::new()),
            task_cancels: Mutex::new(HashMap::new()),
            approvals: Mutex::new(HashMap::new()),
            terminals: Mutex::new(HashMap::new()),
            agent_active: Mutex::new(false),
        })
    }

    pub fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn log(&self, level: &str, message: &str) {
        crate::logging::log_line(&self.data_dir, level, message);
    }
}
