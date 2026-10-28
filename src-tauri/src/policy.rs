use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::files::{ensure_safe_write_path, resolve_in_root};
use crate::models::Settings;

#[derive(Debug, Clone)]
pub enum PolicyDecision {
    Allow,
    NeedsApproval(String),
    Deny(String),
}

struct DangerousPattern {
    reason: &'static str,
    regex: Regex,
}

fn dangerous_patterns() -> &'static Vec<DangerousPattern> {
    static PATTERNS: OnceLock<Vec<DangerousPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let mut list = Vec::new();
        let mut add = |reason: &'static str, pattern: &str| {
            if let Ok(regex) = Regex::new(pattern) {
                list.push(DangerousPattern { reason, regex });
            }
        };
        add(
            "禁止磁盘格式化或分区操作",
            r"(?i)\b(format|diskpart|bcdedit|bootrec)\b",
        );
        add(
            "禁止关机、重启或修改启动配置",
            r"(?i)\b(shutdown|restart-computer|stop-computer)\b",
        );
        add(
            "禁止修改注册表",
            r"(?i)\b(reg\s+(add|delete|import)|set-itemproperty\s+.*hklm|new-itemproperty\s+.*hkcu)\b|\bHKLM:\\|\bHKCU:\\",
        );
        add(
            "禁止修改系统账户、服务、计划任务或防火墙",
            r"(?i)\b(net\s+user|net\s+localgroup|sc\s+(create|delete|config)|schtasks\s+/create|new-service|netsh\s+advfirewall|set-mppreference|set-executionpolicy)\b",
        );
        add(
            "禁止使用编码命令或动态执行隐藏载荷",
            r"(?i)(-enc(odedcommand)?\b|frombase64string|invoke-expression|\biex\b)",
        );
        add(
            "禁止下载并直接执行远程脚本",
            r"(?i)((curl|wget|iwr|invoke-webrequest|irm)\b[^\n|]*\|\s*(iex|invoke-expression|bash|sh|powershell|cmd))",
        );
        add(
            "禁止请求管理员提权",
            r"(?i)(start-process[^\n]*-verb\s+runas|\brunas\b)",
        );
        add(
            "禁止启动脱离任务生命周期的后台进程",
            r"(?i)\b(start-process|start-job|new-psterminal)\b",
        );
        add(
            "禁止递归删除项目根目录或通配符目标",
            r#"(?i)(remove-item\b[^\n]*(\.|\*)\s+-recurse|rm\s+-[a-z]*r[a-z]*f?\s+(\.|\*|/)|rd\s+/s\s+/q\s+(\.|\*)|del\s+/[fsq][^\n]*\*)"#,
        );
        add(
            "禁止在命令中使用 .. 访问项目目录以外内容",
            r#"(?i)(?:^|[\s"'(])\.\.[\\/]"#,
        );
        add(
            "禁止强制重置或清理 Git 工作区",
            r"(?i)\bgit\s+(clean\s+-[a-z]*[fd]|reset\s+--hard)\b",
        );
        add(
            "禁止操作系统保护目录或提权修改权限",
            r"(?i)\b(takeown|icacls\b[^\n]*(grant|reset)|cipher\s+/w|vssadmin|wbadmin)\b",
        );
        add(
            "禁止访问 UNC 网络共享路径",
            r#"(?i)(?:^|[\s"'(])\\\\[a-z0-9._$-]+\\"#,
        );
        list
    })
}

fn absolute_paths(command: &str) -> Vec<String> {
    static DRIVE: OnceLock<Regex> = OnceLock::new();
    let regex = DRIVE.get_or_init(|| {
        Regex::new(r#"(?i)(?:^|[\s"'(])([a-z]:[\\/][^\s"'|;&)]*)"#).unwrap_or_else(|_| Regex::new("$^").expect("regex"))
    });
    regex
        .captures_iter(command)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn external_side_effect(command: &str) -> Option<&'static str> {
    let lower = command.to_lowercase();
    if lower.contains("git push") {
        Some("git push 会推送到远程仓库，属于外部副作用操作")
    } else if lower.contains("npm publish") || lower.contains("cargo publish") {
        Some("发布命令会向外部仓库发布内容")
    } else {
        None
    }
}

pub fn dangerous_command_reason(command: &str, root: &Path) -> Option<String> {
    for pattern in dangerous_patterns() {
        if pattern.regex.is_match(command) {
            return Some(pattern.reason.to_string());
        }
    }
    let lower = command.to_lowercase();
    for token in [
        "$env:userprofile",
        "%userprofile%",
        "$home",
        "~/",
        "$env:appdata",
        "%appdata%",
        "$env:localappdata",
        "%localappdata%",
        "$env:temp",
        "%temp%",
        "$env:systemroot",
        "%systemroot%",
        "$env:windir",
        "%windir%",
        "$env:programfiles",
        "%programfiles%",
    ] {
        if lower.contains(token) {
            return Some("命令访问了项目目录以外的系统或用户目录".into());
        }
    }
    for raw in absolute_paths(command) {
        let path = Path::new(&raw);
        let resolved = if path.exists() {
            path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
        } else {
            path.to_path_buf()
        };
        if !crate::files::path_is_within(root, &resolved)
            && !crate::files::path_is_within(root, path)
        {
            return Some(format!(
                "命令包含项目目录以外的绝对路径：{}",
                path.to_string_lossy()
            ));
        }
    }
    None
}

pub fn evaluate(tool: &str, args: &Value, root: &Path, settings: &Settings) -> PolicyDecision {
    match tool {
        "update_plan" | "list_files" | "search_files" | "read_file" | "git_status"
        | "git_diff" | "web_search" | "fetch_url" | "spawn_subagent" => {
            PolicyDecision::Allow
        }
        "write_file" | "apply_patch" => {
            let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
                return PolicyDecision::Deny("缺少 path 参数".into());
            };
            match resolve_in_root(root, path).and_then(|abs| {
                ensure_safe_write_path(root, &abs)?;
                Ok(abs)
            }) {
                Ok(abs) => {
                    if settings.auto_approve_writes {
                        PolicyDecision::Allow
                    } else {
                        PolicyDecision::NeedsApproval(format!(
                            "将写入项目文件：{}",
                            abs.to_string_lossy()
                        ))
                    }
                }
                Err(err) => PolicyDecision::Deny(err.user_message()),
            }
        }
        "run_command" | "run_tests" => {
            let command = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if command.trim().is_empty() {
                return PolicyDecision::Deny("命令为空".into());
            }
            if let Some(reason) = dangerous_command_reason(command, root) {
                return PolicyDecision::Deny(reason);
            }
            if let Some(cwd) = args.get("cwd").and_then(|v| v.as_str()) {
                if let Err(err) = resolve_in_root(root, cwd) {
                    return PolicyDecision::Deny(err.user_message());
                }
            }
            let preview: String = command.chars().take(240).collect();
            if let Some(reason) = external_side_effect(command) {
                return PolicyDecision::NeedsApproval(format!("{reason}：{preview}"));
            }
            if settings.auto_approve_commands {
                PolicyDecision::Allow
            } else {
                PolicyDecision::NeedsApproval(format!("将执行命令：{preview}"))
            }
        }
        _ => PolicyDecision::Deny(format!("未知工具：{tool}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn root() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        (dir, root)
    }

    #[test]
    fn read_tools_are_allowed() {
        let (_dir, root) = root();
        let decision = evaluate("read_file", &json!({"path": "src/main.rs"}), &root, &Settings::default());
        assert!(matches!(decision, PolicyDecision::Allow));
    }

    #[test]
    fn writes_need_approval_by_default() {
        let (_dir, root) = root();
        let decision = evaluate(
            "write_file",
            &json!({"path": "a.txt", "content": "hi"}),
            &root,
            &Settings::default(),
        );
        assert!(matches!(decision, PolicyDecision::NeedsApproval(_)));
    }

    #[test]
    fn write_outside_root_is_denied() {
        let (_dir, root) = root();
        let decision = evaluate(
            "write_file",
            &json!({"path": "../a.txt", "content": "hi"}),
            &root,
            &Settings::default(),
        );
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }

    #[test]
    fn dangerous_commands_are_denied() {
        let (_dir, root) = root();
        let decision = evaluate(
            "run_command",
            &json!({"command": "format C:"}),
            &root,
            &Settings::default(),
        );
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }

    #[test]
    fn git_push_needs_approval() {
        let (_dir, root) = root();
        let decision = evaluate(
            "run_command",
            &json!({"command": "git push origin main"}),
            &root,
            &Settings::default(),
        );
        assert!(matches!(decision, PolicyDecision::NeedsApproval(_)));
    }

    #[test]
    fn absolute_paths_outside_project_are_denied() {
        let (_dir, root) = root();
        let decision = evaluate(
            "run_command",
            &json!({"command": "Get-Content C:\\Windows\\win.ini"}),
            &root,
            &Settings::default(),
        );
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }

    #[test]
    fn parent_traversal_in_commands_is_denied() {
        let (_dir, root) = root();
        let decision = evaluate(
            "run_command",
            &json!({"command": "Get-Content ..\\secret.txt"}),
            &root,
            &Settings::default(),
        );
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }
}
