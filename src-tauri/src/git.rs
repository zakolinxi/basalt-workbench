use std::path::Path;
use std::process::Command;

use crate::error::{AppError, AppResult};
use crate::models::{FileDiff, GitEntry, GitStatus};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn run_git(root: &Path, args: &[&str]) -> AppResult<std::process::Output> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    cmd.current_dir(root);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd.output()
        .map_err(|e| AppError::msg(format!("无法执行 Git：{e}。请确认已安装 Git 并加入 PATH。")))
}

pub fn is_repo(root: &Path) -> bool {
    run_git(root, &["rev-parse", "--is-inside-work-tree"])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn status(root: &Path) -> GitStatus {
    if !is_repo(root) {
        return GitStatus {
            is_repo: false,
            branch: None,
            entries: Vec::new(),
            error: None,
        };
    }
    let branch = run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let output = match run_git(root, &["status", "--porcelain=v1", "-z", "-uall"]) {
        Ok(output) => output,
        Err(err) => {
            return GitStatus {
                is_repo: true,
                branch,
                entries: Vec::new(),
                error: Some(err.user_message()),
            }
        }
    };
    if !output.status.success() {
        return GitStatus {
            is_repo: true,
            branch,
            entries: Vec::new(),
            error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
        };
    }
    let raw = output.stdout;
    let mut records = raw.split(|b| *b == 0).filter(|r| !r.is_empty()).peekable();
    let mut entries = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let x = record[0] as char;
        let y = record[1] as char;
        let path = String::from_utf8_lossy(&record[3..]).to_string();
        let untracked = x == '?' && y == '?';
        let staged = x != ' ' && x != '?';
        let code = if y != ' ' && y != '?' { y } else { x };
        let status = match code {
            'M' => "modified",
            'A' => "added",
            'D' => "deleted",
            'R' => "renamed",
            'C' => "copied",
            'U' => "conflict",
            '?' => "untracked",
            '!' => "ignored",
            _ => "changed",
        };
        if matches!(x, 'R' | 'C') {
            if let Some(original) = records.next() {
                let from = String::from_utf8_lossy(original).to_string();
                entries.push(GitEntry {
                    path: format!("{from} -> {path}"),
                    status: status.into(),
                    staged,
                    untracked: false,
                });
                continue;
            }
        }
        entries.push(GitEntry {
            path,
            status: status.into(),
            staged,
            untracked,
        });
    }
    GitStatus {
        is_repo: true,
        branch,
        entries,
        error: None,
    }
}

pub fn file_diff(root: &Path, rel_path: &str, root_label: &str) -> AppResult<FileDiff> {
    let abs_path = crate::files::resolve_in_root(root, rel_path)?;
    let head_path = rel_path.replace('\\', "/");
    let old_output = run_git(root, &["show", &format!("HEAD:{head_path}")])?;
    let (old_text, old_truncated, old_binary) = if old_output.status.success() {
        let text = String::from_utf8_lossy(&old_output.stdout).to_string();
        let binary = old_output.stdout.iter().take(8192).any(|b| *b == 0);
        let (text, truncated) = crate::files::truncate_chars(&text, 600_000);
        (text, truncated, binary)
    } else {
        (String::new(), false, false)
    };
    let (new_text, new_truncated, new_binary) = if abs_path.exists() {
        let (text, truncated, binary, _) = crate::files::read_text(&abs_path, 1_500_000)?;
        (text, truncated, binary)
    } else {
        (String::new(), false, false)
    };
    Ok(FileDiff {
        path: rel_path.to_string(),
        old_text,
        new_text,
        old_label: format!("HEAD · {root_label}"),
        new_label: "工作区".into(),
        binary: old_binary || new_binary,
        truncated: old_truncated || new_truncated,
        source: "git".into(),
    })
}

pub fn diff_text(root: &Path, rel_path: Option<&str>) -> AppResult<String> {
    let mut args = vec!["diff", "--no-color", "--no-ext-diff", "--unified=3"];
    if let Some(path) = rel_path {
        args.push("--");
        args.push(path);
    }
    let output = run_git(root, &args)?;
    if !output.status.success() {
        return Err(AppError::msg(format!(
            "git diff 失败：{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let text = crate::files::decode_output(&output.stdout);
    let (text, truncated) = crate::files::truncate_chars(&text, 120_000);
    Ok(if truncated {
        format!("{text}\n\n[输出已截断]")
    } else {
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git should be available");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn reports_status_and_head_diff() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.email", "basalt@example.com"]);
        git(root, &["config", "user.name", "Basalt"]);
        std::fs::write(root.join("hello.txt"), "one\ntwo\n").unwrap();
        let status = status(root);
        assert!(status.is_repo);
        assert!(status.entries.iter().any(|e| e.untracked));
        git(root, &["add", "hello.txt"]);
        git(root, &["commit", "-m", "initial"]);
        std::fs::write(root.join("hello.txt"), "one\nTWO\n").unwrap();
        let diff = file_diff(root, "hello.txt", "demo").unwrap();
        assert!(diff.old_text.contains("two"));
        assert!(diff.new_text.contains("TWO"));
        let text = diff_text(root, Some("hello.txt")).unwrap();
        assert!(text.contains("-two"));
        assert!(text.contains("+TWO"));
    }
}
