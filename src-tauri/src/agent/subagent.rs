use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::{Task, CAP_UNSUPPORTED};
use crate::providers::{
    capabilities_for, chat_adapter, load_chat_runtime, ChatMessage, ChatRequest, ProviderEvent,
    ProviderToolCall,
};
use crate::state::AppState;

use super::tools::{self, ToolResult};

#[derive(Default)]
struct AccCall {
    id: Option<String>,
    name: Option<String>,
    args: String,
}

fn role_prompt(role: &str) -> &'static str {
    match role {
        "reviewer" => "代码审查员：寻找正确性、安全、边界条件和测试缺口，按严重程度排序。",
        "planner" => "方案规划师：比较可选方案、约束和风险，给出可执行的分步建议。",
        _ => "研究助手：收集证据、核对事实、整理来源，给出可复核的结论。",
    }
}

pub async fn run(
    state: &AppState,
    task: &Task,
    root: &Path,
    args: &Value,
    cancel: &CancellationToken,
) -> AppResult<ToolResult> {
    let sub_task = args
        .get("task")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if sub_task.is_empty() {
        return Err(AppError::msg("spawn_subagent 需要非空 task"));
    }
    if sub_task.chars().count() > 4000 {
        return Err(AppError::msg("子代理任务过长"));
    }
    let role: String = args
        .get("role")
        .and_then(|value| value.as_str())
        .unwrap_or("researcher")
        .chars()
        .take(40)
        .collect();
    let context = args
        .get("context")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();

    let rt = load_chat_runtime(state, &task.provider_id)?;
    let model = if task.model.trim().is_empty() {
        rt.provider
            .default_chat_model
            .clone()
            .ok_or_else(|| AppError::msg("子代理没有可用模型"))?
    } else {
        task.model.clone()
    };
    if let Some(caps) = capabilities_for(&rt, &model) {
        if caps.tools == CAP_UNSUPPORTED {
            return Err(AppError::msg("当前模型不支持工具调用，无法启动子代理"));
        }
    }

    let system = format!(
        "你是 Basalt 的只读子代理。\n角色：{}\n工作区：{}\n主任务目标：{}\n\
         你只能读取项目文件、查看 Git 状态与 Diff、联网搜索和读取公开网页；\
         你不能修改文件、执行命令、请求审批或再启动子代理。\n\
         围绕给你的子任务独立调查，最后返回结构化中文报告：结论、证据（文件路径或 URL）、风险与建议。\
         不要编造未实际读取或搜索到的内容；找不到证据时明确说明。",
        role_prompt(&role),
        root.to_string_lossy(),
        task.goal.chars().take(1200).collect::<String>()
    );
    let mut user = format!("子任务：{sub_task}");
    if !context.is_empty() {
        user.push_str("\n\n补充上下文：");
        user.push_str(&context.chars().take(4000).collect::<String>());
    }
    let mut messages = vec![
        ChatMessage::text("system", system),
        ChatMessage::text("user", user),
    ];
    let tool_schemas = tools::schemas_readonly();
    let mut last_text = String::new();
    let mut tool_count = 0usize;
    let mut round = 0usize;

    loop {
        round += 1;
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        let request = ChatRequest {
            model: model.clone(),
            messages: messages.clone(),
            tools: tool_schemas.clone(),
            max_tokens: None,
            temperature: None,
            reasoning_effort: task.reasoning_effort.clone(),
        };
        let adapter = chat_adapter(&rt.provider.protocol)?;
        let client = state.http.clone();
        let rt_for = rt.clone();
        let token = cancel.clone();
        let (tx, mut rx) = unbounded_channel::<ProviderEvent>();
        let handle = tauri::async_runtime::spawn(async move {
            adapter.chat(&client, &rt_for, &request, &tx, &token).await
        });
        let mut text = String::new();
        let mut calls: BTreeMap<usize, AccCall> = BTreeMap::new();
        while let Some(event) = rx.recv().await {
            match event {
                ProviderEvent::TextDelta(delta) => text.push_str(&delta),
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
                ProviderEvent::Usage { .. } | ProviderEvent::Done { .. } => {}
            }
        }
        handle
            .await
            .map_err(|e| AppError::msg(format!("子代理执行异常：{e}")))??;
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        if !text.trim().is_empty() {
            last_text = text.clone();
        }

        let mut assistant_calls = Vec::new();
        for (index, acc) in calls {
            let Some(name) = acc.name else { continue };
            let id = acc
                .id
                .unwrap_or_else(|| format!("sub_{}_{}", round, index));
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

        if assistant_calls.is_empty() {
            let report = if last_text.trim().is_empty() {
                "子代理没有返回文本报告。".to_string()
            } else {
                last_text
            };
            return Ok(result(role, sub_task, report, round, tool_count));
        }

        messages.push(ChatMessage {
            role: "assistant".into(),
            content: text,
            images: Vec::new(),
            tool_calls: assistant_calls.clone(),
            tool_call_id: None,
            tool_name: None,
        });
        for call in assistant_calls {
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            tool_count += 1;
            let output = match tools::execute_readonly(
                state,
                task,
                root,
                &call.name,
                &call.arguments,
                cancel,
            )
            .await
            {
                Ok(outcome) => serde_json::to_string(&json!({
                    "ok": true,
                    "summary": outcome.summary,
                    "data": outcome.data
                }))?,
                Err(err) if err.is_cancelled() => return Err(AppError::Cancelled),
                Err(err) => serde_json::to_string(&json!({
                    "ok": false,
                    "error": err.user_message()
                }))?,
            };
            let (output, _) = crate::files::truncate_chars(&output, 24_000);
            messages.push(ChatMessage {
                role: "tool".into(),
                content: output,
                images: Vec::new(),
                tool_calls: Vec::new(),
                tool_call_id: Some(call.id),
                tool_name: Some(call.name),
            });
        }
    }
}

fn result(
    role: String,
    task: String,
    report: String,
    rounds: usize,
    tool_count: usize,
) -> ToolResult {
    let (report, truncated) = crate::files::truncate_chars(&report, 16_000);
    let summary = format!(
        "子代理（{role}）完成 · {rounds} 轮 · {tool_count} 次工具调用"
    );
    ToolResult {
        data: json!({
            "role": role,
            "task": task,
            "report": report,
            "rounds": rounds,
            "toolCalls": tool_count,
            "truncated": truncated
        }),
        summary,
    }
}
