use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use rusqlite::params;
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

use crate::changes::write_file_tracked;
use crate::error::{AppError, AppResult};
use crate::events::push_task_event;
use crate::files::{
    ensure_safe_write_path, language_for, read_text, resolve_in_root, truncate_chars,
};
use crate::models::{Task, TaskStep};
use crate::providers::ToolSchema;
use crate::state::AppState;

const HEAVY_DIRS: [&str; 11] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    "vendor",
    ".venv",
    "__pycache__",
    ".cache",
];

pub struct ToolResult {
    pub data: Value,
    pub summary: String,
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn should_skip_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(|name| HEAVY_DIRS.contains(&name))
            .unwrap_or(false)
}

pub fn schemas() -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            name: "update_plan".into(),
            description: "提交或更新任务执行计划。复杂任务开始时调用，最多 12 个步骤。status 可为 pending、in_progress、done、failed。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": {"type": "string"},
                                "detail": {"type": "string"},
                                "status": {"type": "string", "enum": ["pending", "in_progress", "done", "failed"]}
                            },
                            "required": ["title"]
                        }
                    }
                },
                "required": ["steps"]
            }),
        },
        ToolSchema {
            name: "list_files".into(),
            description: "列出项目目录中的文件与子目录，自动跳过 node_modules、target 等重型目录。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "项目内相对路径，默认项目根目录"},
                    "depth": {"type": "integer", "minimum": 1, "maximum": 4}
                }
            }),
        },
        ToolSchema {
            name: "search_files".into(),
            description: "在项目文本文件中搜索字面量字符串（不区分大小写），返回文件路径、行号和匹配行。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "path": {"type": "string"},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": 200}
                },
                "required": ["query"]
            }),
        },
        ToolSchema {
            name: "read_file".into(),
            description: "读取项目内文本文件。可使用 start_line 和 end_line 读取指定行范围。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "start_line": {"type": "integer", "minimum": 1},
                    "end_line": {"type": "integer", "minimum": 1}
                },
                "required": ["path"]
            }),
        },
        ToolSchema {
            name: "write_file".into(),
            description: "写入项目内的文件（新建或整体覆盖）。会请求用户批准，并记录可撤销的变更快照。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                    "reason": {"type": "string"}
                },
                "required": ["path", "content"]
            }),
        },
        ToolSchema {
            name: "apply_patch".into(),
            description: "按精确文本替换修改项目文件。old_string 必须与文件内容完全一致。会请求用户批准并记录变更快照。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "old_string": {"type": "string"},
                    "new_string": {"type": "string"},
                    "replace_all": {"type": "boolean"}
                },
                "required": ["path", "old_string", "new_string"]
            }),
        },
        ToolSchema {
            name: "git_status".into(),
            description: "查看项目 Git 状态与分支。".into(),
            parameters: json!({"type": "object", "properties": {}}),
        },
        ToolSchema {
            name: "git_diff".into(),
            description: "查看工作区未提交的 Git Diff，可选限定单个文件。".into(),
            parameters: json!({
                "type": "object",
                "properties": {"path": {"type": "string"}}
            }),
        },
        ToolSchema {
            name: "run_command".into(),
            description: "在项目目录内以 PowerShell 非交互模式运行命令。会请求用户批准；危险命令会被直接拒绝。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "cwd": {"type": "string", "description": "项目内相对工作目录"},
                    "timeout_secs": {"type": "integer", "minimum": 5, "maximum": 600}
                },
                "required": ["command"]
            }),
        },
        ToolSchema {
            name: "run_tests".into(),
            description: "运行测试命令。可自动检测 npm test 或 cargo test，也会请求用户批准。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "timeout_secs": {"type": "integer", "minimum": 5, "maximum": 600}
                }
            }),
        },
        ToolSchema {
            name: "spawn_subagent".into(),
            description: "启动一个只读子代理，独立调查代码、审查方案或联网检索。子代理可以读取项目文件、查看 Git、搜索和读取公开网页，但不会修改文件、执行命令或请求审批。适合把可并行的调研/审查拆出去。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "task": {"type": "string", "description": "明确的子任务与期望输出"},
                    "role": {"type": "string", "enum": ["researcher", "reviewer", "planner"]},
                    "context": {"type": "string", "description": "可选：已知背景、文件范围或约束"}
                },
                "required": ["task"]
            }),
        },
    ]
    .into_iter()
    .chain(super::web::schemas())
    .collect()
}

const READONLY_TOOLS: [&str; 7] = [
    "list_files",
    "search_files",
    "read_file",
    "git_status",
    "git_diff",
    "web_search",
    "fetch_url",
];

pub fn schemas_readonly() -> Vec<ToolSchema> {
    schemas()
        .into_iter()
        .filter(|tool| READONLY_TOOLS.contains(&tool.name.as_str()))
        .collect()
}

pub async fn execute_readonly(
    state: &AppState,
    task: &Task,
    root: &Path,
    name: &str,
    args: &Value,
    cancel: &CancellationToken,
) -> AppResult<ToolResult> {
    if !READONLY_TOOLS.contains(&name) {
        return Err(AppError::msg(format!("子代理不能调用工具：{name}")));
    }
    execute(state, task, root, name, args, cancel).await
}

pub async fn execute(
    state: &AppState,
    task: &Task,
    root: &Path,
    name: &str,
    args: &Value,
    cancel: &CancellationToken,
) -> AppResult<ToolResult> {
    match name {
        "update_plan" => update_plan(state, task, args),
        "list_files" => list_files(state, task, root, args),
        "search_files" => search_files(root, args),
        "read_file" => read_file(root, args),
        "write_file" => write_file(state, task, root, args),
        "apply_patch" => apply_patch(state, task, root, args),
        "git_status" => {
            let status = crate::git::status(root);
            Ok(ToolResult {
                data: serde_json::to_value(status)?,
                summary: "读取 Git 状态".into(),
            })
        }
        "git_diff" => {
            let path = args.get("path").and_then(|v| v.as_str());
            if let Some(path) = path {
                resolve_in_root(root, path)?;
            }
            let text = crate::git::diff_text(root, path)?;
            let display = if text.trim().is_empty() {
                "（没有未提交的 Diff；未跟踪文件不会出现在 git diff 中）".to_string()
            } else {
                text
            };
            Ok(ToolResult {
                data: json!({"diff": display}),
                summary: "读取 Git Diff".into(),
            })
        }
        "run_command" => run_command_tool(state, task, root, args, cancel, false).await,
        "run_tests" => run_command_tool(state, task, root, args, cancel, true).await,
        "web_search" | "fetch_url" => super::web::execute(state, name, args, cancel).await,
        "spawn_subagent" => {
            Box::pin(super::subagent::run(state, task, root, args, cancel)).await
        }
        other => Err(AppError::msg(format!("未知工具：{other}"))),
    }
}

fn update_plan(state: &AppState, task: &Task, args: &Value) -> AppResult<ToolResult> {
    let raw_steps = args
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::msg("update_plan 需要 steps 数组"))?;
    if raw_steps.is_empty() {
        return Err(AppError::msg("执行计划不能为空"));
    }
    if raw_steps.len() > 12 {
        return Err(AppError::msg("执行计划最多 12 个步骤"));
    }
    let mut steps = Vec::new();
    for raw in raw_steps {
        let title = raw
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if title.is_empty() {
            return Err(AppError::msg("计划步骤标题不能为空"));
        }
        let status = raw
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending");
        let status = match status {
            "pending" | "in_progress" | "done" | "failed" => status,
            _ => "pending",
        };
        steps.push(TaskStep {
            title: title.chars().take(200).collect(),
            detail: raw
                .get("detail")
                .and_then(|v| v.as_str())
                .map(|d| d.chars().take(600).collect()),
            status: status.to_string(),
        });
    }
    {
        let conn = state.db();
        conn.execute(
            "UPDATE tasks SET plan_json = ?2, updated_at = ?3 WHERE id = ?1",
            params![task.id, serde_json::to_string(&steps)?, crate::db::now()],
        )?;
    }
    push_task_event(
        state,
        &task.id,
        "PlanUpdated",
        json!({"steps": steps, "taskId": task.id}),
    );
    for (index, step) in steps.iter().enumerate() {
        match step.status.as_str() {
            "in_progress" => push_task_event(
                state,
                &task.id,
                "StepStarted",
                json!({"index": index, "title": step.title, "status": step.status}),
            ),
            "done" => push_task_event(
                state,
                &task.id,
                "StepCompleted",
                json!({"index": index, "title": step.title, "status": step.status}),
            ),
            _ => {}
        }
    }
    Ok(ToolResult {
        data: json!({"steps": steps, "count": steps.len()}),
        summary: format!("更新执行计划（{} 步）", steps.len()),
    })
}

fn list_files(state: &AppState, task: &Task, root: &Path, args: &Value) -> AppResult<ToolResult> {
    let _ = task;
    let base = resolve_in_root(
        root,
        args.get("path").and_then(|v| v.as_str()).unwrap_or(""),
    )?;
    if !base.is_dir() {
        return Err(AppError::msg("list_files 的目标必须是目录"));
    }
    let depth = args
        .get("depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(2)
        .clamp(1, 4) as usize;
    let mut entries = Vec::new();
    for entry in WalkDir::new(&base)
        .max_depth(depth)
        .into_iter()
        .filter_entry(|entry| !should_skip_dir(entry))
        .filter_map(Result::ok)
    {
        if entry.path() == base {
            continue;
        }
        let rel = relative(root, entry.path());
        if rel.is_empty() {
            continue;
        }
        entries.push(json!({
            "path": rel,
            "type": if entry.file_type().is_dir() { "dir" } else { "file" },
            "size": if entry.file_type().is_file() { entry.metadata().map(|m| m.len()).unwrap_or(0) } else { 0 }
        }));
        if entries.len() >= 400 {
            break;
        }
    }
    entries.sort_by(|a, b| {
        a["path"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["path"].as_str().unwrap_or_default())
    });
    let _ = state;
    Ok(ToolResult {
        data: json!({
            "base": relative(root, &base),
            "entries": entries,
            "truncated": entries.len() >= 400
        }),
        summary: format!("列出 {} 项", entries.len()),
    })
}

fn search_files(root: &Path, args: &Value) -> AppResult<ToolResult> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if query.is_empty() {
        return Err(AppError::msg("search_files 需要非空 query"));
    }
    let base = resolve_in_root(
        root,
        args.get("path").and_then(|v| v.as_str()).unwrap_or(""),
    )?;
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(50)
        .clamp(1, 200) as usize;
    let needle = query.to_lowercase();
    let mut matches = Vec::new();
    let mut searched = 0u64;
    'outer: for entry in WalkDir::new(&base)
        .into_iter()
        .filter_entry(|entry| !should_skip_dir(entry))
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if size > 512 * 1024 {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        if bytes.iter().take(4096).any(|b| *b == 0) {
            continue;
        }
        searched += 1;
        let text = String::from_utf8_lossy(&bytes);
        for (line_index, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(&needle) {
                matches.push(json!({
                    "path": relative(root, entry.path()),
                    "line": line_index + 1,
                    "text": line.trim().chars().take(220).collect::<String>()
                }));
                if matches.len() >= max_results {
                    break 'outer;
                }
            }
        }
    }
    Ok(ToolResult {
        data: json!({
            "query": query,
            "matches": matches,
            "searchedFiles": searched,
            "truncated": matches.len() >= max_results
        }),
        summary: format!("搜索到 {} 处匹配", matches.len()),
    })
}

fn read_file(root: &Path, args: &Value) -> AppResult<ToolResult> {
    let rel = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("read_file 需要 path"))?;
    let abs = resolve_in_root(root, rel)?;
    if !abs.is_file() {
        return Err(AppError::msg(format!("文件不存在：{rel}")));
    }
    let (content, truncated_file, binary, size) = read_text(&abs, 400_000)?;
    if binary {
        return Err(AppError::msg("目标是二进制文件，无法读取文本内容"));
    }
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let start = args
        .get("start_line")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .max(1) as usize;
    let end = args
        .get("end_line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(total_lines)
        .min(total_lines);
    if start > total_lines && total_lines > 0 {
        return Err(AppError::msg(format!(
            "起始行 {start} 超出文件总行数 {total_lines}"
        )));
    }
    let slice_start = (start - 1).min(total_lines);
    let slice_end = end.min(total_lines).max(slice_start);
    let selected = lines[slice_start..slice_end].join("\n");
    let (selected, truncated_chars) = truncate_chars(&selected, 120_000);
    Ok(ToolResult {
        data: json!({
            "path": relative(root, &abs),
            "content": selected,
            "startLine": if total_lines == 0 { 0 } else { start },
            "endLine": end,
            "totalLines": total_lines,
            "size": size,
            "language": language_for(rel),
            "truncated": truncated_file || truncated_chars
        }),
        summary: format!("读取 {rel}（{total_lines} 行）"),
    })
}

fn write_file(state: &AppState, task: &Task, root: &Path, args: &Value) -> AppResult<ToolResult> {
    let rel = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("write_file 需要 path"))?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("write_file 需要 content"))?;
    if content.len() > 2 * 1024 * 1024 {
        return Err(AppError::msg("写入内容超过 2MB 限制"));
    }
    let abs = resolve_in_root(root, rel)?;
    if abs.is_dir() {
        return Err(AppError::msg("目标路径是目录"));
    }
    ensure_safe_write_path(root, &abs)?;
    let change = write_file_tracked(state, task, root, rel, content)?;
    Ok(ToolResult {
        data: json!({
            "path": relative(root, &abs),
            "bytes": content.len(),
            "changed": change.is_some(),
            "changeId": change.as_ref().map(|c| c.id.clone())
        }),
        summary: format!(
            "{} {}",
            if change.is_some() { "写入" } else { "内容未变化：" },
            rel
        ),
    })
}

fn apply_patch(state: &AppState, task: &Task, root: &Path, args: &Value) -> AppResult<ToolResult> {
    let rel = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("apply_patch 需要 path"))?;
    let old_string = args
        .get("old_string")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("apply_patch 需要 old_string"))?;
    let new_string = args
        .get("new_string")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("apply_patch 需要 new_string"))?;
    if old_string.is_empty() {
        return Err(AppError::msg("old_string 不能为空，新建文件请使用 write_file"));
    }
    let replace_all = args
        .get("replace_all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let abs = resolve_in_root(root, rel)?;
    if !abs.is_file() {
        return Err(AppError::msg(format!("文件不存在：{rel}。新文件请使用 write_file")));
    }
    ensure_safe_write_path(root, &abs)?;
    let (content, _, binary, _) = read_text(&abs, 2_000_000)?;
    if binary {
        return Err(AppError::msg("目标文件是二进制文件"));
    }
    let occurrences = content.matches(old_string).count();
    if occurrences == 0 {
        return Err(AppError::msg(
            "old_string 在文件中未找到，请先读取文件确认精确内容（包括缩进和换行）",
        ));
    }
    if occurrences > 1 && !replace_all {
        return Err(AppError::msg(format!(
            "old_string 出现 {occurrences} 次，存在歧义。请提供更长上下文，或设置 replace_all=true"
        )));
    }
    let updated = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };
    if updated.len() > 2 * 1024 * 1024 {
        return Err(AppError::msg("修改后文件超过 2MB 限制"));
    }
    let change = write_file_tracked(state, task, root, rel, &updated)?;
    Ok(ToolResult {
        data: json!({
            "path": relative(root, &abs),
            "occurrences": if replace_all { occurrences } else { 1 },
            "changeId": change.as_ref().map(|c| c.id.clone())
        }),
        summary: format!("修改 {rel}"),
    })
}

struct CommandOutcome {
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
    duration_ms: u64,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

async fn kill_tree(child: &mut tokio::process::Child) {
    let pid = child.id();
    if let Some(pid) = pid {
        let _ = tokio::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

async fn execute_shell(
    command: &str,
    cwd: &Path,
    timeout_secs: u64,
    cancel: &CancellationToken,
) -> AppResult<CommandOutcome> {
    let mut cmd = tokio::process::Command::new("powershell.exe");
    cmd.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        command,
    ]);
    cmd.current_dir(cwd);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.as_std_mut().creation_flags(0x08000000);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::msg(format!("无法启动 PowerShell：{e}")))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::msg("无法捕获标准输出"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::msg("无法捕获错误输出"))?;
    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        let _ = stdout.read_to_end(&mut buffer).await;
        buffer
    });
    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        let _ = stderr.read_to_end(&mut buffer).await;
        buffer
    });
    let started = Instant::now();
    let timeout = Duration::from_secs(timeout_secs.clamp(5, 600));
    enum Outcome {
        Exited(std::io::Result<std::process::ExitStatus>),
        Timeout,
        Cancel,
    }
    let outcome = tokio::select! {
        _ = cancel.cancelled() => Outcome::Cancel,
        _ = tokio::time::sleep(timeout) => Outcome::Timeout,
        status = child.wait() => Outcome::Exited(status),
    };
    let (exit_code, timed_out) = match outcome {
        Outcome::Exited(status) => (status.ok().and_then(|s| s.code()), false),
        Outcome::Timeout => {
            kill_tree(&mut child).await;
            (None, true)
        }
        Outcome::Cancel => {
            kill_tree(&mut child).await;
            return Err(AppError::Cancelled);
        }
    };
    let duration_ms = started.elapsed().as_millis() as u64;
    let stdout_bytes = tokio::time::timeout(Duration::from_secs(5), stdout_task)
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let stderr_bytes = tokio::time::timeout(Duration::from_secs(5), stderr_task)
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let stdout = crate::files::decode_output(&stdout_bytes);
    let stderr = crate::files::decode_output(&stderr_bytes);
    let (stdout, stdout_truncated) = truncate_chars(&stdout, 30_000);
    let (stderr, stderr_truncated) = truncate_chars(&stderr, 30_000);
    Ok(CommandOutcome {
        exit_code,
        timed_out,
        stdout,
        stderr,
        duration_ms,
        stdout_truncated,
        stderr_truncated,
    })
}

async fn run_command_tool(
    state: &AppState,
    task: &Task,
    root: &Path,
    args: &Value,
    cancel: &CancellationToken,
    is_test: bool,
) -> AppResult<ToolResult> {
    let mut command = args
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if command.is_empty() && is_test {
        if root.join("package.json").exists() {
            command = "npm test".into();
        } else if root.join("Cargo.toml").exists() {
            command = "cargo test".into();
        } else {
            return Err(AppError::msg("无法自动检测测试命令，请显式提供 command"));
        }
    }
    if command.is_empty() {
        return Err(AppError::msg("command 不能为空"));
    }
    if command.chars().count() > 8000 {
        return Err(AppError::msg("命令过长"));
    }
    if let Some(reason) = crate::policy::dangerous_command_reason(&command, root) {
        return Err(AppError::msg(reason));
    }
    let cwd = if let Some(rel) = args.get("cwd").and_then(|v| v.as_str()) {
        let path = resolve_in_root(root, rel)?;
        if !path.is_dir() {
            return Err(AppError::msg("cwd 必须是目录"));
        }
        path
    } else {
        root.to_path_buf()
    };
    let timeout_secs = args
        .get("timeout_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(120);
    let outcome = execute_shell(&command, &cwd, timeout_secs, cancel).await?;
    let success = outcome.exit_code == Some(0) && !outcome.timed_out;

    let mut chunks = 0;
    for (stream, text) in [("stdout", &outcome.stdout), ("stderr", &outcome.stderr)] {
        if text.is_empty() {
            continue;
        }
        let chars: Vec<char> = text.chars().collect();
        for chunk in chars.chunks(3500) {
            if chunks >= 20 {
                break;
            }
            push_task_event(
                state,
                &task.id,
                "TerminalOutput",
                json!({
                    "stream": stream,
                    "data": chunk.iter().collect::<String>(),
                    "cwd": relative(root, &cwd),
                    "exitCode": outcome.exit_code,
                    "command": command,
                    "source": "agent"
                }),
            );
            chunks += 1;
        }
    }

    if is_test {
        push_task_event(
            state,
            &task.id,
            "TestCompleted",
            json!({
                "success": success,
                "exitCode": outcome.exit_code,
                "timedOut": outcome.timed_out,
                "command": command,
                "durationMs": outcome.duration_ms,
                "stdout": outcome.stdout.chars().take(6000).collect::<String>(),
                "stderr": outcome.stderr.chars().take(6000).collect::<String>()
            }),
        );
    }

    let summary = if outcome.timed_out {
        format!("命令超时：{command}")
    } else {
        format!(
            "命令退出码 {}：{}",
            outcome
                .exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "未知".into()),
            command.chars().take(120).collect::<String>()
        )
    };
    Ok(ToolResult {
        data: json!({
            "command": command,
            "cwd": relative(root, &cwd),
            "exitCode": outcome.exit_code,
            "success": success,
            "timedOut": outcome.timed_out,
            "durationMs": outcome.duration_ms,
            "stdout": outcome.stdout,
            "stderr": outcome.stderr,
            "stdoutTruncated": outcome.stdout_truncated,
            "stderrTruncated": outcome.stderr_truncated
        }),
        summary,
    })
}

#[allow(dead_code)]
pub fn resolve_tool_path(root: &Path, args: &Value) -> AppResult<PathBuf> {
    let rel = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    resolve_in_root(root, rel)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn executes_powershell_and_captures_output() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let outcome = execute_shell(
            "Write-Output 'BASALT_COMMAND_OK'; [Console]::Error.WriteLine('BASALT_ERR')",
            dir.path(),
            30,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(outcome.exit_code, Some(0));
        assert!(outcome.stdout.contains("BASALT_COMMAND_OK"));
        assert!(outcome.stderr.contains("BASALT_ERR"));
        assert!(!outcome.timed_out);
    }

    #[tokio::test]
    async fn cancellation_kills_the_command() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            token.cancel();
        });
        let started = Instant::now();
        let result = execute_shell("Start-Sleep -Seconds 30", dir.path(), 30, &cancel).await;
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
