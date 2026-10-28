use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use tauri::State;
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::changes::list_changes;
use crate::db::{
    get_project, get_provider_row, get_task, insert_message, load_settings, now, row_to_session,
    row_to_task, row_to_tool_call,
};
use crate::error::{AppError, AppResult};
use crate::events::{finish_task, push_task_event};
use crate::files::truncate_chars;
use crate::models::{
    Task, TaskDetail, TaskEvent, CAP_UNSUPPORTED, CAP_VERIFIED, TASK_CANCELLED, TASK_COMPLETED,
    TASK_QUEUED, TASK_RUNNING, TASK_WAITING,
};
use crate::providers::{
    chat_adapter, load_chat_runtime, mark_capabilities, ChatMessage, ChatRequest, ProviderEvent,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCreateInput {
    pub project_id: String,
    pub session_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub goal: String,
    pub provider_id: String,
    pub model: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
}

fn derive_title(goal: &str) -> String {
    let first = goal
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("新任务");
    first.chars().take(48).collect()
}

#[tauri::command]
pub fn task_create(state: State<'_, AppState>, input: TaskCreateInput) -> AppResult<Task> {
    let goal = input.goal.trim().to_string();
    if goal.is_empty() {
        return Err(AppError::msg("任务目标不能为空"));
    }
    if goal.len() > 20_000 {
        return Err(AppError::msg("任务目标过长"));
    }
    let (session, provider) = {
        let conn = state.db();
        get_project(&conn, &input.project_id)?;
        let session = conn
            .query_row(
                "SELECT * FROM sessions WHERE id = ?1",
                [&input.session_id],
                row_to_session,
            )
            .map_err(|_| AppError::msg("会话不存在"))?;
        if let Some(project_id) = session.project_id.as_deref() {
            if project_id != input.project_id {
                return Err(AppError::msg("会话与所选项目不匹配"));
            }
        }
        let provider = get_provider_row(&conn, &input.provider_id)?;
        (session, provider)
    };
    if provider.category != "chat" || provider.is_template || !provider.enabled {
        return Err(AppError::msg("所选供应商不可用于 Agent 任务"));
    }
    let model = if input.model.trim().is_empty() {
        provider
            .default_chat_model
            .clone()
            .ok_or_else(|| AppError::msg("未选择模型"))?
    } else {
        input.model.trim().to_string()
    };
    let model_caps = provider
        .models
        .iter()
        .find(|entry| entry.id == model)
        .map(|entry| entry.capabilities.clone());
    if let Some(model_caps) = &model_caps {
        if model_caps.tools == CAP_UNSUPPORTED {
            return Err(AppError::msg(format!(
                "模型 {model} 标记为不支持工具调用，无法创建 Agent 任务。"
            )));
        }
    }

    let task_id = Uuid::new_v4().to_string();
    let timestamp = now();
    let reasoning_effort = if model_caps
        .as_ref()
        .is_some_and(|caps| caps.reasoning == CAP_UNSUPPORTED)
    {
        None
    } else {
        crate::models::normalize_reasoning_effort(
            input
                .reasoning_effort
                .as_deref()
                .or(session.reasoning_effort.as_deref()),
        )
    };
    let title = input
        .title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| derive_title(&goal));
    let session_title = session.title.clone();
    {
        let conn = state.db();
        let tx = conn.unchecked_transaction()?;
        let message_id = Uuid::new_v4().to_string();
        insert_message(
            &tx,
            &message_id,
            &input.session_id,
            "user",
            &goal,
            "text",
            &[],
            Some(&task_id),
            Some(&model),
        )?;
        if session_title == "新会话" || session_title.trim().is_empty() {
            tx.execute(
                "UPDATE sessions SET title = ?2, provider_id = ?3, model = ?4, mode = 'agent', updated_at = ?5 WHERE id = ?1",
                params![input.session_id, derive_title(&goal), input.provider_id, model, timestamp],
            )?;
        } else {
            tx.execute(
                "UPDATE sessions SET provider_id = ?2, model = ?3, mode = 'agent', updated_at = ?4 WHERE id = ?1",
                params![input.session_id, input.provider_id, model, timestamp],
            )?;
        }
        tx.execute(
            "INSERT INTO tasks(id, project_id, session_id, title, goal, status, provider_id, model,
             reasoning_effort, plan_json, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '[]', ?10, ?10)",
            params![
                task_id,
                input.project_id,
                input.session_id,
                title,
                goal,
                TASK_QUEUED,
                input.provider_id,
                model,
                reasoning_effort,
                timestamp
            ],
        )?;
        tx.commit()?;
    }
    push_task_event(
        &state,
        &task_id,
        "TaskCreated",
        json!({
            "taskId": task_id,
            "title": title,
            "goal": goal,
            "sessionId": input.session_id,
            "projectId": input.project_id,
            "providerId": input.provider_id,
            "model": model
        }),
    );
    let conn = state.db();
    get_task(&conn, &task_id)
}

#[tauri::command]
pub fn task_list(state: State<'_, AppState>) -> AppResult<Vec<Task>> {
    let conn = state.db();
    let mut stmt = conn.prepare("SELECT * FROM tasks ORDER BY created_at DESC LIMIT 300")?;
    let rows = stmt.query_map([], row_to_task)?;
    let collected = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(collected)
}

#[tauri::command]
pub fn task_get(state: State<'_, AppState>, task_id: String) -> AppResult<TaskDetail> {
    let conn = state.db();
    let task = get_task(&conn, &task_id)?;
    let mut events_stmt = conn.prepare(
        "SELECT id, task_id, event_type, payload_json, created_at FROM task_events WHERE task_id = ?1 ORDER BY id ASC",
    )?;
    let events = events_stmt
        .query_map([&task_id], |row| {
            let payload_raw: String = row.get(3)?;
            Ok(TaskEvent {
                id: row.get(0)?,
                task_id: row.get(1)?,
                event_type: row.get(2)?,
                payload: serde_json::from_str(&payload_raw).unwrap_or(json!({})),
                created_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut calls_stmt =
        conn.prepare("SELECT * FROM tool_calls WHERE task_id = ?1 ORDER BY created_at ASC")?;
    let tool_calls = calls_stmt
        .query_map([&task_id], row_to_tool_call)?
        .collect::<Result<Vec<_>, _>>()?;
    let changes = crate::changes::list_changes_conn(&conn, Some(&task_id), None)?;
    Ok(TaskDetail {
        task,
        events,
        tool_calls,
        changes,
    })
}

#[tauri::command]
pub fn task_start(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    task_id: String,
) -> AppResult<()> {
    let task = {
        let conn = state.db();
        let task = get_task(&conn, &task_id)?;
        let project = get_project(&conn, &task.project_id)?;
        if !std::path::Path::new(&project.path).is_dir() {
            return Err(AppError::msg("项目目录不存在，无法执行任务"));
        }
        let provider = get_provider_row(&conn, &task.provider_id)?;
        if !provider.enabled || provider.is_template {
            return Err(AppError::msg("任务使用的供应商已禁用或不存在"));
        }
        task
    };
    if task.status == TASK_RUNNING || task.status == TASK_WAITING {
        return Err(AppError::msg("任务已在运行中"));
    }
    if task.status == TASK_COMPLETED {
        return Err(AppError::msg("任务已完成。如需继续，请在同一会话中创建新任务。"));
    }
    crate::agent::enqueue(app, task_id);
    Ok(())
}

#[tauri::command]
pub fn task_cancel(state: State<'_, AppState>, task_id: String) -> AppResult<bool> {
    let task = {
        let conn = state.db();
        get_task(&conn, &task_id)?
    };
    if task.status == TASK_QUEUED {
        finish_task(
            &state,
            &task_id,
            TASK_CANCELLED,
            Some("任务在开始执行前被取消。"),
            None,
        );
        return Ok(true);
    }
    if task.status != TASK_RUNNING && task.status != TASK_WAITING {
        return Err(AppError::msg("当前任务状态不可取消"));
    }
    let token = {
        let cancels = state
            .task_cancels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cancels.get(&task_id).cloned()
    };
    if let Some(token) = token {
        token.cancel();
    } else {
        finish_task(&state, &task_id, TASK_CANCELLED, None, Some("任务已取消"));
    }
    Ok(true)
}

#[tauri::command]
pub async fn review_task(state: State<'_, AppState>, task_id: String) -> AppResult<String> {
    let (task, settings) = {
        let conn = state.db();
        (get_task(&conn, &task_id)?, load_settings(&conn))
    };
    let route = if settings.routing.review.is_set() {
        settings.routing.review.clone()
    } else if settings.routing.agent_execute.is_set() {
        settings.routing.agent_execute.clone()
    } else {
        settings.routing.chat.clone()
    };
    if !route.is_set() {
        return Err(AppError::msg(
            "没有可用的审查模型。请在设置中配置代码审查路由或聊天路由。",
        ));
    }
    let rt = load_chat_runtime(&state, &route.provider_id)?;
    let model = if route.model.trim().is_empty() {
        rt.provider
            .default_chat_model
            .clone()
            .ok_or_else(|| AppError::msg("审查路由未指定模型"))?
    } else {
        route.model.clone()
    };
    let changes = list_changes(&state, Some(&task_id), None)?;
    if changes.is_empty() {
        return Err(AppError::msg("该任务还没有文件变更可供审查"));
    }
    let mut material = String::new();
    for change in changes.iter().take(20) {
        let diff = crate::changes::change_diff(&state, &change.id)?;
        if diff.binary {
            continue;
        }
        let (old_text, _) = truncate_chars(&diff.old_text, 12_000);
        let (new_text, _) = truncate_chars(&diff.new_text, 12_000);
        material.push_str(&format!(
            "\n### {}\n状态：{}\n--- 变更前 ---\n{}\n--- 当前 ---\n{}\n",
            change.path, change.status, old_text, new_text
        ));
        if material.len() > 60_000 {
            break;
        }
    }
    if material.trim().is_empty() {
        return Err(AppError::msg("变更内容为空或全部为二进制文件"));
    }
    let request = ChatRequest {
        model: model.clone(),
        messages: vec![
            ChatMessage::text(
                "system",
                "你是 Basalt 的代码审查模型。请只基于给出的真实文件变更进行审查，\
                 不要假设你执行过任何命令。指出正确性、安全、边界条件和测试缺口，\
                 按严重程度排序，给出可操作的修改建议。使用中文和 Markdown。",
            ),
            ChatMessage::text(
                "user",
                format!(
                    "任务目标：{}\n\n需要审查的文件变更：\n{}\n\n请输出：1) 主要风险 2) 具体建议 3) 需要补充的验证。",
                    task.goal, material
                ),
            ),
        ],
        tools: Vec::new(),
        max_tokens: None,
        temperature: None,
        reasoning_effort: None,
    };
    let adapter = chat_adapter(&rt.provider.protocol)?;
    let client = state.http.clone();
    let token = CancellationToken::new();
    let (tx, mut rx) = unbounded_channel::<ProviderEvent>();
    let handle = tauri::async_runtime::spawn(async move {
        adapter.chat(&client, &rt, &request, &tx, &token).await
    });
    let mut text = String::new();
    while let Some(event) = rx.recv().await {
        if let ProviderEvent::TextDelta(delta) = event {
            text.push_str(&delta);
        }
    }
    handle
        .await
        .map_err(|e| AppError::msg(format!("审查任务异常：{e}")))??;
    if text.trim().is_empty() {
        return Err(AppError::msg("审查模型没有返回文本内容"));
    }
    let _ = mark_capabilities(
        &state,
        &route.provider_id,
        &model,
        &[("chat", CAP_VERIFIED), ("stream", CAP_VERIFIED)],
    );
    Ok(text)
}

#[tauri::command]
pub fn task_delete(state: State<'_, AppState>, task_id: String) -> AppResult<()> {
    let conn = state.db();
    let task = get_task(&conn, &task_id)?;
    if task.status == TASK_RUNNING || task.status == TASK_WAITING {
        return Err(AppError::msg("运行中的任务不能删除"));
    }
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM changes WHERE task_id = ?1 AND status IN ('pending', 'conflict')",
        [&task_id],
        |row| row.get(0),
    )?;
    if pending > 0 {
        return Err(AppError::msg("存在未处理的文件变更，请先接受或撤销后再删除任务"));
    }
    conn.execute("DELETE FROM task_events WHERE task_id = ?1", [&task_id])?;
    conn.execute("DELETE FROM tool_calls WHERE task_id = ?1", [&task_id])?;
    conn.execute("DELETE FROM changes WHERE task_id = ?1", [&task_id])?;
    conn.execute("UPDATE messages SET task_id = NULL WHERE task_id = ?1", [&task_id])?;
    conn.execute("DELETE FROM tasks WHERE id = ?1", [&task_id])?;
    Ok(())
}
