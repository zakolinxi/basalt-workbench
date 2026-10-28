use url::Url;

const OPENAI_CHAT_SUFFIXES: &[&str] = &["/chat/completions", "/completions", "/models"];
const OPENAI_RESPONSES_SUFFIXES: &[&str] = &["/responses", "/models"];
const ANTHROPIC_SUFFIXES: &[&str] = &["/messages", "/models"];
const OPENAI_IMAGE_SUFFIXES: &[&str] = &[
    "/images/generations",
    "/images/edits",
    "/images/variations",
    "/models",
];
const OLLAMA_SUFFIXES: &[&str] = &[
    "/api/chat",
    "/api/generate",
    "/api/tags",
    "/api/ps",
    "/api/show",
    "/api/embed",
    "/v1/chat/completions",
    "/v1/models",
    "/api",
    "/v1",
];

fn endpoint_suffixes(protocol: &str) -> &'static [&'static str] {
    match protocol {
        "openai_chat" => OPENAI_CHAT_SUFFIXES,
        "openai_responses" => OPENAI_RESPONSES_SUFFIXES,
        "anthropic" => ANTHROPIC_SUFFIXES,
        "openai_images" => OPENAI_IMAGE_SUFFIXES,
        "ollama" => OLLAMA_SUFFIXES,
        _ => &[],
    }
}

fn is_version_segment(segment: &str) -> bool {
    let lower = segment.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix('v') else {
        return false;
    };
    let mut chars = rest.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_digit()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

fn is_local_host(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    if host.starts_with("localhost")
        || host.starts_with("127.")
        || host.starts_with("0.0.0.0")
        || host.starts_with("[::1]")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("host.docker.internal")
        || host.ends_with(".local")
    {
        return true;
    }
    if let Some(rest) = host.strip_prefix("172.") {
        if let Some(second) = rest.split('.').next() {
            if let Ok(octet) = second.parse::<u8>() {
                return (16..=31).contains(&octet);
            }
        }
    }
    false
}

/// Fills in the scheme and API version path so only a host is required.
///
/// Existing base URLs are left alone apart from trimming known endpoint
/// suffixes (for example `/v1/chat/completions`). Ollama keeps the root URL,
/// Gemini uses `/v1beta`, every other protocol uses `/v1`.
pub fn normalize_base_url(raw: &str, protocol: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let mut input = raw.to_string();
    if !input.contains("://") {
        let local = protocol == "ollama" || is_local_host(raw);
        input = format!("{}{}", if local { "http://" } else { "https://" }, input);
    }
    let Ok(mut url) = Url::parse(&input) else {
        return raw.to_string();
    };
    if !matches!(url.scheme(), "http" | "https") {
        return raw.to_string();
    }
    url.set_query(None);
    url.set_fragment(None);

    let mut path = url.path().trim_end_matches('/').to_string();
    loop {
        let mut stripped = false;
        for suffix in endpoint_suffixes(protocol) {
            if let Some(value) = path.strip_suffix(suffix) {
                path = value.trim_end_matches('/').to_string();
                stripped = true;
                break;
            }
        }
        if !stripped {
            break;
        }
    }
    if matches!(protocol, "gemini" | "gemini_images") {
        if let Some(index) = path.find("/models") {
            path.truncate(index);
        }
    }

    let version = if matches!(protocol, "gemini" | "gemini_images") {
        "v1beta"
    } else {
        "v1"
    };
    let last = path
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let incomplete_version = last == "v"
        || (version == "v1beta" && matches!(last.as_str(), "v1b" | "v1be" | "v1bet"));
    if incomplete_version {
        path = match path.rsplit_once('/') {
            Some((head, _)) => format!("{head}/{version}"),
            None => format!("/{version}"),
        };
    }

    let has_version = path.split('/').any(is_version_segment);
    if protocol != "ollama" && !has_version {
        path = if path.is_empty() {
            format!("/{version}")
        } else {
            format!("{path}/{version}")
        };
    }
    url.set_path(if path.is_empty() { "/" } else { &path });
    url.to_string().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::normalize_base_url;

    fn openai(raw: &str) -> String {
        normalize_base_url(raw, "openai_chat")
    }

    #[test]
    fn adds_scheme_and_version() {
        assert_eq!(openai("api.deepseek.com"), "https://api.deepseek.com/v1");
        assert_eq!(
            openai("https://api.deepseek.com"),
            "https://api.deepseek.com/v1"
        );
        assert_eq!(
            openai("https://api.deepseek.com/v1/"),
            "https://api.deepseek.com/v1"
        );
    }

    #[test]
    fn keeps_existing_version_paths() {
        assert_eq!(
            openai("https://open.bigmodel.cn/api/paas/v4"),
            "https://open.bigmodel.cn/api/paas/v4"
        );
        assert_eq!(
            openai("https://dashscope.aliyuncs.com/compatible-mode/v1"),
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
        assert_eq!(
            normalize_base_url("https://api.anthropic.com/v1", "anthropic"),
            "https://api.anthropic.com/v1"
        );
    }

    #[test]
    fn strips_endpoint_suffixes() {
        assert_eq!(
            openai("https://api.openai.com/v1/chat/completions"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_base_url("https://api.anthropic.com/v1/messages", "anthropic"),
            "https://api.anthropic.com/v1"
        );
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/images/generations", "openai_images"),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn gemini_uses_v1beta() {
        assert_eq!(
            normalize_base_url("https://generativelanguage.googleapis.com", "gemini"),
            "https://generativelanguage.googleapis.com/v1beta"
        );
        assert_eq!(
            normalize_base_url(
                "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent",
                "gemini"
            ),
            "https://generativelanguage.googleapis.com/v1beta"
        );
        assert_eq!(
            normalize_base_url("generativelanguage.googleapis.com/v1b", "gemini"),
            "https://generativelanguage.googleapis.com/v1beta"
        );
    }

    #[test]
    fn ollama_keeps_root_and_uses_http() {
        assert_eq!(
            normalize_base_url("127.0.0.1:11434", "ollama"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            normalize_base_url("http://127.0.0.1:11434/api", "ollama"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            normalize_base_url("http://127.0.0.1:11434/v1", "ollama"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            normalize_base_url("http://127.0.0.1:11434/api/v1", "ollama"),
            "http://127.0.0.1:11434"
        );
    }

    #[test]
    fn local_gateways_default_to_http() {
        assert_eq!(
            openai("192.168.1.8:8080"),
            "http://192.168.1.8:8080/v1"
        );
        assert_eq!(openai("localhost:3000"), "http://localhost:3000/v1");
        assert_eq!(
            openai("https://api.openai.com/v"),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn trims_query_and_trailing_slashes() {
        assert_eq!(
            openai("https://api.openai.com/?key=1"),
            "https://api.openai.com/v1"
        );
        assert_eq!(normalize_base_url("", "openai_chat"), "");
    }
}
