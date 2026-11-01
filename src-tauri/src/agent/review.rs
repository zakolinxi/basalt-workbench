use tokio_util::sync::CancellationToken;

use crate::db::load_settings;
use crate::error::{AppError, AppResult};
use crate::models::{Route, Task, CAP_VERIFIED};
use crate::providers::{load_chat_runtime, ChatMessage, ChatRequest, ProviderRuntime};
use crate::state::AppState;

use super::round::{record_usage, run_round};

fn review_routes(settings: &crate::models::Settings) -> Vec<Route> {
    [
        settings.routing.review.clone(),
        settings.routing.agent_execute.clone(),
        settings.routing.chat.clone(),
    ]
    .into_iter()
    .filter(|route| route.is_set())
    .collect()
}

pub async fn generate(state: &AppState, task: &Task) -> AppResult<(String, String)> {
    let settings = {
        let conn = state.db();
        load_settings(&conn)
    };
    let mut resolved: Option<(ProviderRuntime, String)> = None;
    for route in review_routes(&settings) {
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
        resolved = Some((rt, model));
        break;
    }
    let (rt, model) = match resolved {
        Some(value) => value,
        None => {
            let rt = load_chat_runtime(state, &task.provider_id).map_err(|_| {
                AppError::msg(
                    "没有可用的审查模型。请在设置中配置代码审查路由或聊天路由。",
                )
            })?;
            let model = if task.model.trim().is_empty() {
                rt.provider
                    .default_chat_model
                    .clone()
                    .ok_or_else(|| AppError::msg("没有可用的审查模型"))?
            } else {
                task.model.clone()
            };
            (rt, model)
        }
    };
    let changes = crate::changes::list_changes(state, Some(&task.id), None)?;
    if changes.is_empty() {
        return Err(AppError::msg("该任务还没有文件变更可供审查"));
    }
    let mut material = String::new();
    for change in changes.iter().take(20) {
        let diff = crate::changes::change_diff(state, &change.id)?;
        if diff.binary {
            continue;
        }
        let (old_text, _) = crate::files::truncate_chars(&diff.old_text, 12_000);
        let (new_text, _) = crate::files::truncate_chars(&diff.new_text, 12_000);
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
                "你是 Basalt 的独立代码审查模型。请只基于给出的真实文件变更进行审查，\
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
        cache_prompt: false,
    };
    let cancel = CancellationToken::new();
    let mut noop = |_: &str| {};
    let mut retry = |_: u32, _: &AppError| {};
    let result = run_round(
        state,
        &rt,
        &model,
        &request,
        &cancel,
        &settings.recovery,
        &mut noop,
        &mut retry,
    )
    .await
    .map_err(|failure| failure.error)?;
    record_usage(
        state,
        &task.session_id,
        Some(&task.id),
        &rt.provider.id,
        &model,
        result.duration_ms,
        result.usage,
    );
    if result.text.trim().is_empty() {
        return Err(AppError::msg("审查模型没有返回文本内容"));
    }
    let _ = crate::providers::mark_capabilities(
        state,
        &rt.provider.id,
        &model,
        &[("chat", CAP_VERIFIED), ("stream", CAP_VERIFIED)],
    );
    Ok((model, result.text))
}
