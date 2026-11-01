use std::path::Path;

use futures_util::stream::{self, StreamExt};
use rusqlite::params;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::db::{load_settings, now};
use crate::error::{AppError, AppResult};
use crate::events::push_task_event;
use crate::models::{Route, Task, CAP_UNSUPPORTED};
use crate::providers::{
    capabilities_for, load_chat_runtime, ChatMessage, ChatRequest, ProviderRuntime,
};
use crate::state::AppState;

use super::round::{record_usage, run_round};
use super::tools::{self, ToolResult};

const MAX_PARALLEL: usize = 6;
const CONCURRENCY: usize = 4;

fn role_prompt(role: &str) -> &'static str {
    match role {
        "reviewer" => {
            "代码审查员：寻找正确性、安全、边界条件和测试缺口，按严重程度排序，给出可执行的修改建议。"
        }
        "planner" => "方案规划师：比较可选方案、约束和风险，给出可执行的分步建议与取舍。",
        "verifier" => {
            "验证员：核对实现是否满足目标，寻找可复现的问题、缺失的测试和未验证的假设。"
        }
        _ => "研究助手：收集证据、核对事实、整理来源，给出可复核的结论。",
    }
}

fn normalize_role(role: &str) -> String {
    match role {
        "reviewer" | "planner" | "verifier" => role.to_string(),
        _ => "researcher".to_string(),
    }
}

fn resolve_route(state: &AppState, task: &Task, role: &str) -> AppResult<(ProviderRuntime, String)> {
    let settings = {
        let conn = state.db();
        load_settings(&conn)
    };
    let candidates: Vec<Route> = match role {
        "reviewer" => vec![
            settings.routing.review.clone(),
            settings.routing.agent_execute.clone(),
            settings.routing.chat.clone(),
        ],
        "planner" => vec![
            settings.routing.agent_plan.clone(),
            settings.routing.agent_execute.clone(),
            settings.routing.chat.clone(),
        ],
        _ => vec![
            settings.routing.agent_execute.clone(),
            settings.routing.chat.clone(),
        ],
    };
    for route in candidates {
        if !route.is_set() {
            continue;
        }
        let Ok(rt) = load_chat_runtime(state, &route.provider_id) else {
            continue;
        };
        let model = if route.model.trim().is_empty() {
            match rt.provider.default_chat_model.clone() {
                Some(model) => model,
                None => continue,
            }
        } else {
            route.model.clone()
        };
        return Ok((rt, model));
    }
    let rt = load_chat_runtime(state, &task.provider_id)?;
    let model = if task.model.trim().is_empty() {
        rt.provider
            .default_chat_model
            .clone()
            .ok_or_else(|| AppError::msg("子代理没有可用模型"))?
    } else {
        task.model.clone()
    };
    Ok((rt, model))
}

struct AgentSpec {
    id: String,
    role: String,
    task: String,
    context: String,
}

struct RunOutcome {
    id: String,
    role: String,
    task: String,
    provider_id: String,
    model: String,
    status: String,
    report: Option<String>,
    error: Option<String>,
    rounds: u32,
    tool_calls: u32,
}

fn insert_run(state: &AppState, task: &Task, spec: &AgentSpec) -> AppResult<()> {
    let conn = state.db();
    let timestamp = now();
    conn.execute(
        "INSERT INTO subagent_runs(id, task_id, role, task, provider_id, model, status,
         rounds, tool_calls, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, '', '', 'running', 0, 0, ?5, ?5)",
        params![spec.id, task.id, spec.role, spec.task, timestamp],
    )?;
    Ok(())
}

fn finish_run(state: &AppState, outcome: &RunOutcome) {
    let conn = state.db();
    let _ = conn.execute(
        "UPDATE subagent_runs SET provider_id = ?2, model = ?3, status = ?4, report = ?5,
         error = ?6, rounds = ?7, tool_calls = ?8, updated_at = ?9 WHERE id = ?1",
        params![
            outcome.id,
            outcome.provider_id,
            outcome.model,
            outcome.status,
            outcome.report,
            outcome.error,
            outcome.rounds as i64,
            outcome.tool_calls as i64,
            now()
        ],
    );
}

async fn run_one(
    state: &AppState,
    task: &Task,
    root: &Path,
    spec: AgentSpec,
    cancel: &CancellationToken,
) -> RunOutcome {
    let _ = insert_run(state, task, &spec);
    let (rt, model) = match resolve_route(state, task, &spec.role) {
        Ok(route) => route,
        Err(error) => {
            let outcome = RunOutcome {
                id: spec.id,
                role: spec.role,
                task: spec.task,
                provider_id: String::new(),
                model: String::new(),
                status: "failed".into(),
                report: None,
                error: Some(error.user_message()),
                rounds: 0,
                tool_calls: 0,
            };
            finish_run(state, &outcome);
            push_task_event(
                state,
                &task.id,
                "SubagentCompleted",
                json!({
                    "runId": outcome.id,
                    "role": outcome.role,
                    "task": outcome.task,
                    "status": outcome.status,
                    "error": outcome.error,
                    "rounds": 0,
                    "toolCalls": 0
                }),
            );
            return outcome;
        }
    };
    let capabilities = capabilities_for(&rt, &model);
    let tools_supported = capabilities
        .as_ref()
        .map(|caps| caps.tools != CAP_UNSUPPORTED)
        .unwrap_or(true);
    let reasoning_effort = if capabilities
        .as_ref()
        .is_some_and(|caps| caps.reasoning == CAP_UNSUPPORTED)
    {
        None
    } else {
        task.reasoning_effort.clone()
    };
    let skill_note = crate::skills::prompt_section(state);
    let system = format!(
        "你是 Basalt 的只读子代理。\n角色：{}\n工作区：{}\n主任务目标：{}\n\
         你只能读取项目文件、查看 Git 状态与 Diff、联网搜索、读取公开网页{}；\
         你不能修改文件、执行命令、请求审批或再启动子代理。\n\
         围绕给你的子任务独立调查，最后返回结构化中文报告：结论、证据（文件路径或 URL）、风险与建议。\
         不要编造未实际读取或搜索到的内容；找不到证据时明确说明。{skill_note}",
        role_prompt(&spec.role),
        root.to_string_lossy(),
        task.goal.chars().take(1200).collect::<String>(),
        if tools_supported {
            "、调用 read_skill 加载可用 Skill"
        } else {
            "（当前模型不支持工具调用，请基于已有上下文作答并标注不确定性）"
        },
    );
    let mut user = format!("子任务：{}", spec.task);
    if !spec.context.is_empty() {
        user.push_str("\n\n补充上下文：");
        user.push_str(&spec.context.chars().take(8000).collect::<String>());
    }
    let mut messages = vec![
        ChatMessage::text("system", system),
        ChatMessage::text("user", user),
    ];
    let tool_schemas = if tools_supported {
        tools::schemas_readonly_for(state)
    } else {
        Vec::new()
    };
    let recovery = {
        let conn = state.db();
        load_settings(&conn).recovery
    };
    let mut last_text = String::new();
    let mut tool_count = 0u32;
    let mut round = 0u32;

    push_task_event(
        state,
        &task.id,
        "SubagentStarted",
        json!({
            "runId": spec.id,
            "role": spec.role,
            "task": spec.task,
            "providerId": rt.provider.id,
            "model": model
        }),
    );

    loop {
        round += 1;
        if cancel.is_cancelled() {
            let outcome = RunOutcome {
                id: spec.id,
                role: spec.role,
                task: spec.task,
                provider_id: rt.provider.id.clone(),
                model: model.clone(),
                status: "cancelled".into(),
                report: Some(last_text),
                error: Some("子代理已取消".into()),
                rounds: round.saturating_sub(1),
                tool_calls: tool_count,
            };
            finish_run(state, &outcome);
            return outcome;
        }
        let request = ChatRequest {
            model: model.clone(),
            messages: messages.clone(),
            tools: tool_schemas.clone(),
            max_tokens: None,
            temperature: None,
            reasoning_effort: reasoning_effort.clone(),
            cache_prompt: true,
        };
        let mut noop = |_: &str| {};
        let mut retry = |_: u32, _: &AppError| {};
        let result = run_round(
            state,
            &rt,
            &model,
            &request,
            cancel,
            &recovery,
            &mut noop,
            &mut retry,
        )
        .await;
        let round_result = match result {
            Ok(result) => result,
            Err(failure) => {
                record_usage(
                    state,
                    &task.session_id,
                    Some(&task.id),
                    &rt.provider.id,
                    &model,
                    failure.duration_ms,
                    failure.usage,
                );
                let cancelled = failure.error.is_cancelled();
                let outcome = RunOutcome {
                    id: spec.id,
                    role: spec.role,
                    task: spec.task,
                    provider_id: rt.provider.id.clone(),
                    model: model.clone(),
                    status: if cancelled { "cancelled" } else { "failed" }.into(),
                    report: Some(last_text),
                    error: Some(failure.error.user_message()),
                    rounds: round.saturating_sub(1),
                    tool_calls: tool_count,
                };
                finish_run(state, &outcome);
                push_task_event(
                    state,
                    &task.id,
                    "SubagentCompleted",
                    json!({
                        "runId": outcome.id,
                        "role": outcome.role,
                        "task": outcome.task,
                        "status": outcome.status,
                        "error": outcome.error,
                        "rounds": outcome.rounds,
                        "toolCalls": outcome.tool_calls
                    }),
                );
                return outcome;
            }
        };
        record_usage(
            state,
            &task.session_id,
            Some(&task.id),
            &rt.provider.id,
            &model,
            round_result.duration_ms,
            round_result.usage,
        );
        if !round_result.text.trim().is_empty() {
            last_text = round_result.text.clone();
        }
        if round_result.calls.is_empty() {
            let report = if last_text.trim().is_empty() {
                "子代理没有返回文本报告。".to_string()
            } else {
                last_text
            };
            let (report, _) = crate::files::truncate_chars(&report, 16_000);
            let outcome = RunOutcome {
                id: spec.id,
                role: spec.role,
                task: spec.task,
                provider_id: rt.provider.id.clone(),
                model: model.clone(),
                status: "completed".into(),
                report: Some(report),
                error: None,
                rounds: round,
                tool_calls: tool_count,
            };
            finish_run(state, &outcome);
            push_task_event(
                state,
                &task.id,
                "SubagentCompleted",
                json!({
                    "runId": outcome.id,
                    "role": outcome.role,
                    "task": outcome.task,
                    "status": outcome.status,
                    "report": outcome.report,
                    "rounds": outcome.rounds,
                    "toolCalls": outcome.tool_calls
                }),
            );
            return outcome;
        }
        messages.push(ChatMessage {
            role: "assistant".into(),
            content: round_result.text,
            images: Vec::new(),
            tool_calls: round_result.calls.clone(),
            tool_call_id: None,
            tool_name: None,
        });
        for call in round_result.calls {
            if cancel.is_cancelled() {
                let outcome = RunOutcome {
                    id: spec.id,
                    role: spec.role,
                    task: spec.task,
                    provider_id: rt.provider.id.clone(),
                    model: model.clone(),
                    status: "cancelled".into(),
                    report: Some(last_text),
                    error: Some("子代理已取消".into()),
                    rounds: round,
                    tool_calls: tool_count,
                };
                finish_run(state, &outcome);
                return outcome;
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
                }))
                .unwrap_or_else(|_| "{\"ok\":true}".into()),
                Err(err) if err.is_cancelled() => {
                    let outcome = RunOutcome {
                        id: spec.id,
                        role: spec.role,
                        task: spec.task,
                        provider_id: rt.provider.id.clone(),
                        model: model.clone(),
                        status: "cancelled".into(),
                        report: Some(last_text),
                        error: Some("子代理已取消".into()),
                        rounds: round,
                        tool_calls: tool_count,
                    };
                    finish_run(state, &outcome);
                    return outcome;
                }
                Err(err) => serde_json::to_string(&json!({
                    "ok": false,
                    "error": err.user_message()
                }))
                .unwrap_or_else(|_| "{\"ok\":false}".into()),
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

fn outcome_value(outcome: &RunOutcome) -> Value {
    json!({
        "runId": outcome.id,
        "role": outcome.role,
        "task": outcome.task,
        "providerId": outcome.provider_id,
        "model": outcome.model,
        "status": outcome.status,
        "report": outcome.report,
        "error": outcome.error,
        "rounds": outcome.rounds,
        "toolCalls": outcome.tool_calls
    })
}

pub async fn run(
    state: &AppState,
    task: &Task,
    root: &Path,
    args: &Value,
    cancel: &CancellationToken,
) -> AppResult<ToolResult> {
    let spec = spec_from_value(args, None)?;
    let outcome = run_one(state, task, root, spec, cancel).await;
    if outcome.status == "cancelled" {
        return Err(AppError::Cancelled);
    }
    if outcome.status == "failed" {
        return Err(AppError::msg(
            outcome
                .error
                .clone()
                .unwrap_or_else(|| "子代理执行失败".into()),
        ));
    }
    Ok(ToolResult {
        data: outcome_value(&outcome),
        summary: format!(
            "子代理（{}）完成 · {} 轮 · {} 次工具调用",
            outcome.role, outcome.rounds, outcome.tool_calls
        ),
    })
}

pub async fn run_many(
    state: &AppState,
    task: &Task,
    root: &Path,
    args: &Value,
    cancel: &CancellationToken,
) -> AppResult<ToolResult> {
    let raw_agents = args
        .get("agents")
        .and_then(|value| value.as_array())
        .ok_or_else(|| AppError::msg("spawn_agents 需要 agents 数组"))?;
    if raw_agents.is_empty() {
        return Err(AppError::msg("agents 不能为空"));
    }
    if raw_agents.len() > MAX_PARALLEL {
        return Err(AppError::msg(format!(
            "一次最多并行 {MAX_PARALLEL} 个子代理"
        )));
    }
    let mut specs = Vec::new();
    for raw in raw_agents {
        specs.push(spec_from_value(raw, None)?);
    }
    let outcomes = stream::iter(
        specs
            .into_iter()
            .map(|spec| run_one(state, task, root, spec, cancel)),
    )
    .buffer_unordered(CONCURRENCY)
    .collect::<Vec<_>>()
    .await;
    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }
    let failed = outcomes
        .iter()
        .filter(|outcome| outcome.status == "failed")
        .count();
    let total_tools: u32 = outcomes.iter().map(|outcome| outcome.tool_calls).sum();
    let agents: Vec<Value> = outcomes.iter().map(outcome_value).collect();
    Ok(ToolResult {
        data: json!({
            "agents": agents,
            "count": outcomes.len(),
            "failed": failed,
            "toolCalls": total_tools
        }),
        summary: format!(
            "{} 个子代理完成{} · {} 次工具调用",
            outcomes.len(),
            if failed > 0 {
                format!("，{failed} 个失败")
            } else {
                String::new()
            },
            total_tools
        ),
    })
}

fn spec_from_value(args: &Value, id: Option<String>) -> AppResult<AgentSpec> {
    let sub_task = args
        .get("task")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if sub_task.is_empty() {
        return Err(AppError::msg("子代理需要非空 task"));
    }
    if sub_task.chars().count() > 4000 {
        return Err(AppError::msg("子代理任务过长"));
    }
    let role = normalize_role(
        args.get("role")
            .and_then(|value| value.as_str())
            .unwrap_or("researcher"),
    );
    let context = args
        .get("context")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(AgentSpec {
        id: id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        role,
        task: sub_task,
        context,
    })
}

pub fn list_runs(state: &AppState, task_id: &str) -> AppResult<Vec<crate::models::SubagentRun>> {
    let conn = state.db();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, role, task, provider_id, model, status, report, error,
                rounds, tool_calls, created_at, updated_at
         FROM subagent_runs WHERE task_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([task_id], |row| {
        Ok(crate::models::SubagentRun {
            id: row.get(0)?,
            task_id: row.get(1)?,
            role: row.get(2)?,
            task: row.get(3)?,
            provider_id: row.get(4)?,
            model: row.get(5)?,
            status: row.get(6)?,
            report: row.get(7)?,
            error: row.get(8)?,
            rounds: row.get::<_, i64>(9)?.max(0) as u64,
            tool_calls: row.get::<_, i64>(10)?.max(0) as u64,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}
