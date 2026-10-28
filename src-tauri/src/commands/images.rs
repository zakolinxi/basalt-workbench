use std::path::{Path, PathBuf};

use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use tauri::State;
use uuid::Uuid;

use crate::db::{load_settings, now, row_to_image_generation, row_to_session};
use crate::error::{AppError, AppResult};
use crate::events::{emit, EVENT_IMAGE};
use crate::files::path_is_within;
use crate::messaging;
use crate::models::{
    Attachment, ImageCapabilities, ImageEvent, ImageFile, ImageGeneration, CAP_VERIFIED,
};
use crate::providers::{
    image_adapter, load_image_runtime, mark_capabilities, ImageEditRequest, ImageOutput,
    ImageRequest, ProviderRuntime,
};
use crate::state::AppState;


fn extension_for(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

fn save_output(state: &AppState, output: ImageOutput, mode: &str) -> AppResult<ImageFile> {
    let dir = state.data_dir.join("images").join(chrono::Local::now().format("%Y-%m").to_string());
    std::fs::create_dir_all(&dir)?;
    let ext = extension_for(&output.mime);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!("{mode}-{stamp}-{}.{}", &Uuid::new_v4().to_string()[..6], ext);
    let path = dir.join(&name);
    std::fs::write(&path, &output.bytes)?;
    Ok(ImageFile {
        path: path.to_string_lossy().to_string(),
        mime: output.mime,
        name,
        mock: output.mock,
        width: output.width,
        height: output.height,
    })
}

fn emit_image(state: &AppState, generation: &ImageGeneration, status: &str) {
    emit(
        &state.app,
        EVENT_IMAGE,
        ImageEvent {
            generation_id: generation.id.clone(),
            status: status.to_string(),
            generation: Some(generation.clone()),
            error: generation.error.clone(),
        },
    );
}

fn load_generation(state: &AppState, id: &str) -> AppResult<ImageGeneration> {
    let conn = state.db();
    conn.query_row(
        "SELECT * FROM image_generations WHERE id = ?1",
        [id],
        row_to_image_generation,
    )
    .map_err(|_| AppError::msg("图像生成记录不存在"))
}

fn ensure_reference_path(state: &AppState, raw: &str) -> AppResult<PathBuf> {
    let path = Path::new(raw)
        .canonicalize()
        .map_err(|_| AppError::msg("参考图片不存在"))?;
    let mut allowed = false;
    for sub in ["attachments", "images"] {
        if let Ok(root) = state.data_dir.join(sub).canonicalize() {
            if path_is_within(&root, &path) {
                allowed = true;
                break;
            }
        }
    }
    if !allowed {
        return Err(AppError::msg("参考图片必须来自工作台附件或图像目录"));
    }
    let meta = std::fs::metadata(&path)?;
    if meta.len() > 25 * 1024 * 1024 {
        return Err(AppError::msg("参考图片超过 25MB 限制"));
    }
    Ok(path)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenerateInput {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub reference_paths: Vec<String>,
    #[serde(default)]
    pub mask_path: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[allow(clippy::too_many_arguments)]
async fn generate_internal(
    state: &AppState,
    provider_id: Option<&str>,
    model: Option<&str>,
    prompt: &str,
    mode: &str,
    size: Option<String>,
    quality: Option<String>,
    count: u32,
    background: Option<String>,
    reference_paths: Vec<String>,
    mask_path: Option<String>,
    session_id: Option<&str>,
) -> AppResult<ImageGeneration> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::msg("提示词不能为空"));
    }
    if prompt.chars().count() > 8000 {
        return Err(AppError::msg("提示词过长"));
    }
    let settings = {
        let conn = state.db();
        load_settings(&conn)
    };
    let route = if mode == "edit" && settings.routing.image_edit.is_set() {
        settings.routing.image_edit.clone()
    } else {
        settings.routing.image.clone()
    };
    let provider_id = provider_id
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .or_else(|| Some(route.provider_id.clone()).filter(|id| !id.is_empty()))
        .ok_or_else(|| AppError::msg("尚未配置图像供应商"))?;
    let rt = load_image_runtime(state, &provider_id)?;
    let model = model
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .or_else(|| {
            if route.provider_id == provider_id && !route.model.is_empty() {
                Some(route.model.clone())
            } else {
                rt.provider.default_image_model.clone()
            }
        })
        .ok_or_else(|| AppError::msg("未选择图像模型"))?;
    let adapter = image_adapter(&rt.provider.protocol)?;
    let caps = adapter.capabilities(&rt, &model);
    if mode == "edit" && !caps.edit {
        return Err(AppError::msg("当前图像模型不支持图像编辑"));
    }
    if mode != "edit" && !caps.generate {
        return Err(AppError::msg("当前图像模型不支持文本生图"));
    }
    let count = count.max(1);
    if count > caps.max_count {
        return Err(AppError::msg(format!(
            "当前模型单次最多生成 {} 张图片",
            caps.max_count
        )));
    }
    if count > 1 && !caps.multiple {
        return Err(AppError::msg("当前模型不支持单次多图输出"));
    }
    if let Some(size) = &size {
        if !size.trim().is_empty() && !caps.sizes.is_empty() && !caps.sizes.contains(size) {
            return Err(AppError::msg(format!(
                "当前模型不支持尺寸 {size}。可用尺寸：{}",
                caps.sizes.join("、")
            )));
        }
    }
    if let Some(quality) = &quality {
        if !quality.trim().is_empty() && !caps.qualities.is_empty() && !caps.qualities.contains(quality) {
            return Err(AppError::msg(format!(
                "当前模型不支持质量选项 {quality}。可用选项：{}",
                caps.qualities.join("、")
            )));
        }
    }
    if background.is_some() && !caps.transparent_background {
        return Err(AppError::msg("当前模型不支持背景参数"));
    }
    if mask_path.is_some() && !caps.mask {
        return Err(AppError::msg("当前模型不支持蒙版编辑"));
    }
    if !reference_paths.is_empty() && !caps.reference_images {
        return Err(AppError::msg("当前模型不支持参考图片"));
    }
    if reference_paths.len() > 1 {
        return Err(AppError::msg("当前工作台一次只支持一张参考图片"));
    }
    if mode == "edit" && reference_paths.is_empty() {
        return Err(AppError::msg("图像编辑需要一张参考图片"));
    }
    if mode != "edit" && !reference_paths.is_empty() {
        return Err(AppError::msg("参考图片仅在图像编辑模式下使用"));
    }
    if let Some(session_id) = session_id {
        let conn = state.db();
        conn.query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [session_id],
            row_to_session,
        )
        .map_err(|_| AppError::msg("会话不存在"))?;
    }

    let reference = if let Some(raw) = reference_paths.first() {
        Some(ensure_reference_path(state, raw)?)
    } else {
        None
    };
    let mask = mask_path
        .as_deref()
        .filter(|p| !p.is_empty())
        .map(|raw| ensure_reference_path(state, raw))
        .transpose()?;

    let id = Uuid::new_v4().to_string();
    let created_at = now();
    let provider_name = rt.provider.name.clone();
    let params_json = json!({
        "size": size.clone(),
        "quality": quality.clone(),
        "count": count,
        "background": background.clone(),
        "hasReference": reference.is_some(),
        "hasMask": mask.is_some()
    });
    {
        let conn = state.db();
        conn.execute(
            "INSERT INTO image_generations(id, provider_id, provider_name, model, mode, prompt,
             params_json, files_json, status, session_id, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, '[]', 'running', ?8, ?9)",
            params![
                id,
                rt.provider.id,
                provider_name,
                model,
                mode,
                prompt,
                params_json.to_string(),
                session_id,
                created_at
            ],
        )?;
    }
    let running = load_generation(state, &id)?;
    emit_image(state, &running, "started");

    let result: AppResult<Vec<ImageOutput>> = if mode == "edit" {
        let reference = reference.ok_or_else(|| AppError::msg("图像编辑需要一张参考图片"))?;
        adapter
            .edit(
                &state.http,
                &rt,
                &ImageEditRequest {
                    model: model.clone(),
                    prompt: prompt.clone(),
                    size: size.clone(),
                    quality: quality.clone(),
                    count,
                    image: reference,
                    mask,
                },
            )
            .await
    } else {
        adapter
            .generate(
                &state.http,
                &rt,
                &ImageRequest {
                    model: model.clone(),
                    prompt: prompt.clone(),
                    size: size.clone(),
                    quality: quality.clone(),
                    count,
                    background: background.clone(),
                },
            )
            .await
    };

    match result {
        Ok(outputs) => {
            let mut files = Vec::new();
            for output in outputs {
                files.push(save_output(state, output, mode)?);
            }
            {
                let conn = state.db();
                conn.execute(
                    "UPDATE image_generations SET status = 'completed', files_json = ?2 WHERE id = ?1",
                    params![id, serde_json::to_string(&files)?],
                )?;
            }
            let capability = if mode == "edit" {
                "imageEdit"
            } else {
                "imageGen"
            };
            let _ = mark_capabilities(
                state,
                &rt.provider.id,
                &model,
                &[(capability, CAP_VERIFIED)],
            );
            state.log(
                "INFO",
                &format!("图像生成完成：{} / {} / {}", rt.provider.name, model, id),
            );
            let generation = load_generation(state, &id)?;
            emit_image(state, &generation, "completed");
            Ok(generation)
        }
        Err(err) => {
            let message = err.user_message();
            {
                let conn = state.db();
                conn.execute(
                    "UPDATE image_generations SET status = 'failed', error = ?2 WHERE id = ?1",
                    params![id, message],
                )?;
            }
            state.log(
                "WARN",
                &format!("图像生成失败：{} / {}：{}", rt.provider.name, model, message),
            );
            let generation = load_generation(state, &id)?;
            emit_image(state, &generation, "failed");
            Err(err)
        }
    }
}

#[tauri::command]
pub async fn image_generate(
    state: State<'_, AppState>,
    input: ImageGenerateInput,
) -> AppResult<ImageGeneration> {
    let mode = match input.mode.as_deref() {
        Some("edit") => "edit",
        Some("generate") | None => "generate",
        Some(other) => return Err(AppError::msg(format!("无效的图像模式：{other}"))),
    };
    generate_internal(
        &state,
        Some(&input.provider_id),
        Some(&input.model),
        &input.prompt,
        mode,
        input.size,
        input.quality,
        input.count.unwrap_or(1),
        input.background,
        input.reference_paths,
        input.mask_path,
        input.session_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub fn image_capabilities(
    state: State<'_, AppState>,
    provider_id: String,
    model: String,
) -> AppResult<ImageCapabilities> {
    let rt: ProviderRuntime = load_image_runtime(&state, &provider_id)?;
    let adapter = image_adapter(&rt.provider.protocol)?;
    Ok(adapter.capabilities(&rt, &model))
}

#[tauri::command]
pub fn image_history(
    state: State<'_, AppState>,
    session_id: Option<String>,
    limit: Option<u32>,
) -> AppResult<Vec<ImageGeneration>> {
    let limit = limit.unwrap_or(100).clamp(1, 300) as i64;
    let conn = state.db();
    let out = if let Some(session_id) = session_id.filter(|s| !s.is_empty()) {
        let mut stmt = conn.prepare(
            "SELECT * FROM image_generations WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], row_to_image_generation)?;
        rows.collect::<Result<Vec<_>, _>>()?
    } else {
        let mut stmt = conn.prepare(
            "SELECT * FROM image_generations ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], row_to_image_generation)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(out)
}

#[tauri::command]
pub fn image_delete(state: State<'_, AppState>, generation_id: String) -> AppResult<()> {
    let generation = load_generation(&state, &generation_id)?;
    let images_root = state.data_dir.join("images").canonicalize()?;
    for file in &generation.files {
        if let Ok(canon) = Path::new(&file.path).canonicalize() {
            if path_is_within(&images_root, &canon) {
                let _ = std::fs::remove_file(canon);
            }
        }
    }
    let conn = state.db();
    conn.execute(
        "DELETE FROM image_generations WHERE id = ?1",
        [&generation_id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn image_save_as(
    state: State<'_, AppState>,
    source_path: String,
    target_path: String,
) -> AppResult<String> {
    let source = Path::new(&source_path)
        .canonicalize()
        .map_err(|_| AppError::msg("源图片不存在"))?;
    let images_root = state.data_dir.join("images").canonicalize()?;
    if !path_is_within(&images_root, &source) {
        return Err(AppError::msg("只能保存工作台生成的图片"));
    }
    let target = PathBuf::from(target_path.trim());
    if target.as_os_str().is_empty() {
        return Err(AppError::msg("保存路径为空"));
    }
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            return Err(AppError::msg("目标目录不存在"));
        }
    }
    std::fs::copy(&source, &target)?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn chat_image_generate(
    state: State<'_, AppState>,
    session_id: String,
    prompt: String,
) -> AppResult<serde_json::Value> {
    let session = {
        let conn = state.db();
        conn.query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [&session_id],
            row_to_session,
        )
        .map_err(|_| AppError::msg("会话不存在"))?
    };
    let generation = generate_internal(
        &state,
        None,
        None,
        &prompt,
        "generate",
        None,
        None,
        1,
        None,
        Vec::new(),
        None,
        Some(&session_id),
    )
    .await?;
    let attachments: Vec<Attachment> = generation
        .files
        .iter()
        .map(|file| {
            let size = std::fs::metadata(&file.path).map(|m| m.len()).unwrap_or(0);
            Attachment {
                name: file.name.clone(),
                mime: file.mime.clone(),
                size,
                kind: "image".into(),
                path: file.path.clone(),
            }
        })
        .collect();
    let user_text = format!("/img {prompt}");
    let user_message = messaging::add_message(
        &state,
        None,
        &session_id,
        "user",
        &user_text,
        "text",
        &[],
        None,
        None,
    )?;
    let assistant_text = format!(
        "已通过 {} / {} 生成图像。",
        generation.provider_name, generation.model
    );
    let assistant_message = messaging::add_message(
        &state,
        None,
        &session_id,
        "assistant",
        &assistant_text,
        "image",
        &attachments,
        None,
        Some(&generation.model),
    )?;
    if session.title == "新会话" || session.title.trim().is_empty() {
        let title: String = prompt.chars().take(48).collect();
        let conn = state.db();
        let _ = conn.execute(
            "UPDATE sessions SET title = ?2, updated_at = ?3 WHERE id = ?1",
            params![session_id, title, now()],
        );
    }
    Ok(json!({
        "generation": generation,
        "userMessage": user_message,
        "assistantMessage": assistant_message
    }))
}
