pub mod prompt;
pub mod subagent;
pub mod tools;
pub mod web;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use rusqlite::params;
use serde_json::{json, Value};
use tauri::Manager;
use tokio::sync::{mpsc::unbounded_channel, oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::changes::list_changes;
use crate::db::{get_project, get_task, load_settings, now};
use crate::error::{AppError, AppResult};
use crate::events::{finish_task, push_task_event, set_task_status};
use crate::files::{canonical_root, truncate_chars};
use crate::messaging;
use crate::models::{
    ChatEvent, Message, Task, CAP_UNSUPPORTED, CAP_VERIFIED, MAX_AGENT_MAX_ITERATIONS,
    TASK_CANCELLED, TASK_COMPLETED, TASK_FAILED, TASK_INTERRUPTED, TASK_QUEUED, TASK_RUNNING,
    TASK_WAITING,
};
use crate::policy::{evaluate, PolicyDecision};
use crate::providers::{
    capabilities_for, chat_adapter, load_chat_runtime, ChatMessage, ChatRequest, ProviderEvent,
    ProviderToolCall,
};
use crate::state::AppState;

#[derive(Default)]
struct AccCall {
    id: Option<String>,
    name: Option<String>,
    args: String,
}

pub fn enqueue(app: tauri::AppHandle, task_id: String) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        schedule_or_queue(&state, &task_id);
    });
}

fn schedule_or_queue(state: &AppState, task_id: &str) {
    let active = {
        let mut guard = state
            .agent_active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *guard {
            true
        } else {
            *guard = true;
            false
        }
    };
    if let Err(err) = set_task_status(state, task_id, TASK_QUEUED) {
        state.log("ERROR", &format!("任务入队失败：{err}"));
        return;
    }
    push_task_event(state, task_id, "TaskQueued", json!({"taskId": task_id}));
    if !active {
        launch(state, task_id);
    }
}

fn launch(state: &AppState, task_id: &str) {
    let app = state.app.clone();
    let id = task_id.to_string();
    let token = CancellationToken::new();
    {
        let mut cancels = state
            .task_cancels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cancels.insert(id.clone(), token.clone());
    }
    tauri::async_runtime::spawn(async move {
        let guard = app.state::<AppState>();
        let state: &AppState = &guard;
        let result = run_task(state, &id, &token).await;
        finalize(state, &id, result);
        {
            let mut cancels = state
                .task_cancels
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            cancels.remove(&id);
        }
        release_next(state);
    });
}

fn release_next(state: &AppState) {
    let next = {
        let mut active = state
            .agent_active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let next: Option<String> = {
            let conn = state.db();
            conn.query_row(
                "SELECT id FROM tasks WHERE status = 'queued' ORDER BY created_at ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok()
        };
        if next.is_none() {
            *active = false;
        }
        next
    };
    if let Some(task_id) = next {
        launch(state, &task_id);
    }
}

fn emit_chat(
    state: &AppState,
    task: &Task,
    kind: &str,
    text: Option<String>,
    message_id: Option<String>,
    message: Option<Message>,
    error: Option<String>,
) {
    crate::events::emit(
        &state.app,
        crate::events::EVENT_CHAT,
        ChatEvent {
            request_id: format!("task:{}", task.id),
            session_id: task.session_id.clone(),
            task_id: Some(task.id.clone()),
            kind: kind.to_string(),
            text,
            message_id,
            message,
            error,
        },
    );
}

async fn run_task(
    state: &AppState,
    task_id: &str,
    cancel: &CancellationToken,
) -> AppResult<Option<String>> {
    let (task, project_path, settings) = {
        let conn = state.db();
        let task = get_task(&conn, task_id)?;
        let project = get_project(&conn, &task.project_id)?;
        let settings = load_settings(&conn);
        (task, project.path, settings)
    };
    let root = canonical_root(Path::new(&project_path))?;
    let rt = load_chat_runtime(state, &task.provider_id)?;
    let model = if task.model.trim().is_empty() {
        rt.provider
            .default_chat_model
            .clone()
            .ok_or_else(|| AppError::msg("未指定模型"))?
    } else {
        task.model.clone()
    };
    let capabilities = capabilities_for(&rt, &model);
    if let Some(caps) = &capabilities {
        if caps.tools == CAP_UNSUPPORTED {
            return Err(AppError::msg(format!(
                "模型 {model} 被标记为不支持工具调用，无法在 Agent 模式下使用。请在供应商能力矩阵中确认能力，或切换支持工具调用的模型。"
            )));
        }
    }

    set_task_status(state, task_id, TASK_RUNNING)?;
    push_task_event(
        state,
        task_id,
        "TaskStarted",
        json!({
            "taskId": task.id,
            "providerId": task.provider_id,
            "model": model,
            "projectId": task.project_id,
            "networkNote": "真实供应商"
        }),
    );

    let include_images = capabilities
        .as_ref()
        .map(|caps| caps.vision != CAP_UNSUPPORTED)
        .unwrap_or(true);
    let mut messages: Vec<ChatMessage> = Vec::new();
    messages.push(ChatMessage::text("system", prompt::system_prompt(&root)));
    let prior_tools = prior_tool_summary(state, task_id);
    if !prior_tools.is_empty() {
        messages.push(ChatMessage::text(
            "system",
            format!(
                "该任务此前已执行过以下工具调用，不要自动重复执行，除非确有需要：\n{prior_tools}"
            ),
        ));
    }
    let history = messaging::history_for_model(state, &task.session_id, 40, include_images)?;
    let goal_present = history
        .iter()
        .rev()
        .any(|message| message.role == "user" && message.content.contains(task.goal.trim()));
    messages.extend(history);
    if !goal_present {
        messages.push(ChatMessage::text("user", task.goal.clone()));
    }

    let tools = tools::schemas();
    let max_iterations = settings
        .agent_max_iterations
        .clamp(1, MAX_AGENT_MAX_ITERATIONS) as usize;
    let mut last_text = String::new();

    for iteration in 0..max_iterations {
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        let message_id = Uuid::new_v4().to_string();
        let request = ChatRequest {
            model: model.clone(),
            messages: messages.clone(),
            tools: tools.clone(),
            max_tokens: None,
            temperature: None,
            reasoning_effort: if capabilities
                .as_ref()
                .is_some_and(|caps| caps.reasoning == crate::models::CAP_UNSUPPORTED)
            {
                None
            } else {
                task.reasoning_effort.clone()
            },
        };
        let adapter = chat_adapter(&rt.provider.protocol)?;
        let client = state.http.clone();
        let rt_for = rt.clone();
        let req_for = request;
        let token = cancel.clone();
        let (tx, mut rx) = unbounded_channel::<ProviderEvent>();
        let handle = tauri::async_runtime::spawn(async move {
            adapter.chat(&client, &rt_for, &req_for, &tx, &token).await
        });

        let mut text = String::new();
        let mut calls: BTreeMap<usize, AccCall> = BTreeMap::new();
        while let Some(event) = rx.recv().await {
            match event {
                ProviderEvent::TextDelta(delta) => {
                    text.push_str(&delta);
                    emit_chat(
                        state,
                        &task,
                        "delta",
                        Some(delta),
                        Some(message_id.clone()),
                        None,
                        None,
                    );
                }
                ProviderEvent::ToolCallDelta {
                    index,
                    id,
                    name,
                    args_delta,
                } => {
                    let acc = calls.entry(index).or_default();
                    if let Some(id) = id {
                        acc.id = Some(id);
                    }
                    if let Some(name) = name {
                        acc.name = Some(name);
                    }
                    acc.args.push_str(&args_delta);
                }
                ProviderEvent::Usage { .. } => {}
                ProviderEvent::Done { .. } => {}
            }
        }
        let provider_result = handle
            .await
            .map_err(|e| AppError::msg(format!("模型任务执行异常：{e}")))?;
        if let Err(err) = provider_result {
            if !text.is_empty() {
                persist_assistant(state, &task, &message_id, &text, &model).ok();
            }
            if err.is_cancelled() {
                emit_chat(
                    state,
                    &task,
                    "cancelled",
                    None,
                    Some(message_id),
                    None,
                    Some("已停止生成".into()),
                );
                return Err(AppError::Cancelled);
            }
            emit_chat(
                state,
                &task,
                "error",
                None,
                Some(message_id),
                None,
                Some(err.user_message()),
            );
            return Err(err);
        }
        if cancel.is_cancelled() {
            if !text.is_empty() {
                persist_assistant(state, &task, &message_id, &text, &model).ok();
            }
            emit_chat(
                state,
                &task,
                "cancelled",
                None,
                Some(message_id),
                None,
                Some("任务已取消".into()),
            );
            return Err(AppError::Cancelled);
        }

        let mut updates = vec![("chat", CAP_VERIFIED)];
        if rt.provider.stream {
            updates.push(("stream", CAP_VERIFIED));
        }
        let _ = crate::providers::mark_capabilities(
            state,
            &task.provider_id,
            &model,
            &updates,
        );

        let mut assistant_calls = Vec::new();
        for (index, acc) in calls {
            let Some(name) = acc.name else { continue };
            let id = acc
                .id
                .unwrap_or_else(|| format!("call_{}_{}", &task.id[..task.id.len().min(8)], index));
            let raw = acc.args.trim();
            let arguments: Value = if raw.is_empty() {
                json!({})
            } else {
                serde_json::from_str(raw).unwrap_or_else(|_| json!({"__raw": raw}))
            };
            assistant_calls.push(ProviderToolCall {
                id,
                name,
                arguments,
            });
        }

        if !text.trim().is_empty() {
            last_text = text.clone();
            if let Ok(message) = persist_assistant(state, &task, &message_id, &text, &model) {
                emit_chat(
                    state,
                    &task,
                    "message",
                    None,
                    Some(message_id.clone()),
                    Some(message),
                    None,
                );
            }
        }

        if assistant_calls.is_empty() {
            return Ok(Some(if last_text.trim().is_empty() {
                format!(
                    "任务执行完成（第 {} 轮）。模型未输出文本总结，请查看时间线中的真实工具结果。",
                    iteration + 1
                )
            } else {
                last_text
            }));
        }

        messages.push(ChatMessage {
            role: "assistant".into(),
            content: text.clone(),
            images: Vec::new(),
            tool_calls: assistant_calls.clone(),
            tool_call_id: None,
            tool_name: None,
        });

        for call in assistant_calls {
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            let tool_content = execute_tool_call(
                state,
                &task,
                &root,
                &call,
                cancel,
                &settings,
            )
            .await?;
            messages.push(ChatMessage {
                role: "tool".into(),
                content: tool_content,
                images: Vec::new(),
                tool_calls: Vec::new(),
                tool_call_id: Some(call.id),
                tool_name: Some(call.name),
            });
        }
    }

    Err(AppError::RoundLimit(format!(
        "已达到安全执行上限 {max_iterations} 轮。任务进度与变更已保留，可在设置中提高上限后点击「继续」接着执行。"
    )))
}

fn persist_assistant(
    state: &AppState,
    task: &Task,
    message_id: &str,
    text: &str,
    model: &str,
) -> AppResult<Message> {
    messaging::add_message(
        state,
        Some(message_id),
        &task.session_id,
        "assistant",
        text,
        "text",
        &[],
        Some(&task.id),
        Some(model),
    )
}

fn prior_tool_summary(state: &AppState, task_id: &str) -> String {
    let conn = state.db();
    let mut stmt = conn
        .prepare(
            "SELECT name, status FROM tool_calls WHERE task_id = ?1 ORDER BY created_at ASC LIMIT 16",
        )
        .ok();
    let Some(stmt) = stmt.as_mut() else {
        return String::new();
    };
    let rows = stmt.query_map([task_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
        ))
    });
    let Ok(rows) = rows else {
        return String::new();
    };
    rows.filter_map(Result::ok)
        .map(|(name, status)| format!("- {name}: {status}"))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn execute_tool_call(
    state: &AppState,
    task: &Task,
    root: &Path,
    call: &ProviderToolCall,
    cancel: &CancellationToken,
    settings: &crate::models::Settings,
) -> AppResult<String> {
    let call_db_id = Uuid::new_v4().to_string();
    let created_at = now();
    let decision = evaluate(&call.name, &call.arguments, root, settings);
    let requires_approval = matches!(decision, PolicyDecision::NeedsApproval(_));
    {
        let conn = state.db();
        conn.execute(
            "INSERT INTO tool_calls(id, task_id, name, args_json, status, requires_approval, created_at)
             VALUES(?1, ?2, ?3, ?4, 'pending', ?5, ?6)",
            params![
                call_db_id,
                task.id,
                call.name,
                call.arguments.to_string(),
                requires_approval as i64,
                created_at
            ],
        )?;
    }
    push_task_event(
        state,
        &task.id,
        "ToolCallStarted",
        json!({
            "callId": call_db_id,
            "toolCallId": call.id,
            "name": call.name,
            "args": call.arguments,
            "requiresApproval": requires_approval
        }),
    );

    let denial = match &decision {
        PolicyDecision::Deny(reason) => Some(reason.clone()),
        _ => None,
    };
    if let Some(reason) = denial {
        let content = serde_json::to_string(&json!({"ok": false, "deniedByPolicy": true, "error": reason}))?;
        finish_tool_call(
            state,
            task,
            &call_db_id,
            &call.name,
            "denied",
            Some(&call.id),
            &json!({"ok": false, "error": reason}),
            &format!("策略拒绝：{}", reason.chars().take(300).collect::<String>()),
        );
        return Ok(content);
    }

    if let PolicyDecision::NeedsApproval(reason) = &decision {
        set_task_status(state, &task.id, TASK_WAITING)?;
        {
            let conn = state.db();
            let _ = conn.execute(
                "UPDATE tool_calls SET status = 'waiting_approval' WHERE id = ?1",
                [call_db_id.as_str()],
            );
        }
        let (approval_tx, approval_rx) = oneshot::channel::<bool>();
        {
            let mut approvals = state
                .approvals
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            approvals.insert(call_db_id.clone(), approval_tx);
        }
        push_task_event(
            state,
            &task.id,
            "ApprovalRequired",
            json!({
                "callId": call_db_id,
                "toolCallId": call.id,
                "name": call.name,
                "args": call.arguments,
                "reason": reason
            }),
        );
        enum ApprovalOutcome {
            Decided(bool),
            Cancelled,
            Timeout,
        }
        let outcome = tokio::select! {
            _ = cancel.cancelled() => ApprovalOutcome::Cancelled,
            result = approval_rx => ApprovalOutcome::Decided(result.unwrap_or(false)),
            _ = tokio::time::sleep(Duration::from_secs(900)) => ApprovalOutcome::Timeout,
        };
        {
            let mut approvals = state
                .approvals
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            approvals.remove(&call_db_id);
        }
        if matches!(outcome, ApprovalOutcome::Cancelled) {
            let conn = state.db();
            let _ = conn.execute(
                "UPDATE tool_calls SET status = 'cancelled', completed_at = ?2 WHERE id = ?1",
                params![call_db_id, now()],
            );
            return Err(AppError::Cancelled);
        }
        let approved = matches!(outcome, ApprovalOutcome::Decided(true));
        push_task_event(
            state,
            &task.id,
            "ApprovalResolved",
            json!({
                "callId": call_db_id,
                "approved": approved,
                "timedOut": matches!(outcome, ApprovalOutcome::Timeout)
            }),
        );
        if !approved {
            set_task_status(state, &task.id, TASK_RUNNING)?;
            let reason_text = if matches!(outcome, ApprovalOutcome::Timeout) {
                "审批等待超时，操作未执行"
            } else {
                "用户拒绝执行该工具调用"
            };
            let content = serde_json::to_string(&json!({"ok": false, "denied": true, "error": reason_text}))?;
            finish_tool_call(
                state,
                task,
                &call_db_id,
                &call.name,
                "denied",
                Some(&call_db_id),
                &json!({"ok": false, "error": reason_text}),
                reason_text,
            );
            return Ok(content);
        }
        set_task_status(state, &task.id, TASK_RUNNING)?;
    }

    {
        let conn = state.db();
        let _ = conn.execute(
            "UPDATE tool_calls SET status = 'running' WHERE id = ?1",
            [call_db_id.as_str()],
        );
    }
    let started = Instant::now();
    let result = tools::execute(state, task, root, &call.name, &call.arguments, cancel).await;
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(outcome) => {
            let data_json = serde_json::to_string(&outcome.data).unwrap_or_default();
            let (preview, _) = truncate_chars(&data_json, 70_000);
            let model_content =
                serde_json::to_string(&json!({"ok": true, "data": outcome.data}))?;
            let preview_short: String = preview.chars().take(5000).collect();
            finish_tool_call(
                state,
                task,
                &call_db_id,
                &call.name,
                "completed",
                Some(&call.id),
                &json!({"ok": true, "data": preview_short, "summary": outcome.summary, "durationMs": duration_ms}),
                &outcome.summary,
            );
            let _ = crate::providers::mark_capabilities(
                state,
                &task.provider_id,
                &task.model,
                &[("tools", CAP_VERIFIED)],
            );
            Ok(model_content)
        }
        Err(err) if err.is_cancelled() => {
            finish_tool_call(
                state,
                task,
                &call_db_id,
                &call.name,
                "cancelled",
                Some(&call.id),
                &json!({"ok": false, "error": "操作已取消"}),
                "操作已取消",
            );
            Err(AppError::Cancelled)
        }
        Err(err) => {
            let message = err.user_message();
            let model_content = serde_json::to_string(&json!({"ok": false, "error": message}))?;
            finish_tool_call(
                state,
                task,
                &call_db_id,
                &call.name,
                "failed",
                Some(&call.id),
                &json!({"ok": false, "error": message}),
                &message,
            );
            Ok(model_content)
        }
    }
}

fn finish_tool_call(
    state: &AppState,
    task: &Task,
    call_db_id: &str,
    name: &str,
    status: &str,
    provider_call_id: Option<&str>,
    result: &Value,
    summary: &str,
) {
    {
        let conn = state.db();
        let _ = conn.execute(
            "UPDATE tool_calls SET status = ?2, result_json = ?3, completed_at = ?4 WHERE id = ?1",
            params![call_db_id, status, result.to_string(), now()],
        );
    }
    push_task_event(
        state,
        &task.id,
        "ToolCallCompleted",
        json!({
            "callId": call_db_id,
            "toolCallId": provider_call_id,
            "name": name,
            "status": status,
            "result": result,
            "summary": summary
        }),
    );
}

fn finalize(state: &AppState, task_id: &str, result: AppResult<Option<String>>) {
    let task = {
        let conn = state.db();
        get_task(&conn, task_id).ok()
    };
    let Some(task) = task else { return };
    {
        let mut approvals = state
            .approvals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        approvals.retain(|_, sender| !sender.is_closed());
    }
    match result {
        Ok(Some(report)) => {
            let report = augment_report(state, &task, &report);
            finish_task(state, task_id, TASK_COMPLETED, Some(&report), None);
        }
        Ok(None) => {
            finish_task(
                state,
                task_id,
                TASK_COMPLETED,
                Some("任务执行完成。"),
                None,
            );
        }
        Err(err) if err.is_cancelled() => {
            let conn = state.db();
            let _ = conn.execute(
                "UPDATE tool_calls SET status = 'cancelled', completed_at = ?2 WHERE task_id = ?1 AND status IN ('pending', 'waiting_approval', 'running')",
                params![task_id, now()],
            );
            drop(conn);
            finish_task(
                state,
                task_id,
                TASK_CANCELLED,
                Some("任务已由用户取消。未完成的工具调用不会再执行。"),
                Some("任务已取消"),
            );
        }
        Err(err) if err.is_round_limit() => {
            let conn = state.db();
            let _ = conn.execute(
                "UPDATE tool_calls SET status = 'cancelled', completed_at = ?2 WHERE task_id = ?1 AND status IN ('pending', 'waiting_approval', 'running')",
                params![task_id, now()],
            );
            drop(conn);
            finish_task(
                state,
                task_id,
                TASK_INTERRUPTED,
                Some("已达到安全执行上限，进度与变更已保留。提高上限后点击「继续」可以接着执行。"),
                Some(&err.user_message()),
            );
        }
        Err(err) => {
            finish_task(
                state,
                task_id,
                TASK_FAILED,
                None,
                Some(&err.user_message()),
            );
        }
    }
}

fn augment_report(state: &AppState, task: &Task, base: &str) -> String {
    let changes = list_changes(state, Some(&task.id), None).unwrap_or_default();
    let mut out = base.trim().to_string();
    if !changes.is_empty() {
        out.push_str("\n\n真实变更记录：\n");
        for change in changes.iter().take(30) {
            out.push_str(&format!("- {} [{}]\n", change.path, change.status));
        }
    }
    let test_note = {
        let conn = state.db();
        conn.query_row(
            "SELECT payload_json FROM task_events WHERE task_id = ?1 AND event_type = 'TestCompleted' ORDER BY id DESC LIMIT 1",
            [&task.id],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
    };
    if let Some(test) = test_note {
        out.push_str(&format!(
            "\n最近一次测试：{} {}（退出码 {}）",
            test.get("command").and_then(|v| v.as_str()).unwrap_or(""),
            if test.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                "通过"
            } else {
                "未通过"
            },
            test
                .get("exitCode")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "未知".into())
        ));
    }
    out
}
