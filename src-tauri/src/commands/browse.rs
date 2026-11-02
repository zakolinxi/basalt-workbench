use std::path::PathBuf;

use serde::Serialize;

use crate::error::{AppError, AppResult};

/// One sub-directory of the listing. Only folders are returned: the picker is
/// used to choose a project root, never a file.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirItem {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirListing {
    pub path: String,
    pub parent: Option<String>,
    pub home: String,
    pub entries: Vec<DirItem>,
}

fn home_dir() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("C:\\"))
}

/// Lists directories on the machine running Basalt, so a phone client can pick
/// a project folder without a native file dialog.
#[tauri::command]
pub fn browse_directories(path: Option<String>) -> AppResult<DirListing> {
    let home = home_dir();
    let requested = path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.clone());

    let root = requested
        .canonicalize()
        .map_err(|_| AppError::msg("目录不存在或无权访问"))?;
    if !root.is_dir() {
        return Err(AppError::msg("该路径不是目录"));
    }

    let mut entries = Vec::new();
    let reader = std::fs::read_dir(&root).map_err(|error| AppError::msg(format!("无法读取目录：{error}")))?;
    for entry in reader.flatten() {
        if entries.len() >= 400 {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.eq_ignore_ascii_case("System Volume Information") {
            continue;
        }
        let is_dir = entry
            .file_type()
            .map(|kind| kind.is_dir())
            .unwrap_or(false);
        if !is_dir {
            continue;
        }
        entries.push(DirItem {
            name,
            path: entry.path().to_string_lossy().to_string(),
        });
    }
    entries.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));

    Ok(DirListing {
        path: root.to_string_lossy().to_string(),
        parent: root
            .parent()
            .map(|parent| parent.to_string_lossy().to_string()),
        home: home.to_string_lossy().to_string(),
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_directories() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("src")).unwrap();
        std::fs::write(temp.path().join("readme.md"), "x").unwrap();
        std::fs::create_dir(temp.path().join(".git")).unwrap();

        let listing = browse_directories(Some(temp.path().to_string_lossy().to_string())).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, vec!["src"]);
    }

    #[test]
    fn rejects_files_and_missing_paths() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(browse_directories(Some(file.to_string_lossy().to_string())).is_err());
        assert!(browse_directories(Some("Z:\\nowhere\\at\\all".into())).is_err());
    }

    #[test]
    fn home_fallback_is_used_when_path_missing() {
        let listing = browse_directories(None).unwrap();
        assert!(!listing.home.is_empty());
        assert!(!listing.path.is_empty());
    }
}
