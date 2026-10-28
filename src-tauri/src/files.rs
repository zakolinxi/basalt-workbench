use std::io::Read;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

const TEXT_PREVIEW_LIMIT: u64 = 400 * 1024;

fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase()
}

pub fn path_is_within(root: &Path, candidate: &Path) -> bool {
    let root_s = normalized(root);
    let cand_s = normalized(candidate);
    cand_s == root_s || cand_s.starts_with(&(root_s + "\\"))
}


pub fn display_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

pub fn canonical_root(root: &Path) -> AppResult<PathBuf> {
    let canon = root
        .canonicalize()
        .map_err(|e| AppError::msg(format!("项目目录不可用：{e}")))?;
    if !canon.is_dir() {
        return Err(AppError::msg("项目路径不是目录"));
    }
    Ok(canon)
}

pub fn resolve_in_root(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let rel = relative.trim();
    if rel.is_empty() {
        return Ok(root.to_path_buf());
    }
    if rel.contains('\0') {
        return Err(AppError::msg("路径包含非法字符"));
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(AppError::msg("不允许使用绝对路径，必须使用项目内相对路径"));
    }
    for comp in rel_path.components() {
        match comp {
            Component::ParentDir => return Err(AppError::msg("路径越界：不允许使用 .. 访问项目目录以外内容")),
            Component::Prefix(_) | Component::RootDir => {
                return Err(AppError::msg("路径越界：不允许使用绝对路径"))
            }
            _ => {}
        }
    }
    let candidate = root.join(rel_path);
    if candidate.exists() {
        let canon = candidate
            .canonicalize()
            .map_err(|e| AppError::msg(format!("无法解析路径：{e}")))?;
        if !path_is_within(root, &canon) {
            return Err(AppError::msg("路径越界：解析后的路径不在项目目录内（可能包含符号链接）"));
        }
        return Ok(canon);
    }

    let mut ancestor = candidate.clone();
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    while !ancestor.exists() {
        let name = ancestor
            .file_name()
            .ok_or_else(|| AppError::msg("无效路径"))?
            .to_os_string();
        suffix.push(name);
        ancestor = ancestor
            .parent()
            .ok_or_else(|| AppError::msg("无效路径"))?
            .to_path_buf();
    }
    let canon_ancestor = ancestor
        .canonicalize()
        .map_err(|e| AppError::msg(format!("无法解析路径：{e}")))?;
    if !path_is_within(root, &canon_ancestor) {
        return Err(AppError::msg("路径越界：目标位于项目目录之外"));
    }
    let mut result = canon_ancestor;
    for part in suffix.iter().rev() {
        result.push(part);
    }
    Ok(result)
}

pub fn read_text(path: &Path, max_bytes: u64) -> AppResult<(String, bool, bool, u64)> {
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return Err(AppError::msg("目标不是文件"));
    }
    let size = meta.len();
    let mut file = std::fs::File::open(path)?;
    let mut buffer = Vec::new();
    let limit = max_bytes.min(TEXT_PREVIEW_LIMIT.max(max_bytes));
    file.by_ref().take(limit + 1).read_to_end(&mut buffer)?;
    let truncated = buffer.len() as u64 > limit;
    if truncated {
        buffer.truncate(limit as usize);
    }
    let is_binary = buffer.iter().take(8192).any(|b| *b == 0);
    if is_binary {
        return Ok((String::new(), truncated, true, size));
    }
    let text = String::from_utf8_lossy(&buffer).to_string();
    Ok((text, truncated, false, size))
}

pub fn read_limited_bytes(path: &Path, max_bytes: u64) -> AppResult<Vec<u8>> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > max_bytes {
        return Err(AppError::msg(format!(
            "文件过大（{} 字节），超过限制 {} 字节",
            meta.len(),
            max_bytes
        )));
    }
    Ok(std::fs::read(path)?)
}

pub fn decode_output(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let (decoded, _, _) = encoding_rs::GBK.decode(bytes);
            decoded.to_string()
        }
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn hash_text(text: &str) -> String {
    hash_bytes(text.as_bytes())
}

pub fn hash_file(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    Ok(hash_bytes(&bytes))
}

pub fn language_for(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "rs" => "rust",
        "json" => "json",
        "md" | "mdx" => "markdown",
        "css" => "css",
        "html" | "htm" => "html",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" => "cpp",
        "cs" => "csharp",
        "toml" => "ini",
        "yaml" | "yml" => "yaml",
        "xml" => "xml",
        "sql" => "sql",
        "sh" | "bash" => "shell",
        "ps1" => "powershell",
        "bat" | "cmd" => "bat",
        _ => "plaintext",
    }
    .to_string()
}

pub fn truncate_chars(text: &str, max: usize) -> (String, bool) {
    if text.chars().count() <= max {
        (text.to_string(), false)
    } else {
        (text.chars().take(max).collect(), true)
    }
}

pub fn ensure_safe_write_path(root: &Path, path: &Path) -> AppResult<()> {
    if !path_is_within(root, path) {
        return Err(AppError::msg("拒绝写入项目目录以外的路径"));
    }
    let lower = normalized(path);
    for blocked in ["\\.git\\", "\\.basalt\\"] {
        if lower.contains(blocked) {
            return Err(AppError::msg("拒绝写入受保护的版本控制或应用内部目录"));
        }
    }
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.eq_ignore_ascii_case("basalt.db") {
            return Err(AppError::msg("拒绝写入应用数据库文件"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_traversal() {
        let root = std::env::temp_dir();
        let err = resolve_in_root(&root, "../secret.txt").unwrap_err();
        assert!(err.to_string().contains("越界"));
    }

    #[test]
    fn rejects_absolute_paths() {
        let root = std::env::temp_dir();
        let err = resolve_in_root(&root, "C:\\Windows\\win.ini").unwrap_err();
        assert!(err.to_string().contains("绝对路径"));
    }

    #[test]
    fn accepts_nested_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical_root(dir.path()).unwrap();
        let path = resolve_in_root(&root, "src/main.rs").unwrap();
        assert!(path.starts_with(&root));
    }

    #[test]
    fn blocks_git_writes() {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical_root(dir.path()).unwrap();
        let path = resolve_in_root(&root, ".git/config").unwrap();
        assert!(ensure_safe_write_path(&root, &path).is_err());
    }

    #[test]
    fn detects_binary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        std::fs::write(&path, [0u8, 1, 2, 3]).unwrap();
        let (_, _, binary, _) = read_text(&path, 1024).unwrap();
        assert!(binary);
    }
}
