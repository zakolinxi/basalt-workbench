use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tauri::Emitter;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::events::{EVENT_TERMINAL, EVENT_TERMINAL_EXIT};
use crate::models::TerminalEvent;
use crate::state::AppState;

pub struct TerminalSession {
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
    pub child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    pub shell: String,
}

impl TerminalSession {
    pub fn write(&mut self, data: &str) -> AppResult<()> {
        self.writer.write_all(data.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> AppResult<()> {
        self.master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| AppError::msg(format!("调整终端尺寸失败：{e}")))
    }

    pub fn kill(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

fn shell_command(shell: &str, cwd: &Path) -> AppResult<CommandBuilder> {
    let mut cmd = match shell {
        "cmd" => CommandBuilder::new("cmd.exe"),
        "wsl" => {
            let mut c = CommandBuilder::new("wsl.exe");
            c.arg("--cd");
            c.arg(cwd.to_string_lossy().to_string());
            c
        }
        "bash" => CommandBuilder::new("bash.exe"),
        _ => {
            let mut c = CommandBuilder::new("powershell.exe");
            c.arg("-NoLogo");
            c.arg("-NoProfile");
            c
        }
    };
    cmd.cwd(cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("BASALT_TERMINAL", "1");
    Ok(cmd)
}

pub fn create_terminal(
    state: &AppState,
    cwd: &Path,
    shell: &str,
    cols: u16,
    rows: u16,
) -> AppResult<String> {
    if !cwd.is_dir() {
        return Err(AppError::msg("终端工作目录不存在"));
    }
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::msg(format!("创建终端失败：{e}")))?;
    let shell_cwd = crate::files::display_path(cwd);
    let cmd = shell_command(shell, &shell_cwd)?;
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| AppError::msg(format!("启动终端进程失败：{e}")))?;
    let child = Arc::new(Mutex::new(child));
    drop(pair.slave);
    let master = pair.master;
    let reader = master
        .try_clone_reader()
        .map_err(|e| AppError::msg(format!("读取终端输出失败：{e}")))?;
    let writer = master
        .take_writer()
        .map_err(|e| AppError::msg(format!("写入终端失败：{e}")))?;

    let terminal_id = Uuid::new_v4().to_string();
    let session = TerminalSession {
        writer,
        master,
        child: child.clone(),
        shell: shell.to_string(),
    };
    {
        let mut terminals = state
            .terminals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        terminate_oldest_if_needed(&mut terminals);
        terminals.insert(terminal_id.clone(), session);
    }

    let app = state.app.clone();
    let tid = terminal_id.clone();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buffer = vec![0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buffer[..n]).to_string();
                    let _ = app.emit(
                        EVENT_TERMINAL,
                        TerminalEvent {
                            terminal_id: tid.clone(),
                            data,
                        },
                    );
                }
                Err(_) => break,
            }
        }
    });

    let monitor_app = state.app.clone();
    let monitor_id = terminal_id.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(400));
            let status = {
                match child.lock() {
                    Ok(mut guard) => guard.try_wait(),
                    Err(_) => break,
                }
            };
            match status {
                Ok(Some(exit)) => {
                    let _ = monitor_app.emit(
                        EVENT_TERMINAL_EXIT,
                        serde_json::json!({
                            "terminalId": monitor_id,
                            "exitCode": exit.exit_code()
                        }),
                    );
                    break;
                }
                Ok(None) => continue,
                Err(_) => break,
            }
        }
    });

    Ok(terminal_id)
}

fn terminate_oldest_if_needed(terminals: &mut HashMap<String, TerminalSession>) {
    const MAX_TERMINALS: usize = 12;
    if terminals.len() >= MAX_TERMINALS {
        if let Some(key) = terminals.keys().next().cloned() {
            if let Some(mut session) = terminals.remove(&key) {
                session.kill();
            }
        }
    }
}

pub fn write_terminal(state: &AppState, id: &str, data: &str) -> AppResult<()> {
    let mut terminals = state
        .terminals
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session = terminals
        .get_mut(id)
        .ok_or_else(|| AppError::msg("终端不存在或已退出"))?;
    session.write(data)
}

pub fn resize_terminal(state: &AppState, id: &str, cols: u16, rows: u16) -> AppResult<()> {
    let mut terminals = state
        .terminals
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session = terminals
        .get_mut(id)
        .ok_or_else(|| AppError::msg("终端不存在或已退出"))?;
    session.resize(cols, rows)
}

pub fn kill_terminal(state: &AppState, id: &str) {
    let mut terminals = state
        .terminals
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(mut session) = terminals.remove(id) {
        session.kill();
    }
}

pub fn kill_all_terminals(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(state) = app.try_state::<AppState>() {
        let mut terminals = state
            .terminals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (_, mut session) in terminals.drain() {
            session.kill();
        }
    }
}
