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

pub fn branches(root: &Path) -> AppResult<Vec<String>> {
    if !is_repo(root) {
        return Err(AppError::msg("当前目录不是 Git 仓库"));
    }
    let output = run_git(
        root,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname:short)",
            "refs/heads",
        ],
    )?;
    if !output.status.success() {
        return Err(AppError::msg(format!(
            "读取分支失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn safe_relative_path(path: &str) -> AppResult<String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(AppError::msg("文件路径不能为空"));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute() {
        return Err(AppError::msg("Git 操作只能使用项目内相对路径"));
    }
    for component in parsed.components() {
        if matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        ) {
            return Err(AppError::msg("Git 操作路径不能包含 .. 或根路径"));
        }
    }
    Ok(path.replace('\\', "/"))
}

fn validate_branch_name(root: &Path, branch: &str) -> AppResult<String> {
    let branch = branch.trim();
    if branch.is_empty() || branch.chars().count() > 200 {
        return Err(AppError::msg("分支名不能为空且不得超过 200 字符"));
    }
    if branch.starts_with('-') {
        return Err(AppError::msg("分支名不能以 - 开头"));
    }
    let output = run_git(root, &["check-ref-format", "--branch", branch])?;
    if !output.status.success() {
        return Err(AppError::msg(format!(
            "分支名无效：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(branch.to_string())
}

pub fn checkout_branch(root: &Path, branch: &str) -> AppResult<()> {
    if !is_repo(root) {
        return Err(AppError::msg("当前目录不是 Git 仓库"));
    }
    let branch = validate_branch_name(root, branch)?;
    let output = run_git(root, &["checkout", &branch])?;
    if !output.status.success() {
        return Err(AppError::msg(format!(
            "切换分支失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

pub fn create_branch(root: &Path, branch: &str) -> AppResult<()> {
    if !is_repo(root) {
        return Err(AppError::msg("当前目录不是 Git 仓库"));
    }
    let branch = validate_branch_name(root, branch)?;
    let output = run_git(root, &["checkout", "-b", &branch])?;
    if !output.status.success() {
        return Err(AppError::msg(format!(
            "新建分支失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

pub fn commit(root: &Path, message: &str, paths: &[String]) -> AppResult<String> {
    if !is_repo(root) {
        return Err(AppError::msg("当前目录不是 Git 仓库"));
    }
    let message = message.trim();
    if message.is_empty() {
        return Err(AppError::msg("提交信息不能为空"));
    }
    if message.chars().count() > 5000 {
        return Err(AppError::msg("提交信息过长"));
    }
    let mut safe_paths = Vec::new();
    for path in paths {
        safe_paths.push(safe_relative_path(path)?);
    }
    if safe_paths.is_empty() {
        let output = run_git(root, &["add", "-A", "--", "."])?;
        if !output.status.success() {
            return Err(AppError::msg(format!(
                "git add 失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let output = run_git(root, &["commit", "-m", message])?;
        if !output.status.success() {
            return Err(AppError::msg(format!(
                "提交失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
    } else {
        let mut add_args: Vec<&str> = vec!["add", "-A", "--"];
        for path in &safe_paths {
            add_args.push(path);
        }
        let output = run_git(root, &add_args)?;
        if !output.status.success() {
            return Err(AppError::msg(format!(
                "git add 失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let mut commit_args: Vec<&str> = vec!["commit", "-m", message, "--"];
        for path in &safe_paths {
            commit_args.push(path);
        }
        let output = run_git(root, &commit_args)?;
        if !output.status.success() {
            return Err(AppError::msg(format!(
                "提交失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
    }
    let output = run_git(root, &["rev-parse", "--short", "HEAD"])?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Restores one tracked file to HEAD. Untracked files are intentionally not
/// deleted here; removing them is a separate, explicit action.
pub fn restore_file(root: &Path, path: &str) -> AppResult<()> {
    if !is_repo(root) {
        return Err(AppError::msg("当前目录不是 Git 仓库"));
    }
    let path = safe_relative_path(path)?;
    let tracked = run_git(root, &["ls-files", "--error-unmatch", "--", &path])?;
    if !tracked.status.success() {
        return Err(AppError::msg(
            "该文件未被 Git 跟踪，无法还原。未跟踪文件请在文件树中删除。",
        ));
    }
    let output = run_git(
        root,
        &["restore", "--source=HEAD", "--staged", "--worktree", "--", &path],
    )?;
    if !output.status.success() {
        return Err(AppError::msg(format!(
            "还原失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
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
        let initial = status(root);
        assert!(initial.is_repo);
        assert!(initial.entries.iter().any(|e| e.untracked));
        git(root, &["add", "hello.txt"]);
        git(root, &["commit", "-m", "initial"]);
        assert_eq!(branches(root).unwrap(), vec!["main".to_string()]);
        create_branch(root, "feature").unwrap();
        assert_eq!(status(root).branch.as_deref(), Some("feature"));
        let all = branches(root).unwrap();
        assert!(all.contains(&"main".to_string()));
        assert!(all.contains(&"feature".to_string()));
        checkout_branch(root, "main").unwrap();
        assert_eq!(status(root).branch.as_deref(), Some("main"));
        assert!(create_branch(root, "bad branch").is_err());
        std::fs::write(root.join("hello.txt"), "one\nTWO\n").unwrap();
        let diff = file_diff(root, "hello.txt", "demo").unwrap();
        assert!(diff.old_text.contains("two"));
        assert!(diff.new_text.contains("TWO"));
        let text = diff_text(root, Some("hello.txt")).unwrap();
        assert!(text.contains("-two"));
        assert!(text.contains("+TWO"));
        std::fs::write(root.join("second.txt"), "new file\n").unwrap();
        let hash = commit(root, "add second file", &["second.txt".to_string()]).unwrap();
        assert!(!hash.is_empty());
        assert!(!status(root)
            .entries
            .iter()
            .any(|entry| entry.path == "second.txt"));
        std::fs::write(root.join("second.txt"), "changed\n").unwrap();
        restore_file(root, "second.txt").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("second.txt"))
                .unwrap()
                .replace("\r\n", "\n"),
            "new file\n"
        );
        assert!(restore_file(root, "../outside.txt").is_err());
        std::fs::write(root.join("untracked.txt"), "x").unwrap();
        assert!(restore_file(root, "untracked.txt").is_err());
    }
}
