use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chrono::Local;

use crate::error::redact;

pub fn log_line(data_dir: &Path, level: &str, message: &str) {
    let logs_dir = data_dir.join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);
    let path = logs_dir.join("basalt.log");
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 5 * 1024 * 1024 {
            let rotated = logs_dir.join("basalt.log.1");
            let _ = std::fs::remove_file(&rotated);
            let _ = std::fs::rename(&path, rotated);
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let line = format!(
            "{} [{}] {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            level,
            redact(message)
        );
        let _ = file.write_all(line.as_bytes());
    }
}

pub fn info(data_dir: &Path, message: &str) {
    log_line(data_dir, "INFO", message);
}

pub fn warn(data_dir: &Path, message: &str) {
    log_line(data_dir, "WARN", message);
}

pub fn error(data_dir: &Path, message: &str) {
    log_line(data_dir, "ERROR", message);
}
