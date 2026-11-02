use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::remote::{self, RemoteStatus};
use crate::state::AppState;

pub fn read_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

pub fn write_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    crate::db::set_setting_raw(conn, key, value)
}

#[tauri::command]
pub fn remote_status(state: State<'_, AppState>) -> AppResult<RemoteStatus> {
    let conn = state.db();
    Ok(remote::status(&conn))
}

#[tauri::command]
pub fn remote_set_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
    port: Option<u16>,
) -> AppResult<RemoteStatus> {
    {
        let conn = state.db();
        write_setting(&conn, "remoteEnabled", if enabled { "1" } else { "0" })?;
        if let Some(port) = port {
            if port < 1024 {
                return Err(crate::error::AppError::msg("端口需大于 1023"));
            }
            write_setting(&conn, "remotePort", &port.to_string())?;
        }
    }

    if enabled {
        remote::start_from_settings(&app, &state).map_err(crate::error::AppError::msg)?;
    } else {
        remote::stop();
        state.log("INFO", "远程访问网关已关闭");
    }
    let conn = state.db();
    Ok(remote::status(&conn))
}

#[tauri::command]
pub fn remote_rotate_code(state: State<'_, AppState>) -> AppResult<RemoteStatus> {
    remote::rotate_pairing_code();
    let conn = state.db();
    Ok(remote::status(&conn))
}
