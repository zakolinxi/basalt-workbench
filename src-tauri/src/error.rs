use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("数据库错误：{0}")]
    Db(#[from] rusqlite::Error),
    #[error("文件系统错误：{0}")]
    Io(#[from] std::io::Error),
    #[error("网络请求失败：{0}")]
    Http(#[from] reqwest::Error),
    #[error("{message}")]
    HttpStatus { status: u16, message: String },
    #[error("数据解析失败：{0}")]
    Json(#[from] serde_json::Error),
    #[error("密钥存储错误：{0}")]
    Keyring(#[from] keyring::Error),
    #[error("操作已取消")]
    Cancelled,
}

impl AppError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Msg(message.into())
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled) || self.to_string().contains("操作已取消")
    }

    /// Transient failures worth retrying without asking the user. Permission,
    /// validation and parsing errors are deliberately excluded.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http(err) => {
                err.is_timeout() || err.is_connect() || err.is_request() || err.is_body()
            }
            Self::HttpStatus { status, .. } => {
                matches!(*status, 408 | 409 | 425 | 429) || (500..=599).contains(status)
            }
            Self::Msg(message) => {
                message.contains("服务端请求超时")
                    || message.contains("供应商服务异常")
                    || message.contains("请求过于频繁")
            }
            _ => false,
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Http(err) => normalize_reqwest_error(err),
            other => other.to_string(),
        }
    }
}

impl From<AppError> for String {
    fn from(value: AppError) -> Self {
        value.user_message()
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.user_message())
    }
}

pub type AppResult<T> = Result<T, AppError>;
pub type CmdResult<T> = Result<T, String>;

fn normalize_reqwest_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        return "网络请求超时，请检查 Base URL、网络或代理设置。".into();
    }
    if err.is_connect() {
        return "无法连接到服务，请检查 Base URL、网络或代理设置。".into();
    }
    if err.is_decode() {
        return format!("服务返回了无法解析的数据：{err}");
    }
    format!("网络请求失败：{err}")
}

pub fn http_status_error(status: reqwest::StatusCode, body: &str) -> AppError {
    let friendly = match status.as_u16() {
        400 => "请求参数无效".to_string(),
        401 => "认证失败：API Key 无效或已过期".to_string(),
        403 => "权限不足：当前密钥无法访问该资源".to_string(),
        404 => "接口或模型不存在：请检查 Base URL 与模型 ID".to_string(),
        408 => "服务端请求超时".to_string(),
        413 => "请求内容过大".to_string(),
        429 => "请求过于频繁或额度不足".to_string(),
        500..=599 => "供应商服务异常".to_string(),
        _ => format!("请求失败（HTTP {}）", status.as_u16()),
    };
    let snippet: String = body.chars().take(600).collect();
    let message = if snippet.trim().is_empty() {
        friendly
    } else {
        format!("{friendly}：{snippet}")
    };
    AppError::HttpStatus {
        status: status.as_u16(),
        message,
    }
}

pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for part in text.split_whitespace() {
        let lower = part.to_ascii_lowercase();
        if lower.starts_with("sk-") || lower.starts_with("xai-") || lower.starts_with("aiza") {
            out.push_str("[REDACTED] ");
        } else {
            out.push_str(part);
            out.push(' ');
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secret_tokens() {
        let text = "failed with sk-abc123 and normal text";
        assert!(!redact(text).contains("sk-abc123"));
    }

    #[test]
    fn http_errors_are_friendly() {
        let err = http_status_error(reqwest::StatusCode::UNAUTHORIZED, "invalid key");
        assert!(err.to_string().contains("认证失败"));
    }

    #[test]
    fn only_transient_http_status_retries() {
        assert!(http_status_error(reqwest::StatusCode::TOO_MANY_REQUESTS, "busy").is_retryable());
        assert!(http_status_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "down").is_retryable());
        assert!(!http_status_error(reqwest::StatusCode::UNAUTHORIZED, "bad key").is_retryable());
        assert!(!http_status_error(reqwest::StatusCode::NOT_FOUND, "missing").is_retryable());
    }
}
