use std::net::IpAddr;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use serde_json::{json, Value};
use tokio::net::lookup_host;

use crate::error::{AppError, AppResult};
use crate::models::{DiscoveredModel, ImageCapabilities};

use super::http::{ensure_ok, merge_params, request, with_bearer};
use super::{
    ImageAdapter, ImageEditRequest, ImageOutput, ImageRequest, ProviderRuntime,
};

pub struct OpenAiImageAdapter;

fn image_capabilities_for(model: &str) -> ImageCapabilities {
    let lower = model.to_lowercase();
    if lower.contains("dall-e-3") {
        ImageCapabilities {
            generate: true,
            edit: false,
            reference_images: false,
            mask: false,
            multiple: false,
            max_count: 1,
            sizes: vec!["1024x1024".into(), "1792x1024".into(), "1024x1792".into()],
            qualities: vec!["standard".into(), "hd".into()],
            transparent_background: false,
            async_tasks: false,
            notes: vec!["能力基于 DALL·E 3 官方文档；实际以接口返回为准".into()],
        }
    } else if lower.contains("dall-e-2") {
        ImageCapabilities {
            generate: true,
            edit: true,
            reference_images: true,
            mask: true,
            multiple: true,
            max_count: 10,
            sizes: vec!["256x256".into(), "512x512".into(), "1024x1024".into()],
            qualities: vec![],
            transparent_background: false,
            async_tasks: false,
            notes: vec![],
        }
    } else if lower.contains("gpt-image") {
        ImageCapabilities {
            generate: true,
            edit: true,
            reference_images: true,
            mask: true,
            multiple: true,
            max_count: 10,
            sizes: vec![
                "1024x1024".into(),
                "1536x1024".into(),
                "1024x1536".into(),
                "auto".into(),
            ],
            qualities: vec!["auto".into(), "low".into(), "medium".into(), "high".into()],
            transparent_background: true,
            async_tasks: false,
            notes: vec!["能力基于 GPT Image 官方文档；实际以接口返回为准".into()],
        }
    } else {
        ImageCapabilities {
            generate: true,
            edit: true,
            reference_images: true,
            mask: false,
            multiple: true,
            max_count: 4,
            sizes: vec!["1024x1024".into(), "1536x1024".into(), "1024x1536".into()],
            qualities: vec![],
            transparent_background: false,
            async_tasks: false,
            notes: vec!["自定义图像接口能力未经验证，生成失败时请核对供应商文档".into()],
        }
    }
}

fn detect_mime(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png".into()
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg".into()
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp".into()
    } else if bytes.starts_with(b"GIF8") {
        "image/gif".into()
    } else if bytes
        .iter()
        .take(512)
        .copied()
        .collect::<Vec<u8>>()
        .windows(4)
        .any(|w| w == b"<svg")
    {
        "image/svg+xml".into()
    } else {
        "application/octet-stream".into()
    }
}

fn png_dimensions(bytes: &[u8]) -> (Option<u32>, Option<u32>) {
    if bytes.len() >= 24 && bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        (Some(width), Some(height))
    } else {
        (None, None)
    }
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

async fn download_remote_image(url: &str) -> AppResult<Vec<u8>> {
    let parsed = url::Url::parse(url).map_err(|e| AppError::msg(format!("图片 URL 无效：{e}")))?;
    if parsed.scheme() != "https" {
        return Err(AppError::msg("出于安全考虑，只允许下载 HTTPS 图片地址"));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::msg("图片 URL 缺少主机名"))?
        .to_string();
    if host.eq_ignore_ascii_case("localhost") {
        return Err(AppError::msg("拒绝下载本地地址图片"));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            return Err(AppError::msg("拒绝下载内网地址图片"));
        }
    } else {
        let port = parsed.port_or_known_default().unwrap_or(443);
        let addrs = lookup_host((host.as_str(), port))
            .await
            .map_err(|e| AppError::msg(format!("无法解析图片地址：{e}")))?;
        for addr in addrs {
            if is_private_ip(&addr.ip()) {
                return Err(AppError::msg("拒绝下载解析到内网地址的图片"));
            }
        }
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(90))
        .build()?;
    let response = client.get(url).send().await?;
    let response = ensure_ok(response).await?;
    if let Some(length) = response.content_length() {
        if length > 30 * 1024 * 1024 {
            return Err(AppError::msg("远程图片超过 30MB 限制"));
        }
    }
    let bytes = response.bytes().await?;
    if bytes.len() > 30 * 1024 * 1024 {
        return Err(AppError::msg("远程图片超过 30MB 限制"));
    }
    Ok(bytes.to_vec())
}

async fn parse_image_response(value: Value) -> AppResult<Vec<ImageOutput>> {
    if let Some(error) = value.get("error") {
        return Err(AppError::msg(format!("图像服务返回错误：{error}")));
    }
    let data = value
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::msg("图像响应缺少 data 字段"))?;
    let mut outputs = Vec::new();
    for entry in data {
        if let Some(b64) = entry.get("b64_json").and_then(|v| v.as_str()) {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| AppError::msg(format!("图片解码失败：{e}")))?;
            let mime = detect_mime(&bytes);
            let (width, height) = png_dimensions(&bytes);
            outputs.push(ImageOutput {
                bytes,
                mime,
                width,
                height,
                mock: false,
            });
        } else if let Some(url) = entry.get("url").and_then(|v| v.as_str()) {
            let bytes = download_remote_image(url).await?;
            let mime = detect_mime(&bytes);
            let (width, height) = png_dimensions(&bytes);
            outputs.push(ImageOutput {
                bytes,
                mime,
                width,
                height,
                mock: false,
            });
        } else {
            return Err(AppError::msg("图像响应中包含无法识别的图片项"));
        }
    }
    if outputs.is_empty() {
        return Err(AppError::msg("图像服务未返回任何图片"));
    }
    Ok(outputs)
}

#[async_trait]
impl ImageAdapter for OpenAiImageAdapter {
    fn capabilities(&self, _rt: &ProviderRuntime, model: &str) -> ImageCapabilities {
        image_capabilities_for(model)
    }

    async fn list_models(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
    ) -> AppResult<Vec<DiscoveredModel>> {
        let url = rt.endpoint("models")?;
        let builder = with_bearer(request(client, reqwest::Method::GET, &url, rt), rt);
        let response = ensure_ok(builder.send().await?).await?;
        let value: Value = response.json().await?;
        let mut models = Vec::new();
        if let Some(items) = value
            .get("data")
            .and_then(|v| v.as_array())
            .or_else(|| value.as_array())
        {
            for item in items {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    models.push(DiscoveredModel {
                        id: id.to_string(),
                        name: item
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(id)
                            .to_string(),
                        context_length: None,
                    });
                }
            }
        }
        Ok(models)
    }

    async fn generate(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
        req: &ImageRequest,
    ) -> AppResult<Vec<ImageOutput>> {
        let url = rt.endpoint("images/generations")?;
        let count = req.count.max(1);
        let mut body = json!({
            "model": req.model,
            "prompt": req.prompt,
            "n": count
        });
        if let Some(size) = &req.size {
            body["size"] = json!(size);
        }
        if let Some(quality) = &req.quality {
            body["quality"] = json!(quality);
        }
        if let Some(background) = &req.background {
            body["background"] = json!(background);
        }
        if req.model.to_lowercase().contains("dall-e") {
            body["response_format"] = json!("b64_json");
        }
        let body = merge_params(body, rt);
        let builder = with_bearer(request(client, reqwest::Method::POST, &url, rt), rt).json(&body);
        let response = ensure_ok(builder.send().await?).await?;
        let value: Value = response.json().await?;
        parse_image_response(value).await
    }

    async fn edit(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
        req: &ImageEditRequest,
    ) -> AppResult<Vec<ImageOutput>> {
        let url = rt.endpoint("images/edits")?;
        let bytes = std::fs::read(&req.image)?;
        if bytes.len() > 25 * 1024 * 1024 {
            return Err(AppError::msg("参考图片超过 25MB 限制"));
        }
        let mime = detect_mime(&bytes);
        if !matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/webp") {
            return Err(AppError::msg("参考图片必须是 PNG、JPEG 或 WebP"));
        }
        let name = req
            .image
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "image.png".into());
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(name)
            .mime_str(&mime)
            .map_err(|e| AppError::msg(format!("构造图片表单失败：{e}")))?;
        let mut form = reqwest::multipart::Form::new()
            .text("model", req.model.clone())
            .text("prompt", req.prompt.clone())
            .text("n", req.count.max(1).to_string())
            .part("image", part);
        if let Some(size) = &req.size {
            form = form.text("size", size.clone());
        }
        if let Some(quality) = &req.quality {
            form = form.text("quality", quality.clone());
        }
        if let Some(mask) = &req.mask {
            let mask_bytes = std::fs::read(mask)?;
            if mask_bytes.len() > 25 * 1024 * 1024 {
                return Err(AppError::msg("蒙版图片超过 25MB 限制"));
            }
            let mask_part = reqwest::multipart::Part::bytes(mask_bytes)
                .file_name("mask.png")
                .mime_str("image/png")
                .map_err(|e| AppError::msg(format!("构造蒙版表单失败：{e}")))?;
            form = form.part("mask", mask_part);
        }
        let builder = with_bearer(request(client, reqwest::Method::POST, &url, rt), rt).multipart(form);
        let response = ensure_ok(builder.send().await?).await?;
        let value: Value = response.json().await?;
        parse_image_response(value).await
    }
}

pub struct GeminiImageAdapter;

fn gemini_capabilities(model: &str) -> ImageCapabilities {
    let is_image_model = model.to_lowercase().contains("image");
    ImageCapabilities {
        generate: is_image_model,
        edit: is_image_model,
        reference_images: is_image_model,
        mask: false,
        multiple: false,
        max_count: 1,
        sizes: vec![
            "1024x1024".into(),
            "1536x1024".into(),
            "1024x1536".into(),
            "1280x720".into(),
            "720x1280".into(),
        ],
        qualities: vec![],
        transparent_background: false,
        async_tasks: false,
        notes: vec![
            "Gemini 图像接口为同步请求；画幅按 aspectRatio 映射".into(),
            "不支持蒙版编辑与单次多图输出".into(),
        ],
    }
}

fn aspect_ratio(size: &str) -> Option<&'static str> {
    match size {
        "1024x1024" => Some("1:1"),
        "1536x1024" => Some("3:2"),
        "1024x1536" => Some("2:3"),
        "1280x720" => Some("16:9"),
        "720x1280" => Some("9:16"),
        _ => None,
    }
}

fn gemini_key_header(builder: reqwest::RequestBuilder, rt: &ProviderRuntime) -> AppResult<reqwest::RequestBuilder> {
    if !rt.has_key() {
        return Err(AppError::msg("Gemini 需要 API Key"));
    }
    Ok(builder.header("x-goog-api-key", rt.api_key.as_deref().unwrap_or_default()))
}

fn model_id(raw: &str) -> String {
    raw.trim().trim_start_matches("models/").to_string()
}

#[async_trait]
impl ImageAdapter for GeminiImageAdapter {
    fn capabilities(&self, _rt: &ProviderRuntime, model: &str) -> ImageCapabilities {
        gemini_capabilities(model)
    }

    async fn list_models(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
    ) -> AppResult<Vec<DiscoveredModel>> {
        let url = rt.endpoint("models")?;
        let builder = gemini_key_header(request(client, reqwest::Method::GET, &url, rt), rt)?;
        let response = ensure_ok(builder.send().await?).await?;
        let value: Value = response.json().await?;
        let mut models = Vec::new();
        if let Some(items) = value.get("models").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(full_name) = item.get("name").and_then(|v| v.as_str()) {
                    let id = model_id(full_name);
                    models.push(DiscoveredModel {
                        id: id.clone(),
                        name: item
                            .get("displayName")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&id)
                            .to_string(),
                        context_length: item.get("inputTokenLimit").and_then(|v| v.as_u64()),
                    });
                }
            }
        }
        Ok(models)
    }

    async fn generate(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
        req: &ImageRequest,
    ) -> AppResult<Vec<ImageOutput>> {
        self.request_images(client, rt, req, None).await
    }

    async fn edit(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
        req: &ImageEditRequest,
    ) -> AppResult<Vec<ImageOutput>> {
        if req.mask.is_some() {
            return Err(AppError::msg("Gemini 图像接口不支持蒙版编辑"));
        }
        let bytes = std::fs::read(&req.image)?;
        if bytes.len() > 25 * 1024 * 1024 {
            return Err(AppError::msg("参考图片超过 25MB 限制"));
        }
        let mime = detect_mime(&bytes);
        if !mime.starts_with("image/") {
            return Err(AppError::msg("参考文件不是图片"));
        }
        let request = ImageRequest {
            model: req.model.clone(),
            prompt: req.prompt.clone(),
            size: req.size.clone(),
            quality: req.quality.clone(),
            count: req.count,
            background: None,
        };
        self.request_images(client, rt, &request, Some((mime, bytes)))
            .await
    }
}

impl GeminiImageAdapter {
    async fn request_images(
        &self,
        client: &reqwest::Client,
        rt: &ProviderRuntime,
        req: &ImageRequest,
        reference: Option<(String, Vec<u8>)>,
    ) -> AppResult<Vec<ImageOutput>> {
        let model = model_id(&req.model);
        if model.is_empty()
            || !model
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(AppError::msg("图像模型 ID 包含无效字符"));
        }
        let url = rt.endpoint(&format!("models/{model}:generateContent"))?;
        let mut parts = vec![json!({"text": req.prompt})];
        if let Some((mime, bytes)) = reference {
            parts.push(json!({
                "inlineData": {
                    "mimeType": mime,
                    "data": base64::engine::general_purpose::STANDARD.encode(bytes)
                }
            }));
        }
        let mut generation_config = json!({
            "responseModalities": ["TEXT", "IMAGE"]
        });
        if let Some(size) = req.size.as_deref().and_then(aspect_ratio) {
            generation_config["imageConfig"] = json!({"aspectRatio": size});
        }
        let mut body = json!({
            "contents": [{"role": "user", "parts": parts}],
            "generationConfig": generation_config
        });
        body = merge_params(body, rt);
        let builder = gemini_key_header(request(client, reqwest::Method::POST, &url, rt), rt)?;
        let response = ensure_ok(builder.json(&body).send().await?).await?;
        let value: Value = response.json().await?;
        if let Some(error) = value.get("error") {
            return Err(AppError::msg(format!("Gemini 图像错误：{error}")));
        }
        let mut outputs = Vec::new();
        let mut text_note: Option<String> = None;
        if let Some(parts) = value
            .get("candidates")
            .and_then(|v| v.as_array())
            .and_then(|v| v.first())
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|v| v.as_array())
        {
            for part in parts {
                if let Some(inline) = part
                    .get("inlineData")
                    .or_else(|| part.get("inline_data"))
                {
                    if let Some(data) = inline.get("data").and_then(|v| v.as_str()) {
                        let bytes = base64::engine::general_purpose::STANDARD
                            .decode(data)
                            .map_err(|e| AppError::msg(format!("图片解码失败：{e}")))?;
                        let mime = inline
                            .get("mimeType")
                            .or_else(|| inline.get("mime_type"))
                            .and_then(|v| v.as_str())
                            .map(str::to_string)
                            .unwrap_or_else(|| detect_mime(&bytes));
                        let (width, height) = png_dimensions(&bytes);
                        outputs.push(ImageOutput {
                            bytes,
                            mime,
                            width,
                            height,
                            mock: false,
                        });
                    }
                }
                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                    if !text.trim().is_empty() {
                        text_note = Some(text.to_string());
                    }
                }
            }
        }
        if outputs.is_empty() {
            return Err(AppError::msg(format!(
                "Gemini 未返回图片{}",
                text_note
                    .map(|t| format!("：{}", t.chars().take(400).collect::<String>()))
                    .unwrap_or_default()
            )));
        }
        Ok(outputs)
    }
}

