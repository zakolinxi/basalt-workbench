use std::time::Duration;

use reqwest::header::{HeaderName, HeaderValue};
use reqwest::{Client, RequestBuilder, Response};

use crate::error::{http_status_error, AppError, AppResult};

use super::ProviderRuntime;

const FORBIDDEN_HEADERS: [&str; 6] = [
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "authorization",
    "x-api-key",
];

pub fn join_url(base: &str, suffix: &str) -> AppResult<String> {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(AppError::msg("未配置 Base URL"));
    }
    let joined = if suffix.is_empty() {
        base.to_string()
    } else {
        format!("{}/{}", base, suffix.trim_start_matches('/'))
    };
    let parsed = url::Url::parse(&joined)
        .map_err(|e| AppError::msg(format!("Base URL 无效：{e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::msg("Base URL 必须使用 http 或 https"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::msg("Base URL 不能包含用户名或密码"));
    }
    Ok(joined)
}

pub fn request(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    rt: &ProviderRuntime,
) -> RequestBuilder {
    let mut builder = client
        .request(method, url)
        .timeout(Duration::from_secs(rt.provider.timeout_secs.clamp(5, 600)));
    let mut has_custom_ua = false;
    if let Some(headers) = rt.provider.headers.as_object() {
        for (key, value) in headers {
            let lower = key.to_ascii_lowercase();
            if FORBIDDEN_HEADERS.contains(&lower.as_str()) {
                continue;
            }
            let Some(value) = value.as_str() else { continue };
            if value.contains('\r') || value.contains('\n') {
                continue;
            }
            if lower == "user-agent" {
                has_custom_ua = true;
            }
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                builder = builder.header(name, value);
            }
        }
    }
    if !has_custom_ua {
        if let Ok(value) = HeaderValue::from_str(rt.user_agent.trim()) {
            if !rt.user_agent.trim().is_empty() {
                builder = builder.header(reqwest::header::USER_AGENT, value);
            }
        }
    }
    builder
}

pub fn with_bearer(mut builder: RequestBuilder, rt: &ProviderRuntime) -> RequestBuilder {
    if let Some(key) = rt.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
        builder = builder.bearer_auth(key);
    }
    builder
}

pub async fn ensure_ok(response: Response) -> AppResult<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    Err(http_status_error(status, &body))
}

pub fn merge_params(mut body: serde_json::Value, rt: &ProviderRuntime) -> serde_json::Value {
    if let (Some(target), Some(params)) = (body.as_object_mut(), rt.provider.params.as_object()) {
        for (key, value) in params {
            if matches!(
                key.as_str(),
                "model" | "messages" | "input" | "contents" | "stream" | "tools"
            ) {
                continue;
            }
            target.insert(key.clone(), value.clone());
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_urls_without_duplicate_slashes() {
        assert_eq!(
            join_url("https://api.openai.com/v1/", "/models").unwrap(),
            "https://api.openai.com/v1/models"
        );
        assert!(join_url("", "models").is_err());
        assert!(join_url("ftp://x", "models").is_err());
    }
}
