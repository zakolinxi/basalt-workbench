use std::net::IpAddr;
use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::error::{AppError, AppResult};
use crate::providers::ToolSchema;
use crate::state::AppState;

use super::tools::ToolResult;

const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;

pub fn schemas() -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            name: "web_search".into(),
            description: "联网搜索公开网页，返回标题、URL 与摘要。用于最新版本、官方文档、新闻等外部信息。查询中不要包含 API Key、令牌或隐私数据。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "搜索关键词"},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": 10}
                },
                "required": ["query"]
            }),
        },
        ToolSchema {
            name: "fetch_url".into(),
            description: "抓取并阅读一个公开 HTTPS 页面，转换为纯文本。仅允许公网地址，拒绝 localhost、内网与凭据 URL。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "公开 HTTPS URL"},
                    "max_chars": {"type": "integer", "minimum": 1000, "maximum": 50000}
                },
                "required": ["url"]
            }),
        },
    ]
}

pub async fn execute(
    state: &AppState,
    name: &str,
    args: &Value,
    cancel: &CancellationToken,
) -> AppResult<ToolResult> {
    match name {
        "web_search" => search(state, args, cancel).await,
        "fetch_url" => fetch_page(state, args, cancel).await,
        other => Err(AppError::msg(format!("未知网络工具：{other}"))),
    }
}

fn user_agent(state: &AppState) -> String {
    let conn = state.db();
    let settings = crate::db::load_settings(&conn);
    if settings.user_agent.trim().is_empty() {
        crate::models::default_user_agent()
    } else {
        settings.user_agent
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
                || v4.is_documentation()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

async fn validate_public_url(raw: &str) -> AppResult<Url> {
    let parsed = Url::parse(raw).map_err(|e| AppError::msg(format!("URL 无效：{e}")))?;
    if parsed.scheme() != "https" {
        return Err(AppError::msg("仅允许访问 HTTPS 公网地址"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::msg("URL 不能包含用户名或密码"));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::msg("URL 缺少主机名"))?
        .to_string();
    if host.eq_ignore_ascii_case("localhost") {
        return Err(AppError::msg("拒绝访问本地地址"));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            return Err(AppError::msg("拒绝访问内网地址"));
        }
    } else {
        let port = parsed.port_or_known_default().unwrap_or(443);
        let addrs = tokio::net::lookup_host((host.as_str(), port))
            .await
            .map_err(|e| AppError::msg(format!("无法解析主机名：{e}")))?;
        let mut any = false;
        for addr in addrs {
            any = true;
            if is_private_ip(&addr.ip()) {
                return Err(AppError::msg("拒绝访问解析到内网的地址"));
            }
        }
        if !any {
            return Err(AppError::msg("主机名没有可用的公网地址"));
        }
    }
    Ok(parsed)
}

struct FetchedPage {
    url: Url,
    content_type: String,
    body: Vec<u8>,
    truncated: bool,
}

fn web_client() -> AppResult<&'static reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .build()?;
    let _ = CLIENT.set(client);
    Ok(CLIENT.get().expect("web client initialized"))
}

async fn fetch_public(
    state: &AppState,
    raw_url: &str,
    cancel: &CancellationToken,
) -> AppResult<FetchedPage> {
    let mut current = validate_public_url(raw_url).await?;
    for _ in 0..4 {
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        let request = web_client()?
            .get(current.clone())
            .header(reqwest::header::USER_AGENT, user_agent(state))
            .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml,text/plain,application/json;q=0.9,*/*;q=0.5")
            .timeout(Duration::from_secs(25));
        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(AppError::Cancelled),
            result = request.send() => result?,
        };
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| AppError::msg("重定向缺少 Location"))?;
            let next = current
                .join(location)
                .map_err(|e| AppError::msg(format!("重定向地址无效：{e}")))?;
            current = validate_public_url(next.as_str()).await?;
            continue;
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::msg(format!(
                "页面请求失败（HTTP {}）：{}",
                status.as_u16(),
                body.chars().take(300).collect::<String>()
            )));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let mut body = Vec::new();
        let mut truncated = false;
        let mut response = response;
        while let Some(chunk) = tokio::select! {
            _ = cancel.cancelled() => return Err(AppError::Cancelled),
            chunk = response.chunk() => chunk?,
        } {
            if body.len() + chunk.len() > MAX_PAGE_BYTES {
                let remaining = MAX_PAGE_BYTES.saturating_sub(body.len());
                body.extend_from_slice(&chunk[..remaining]);
                truncated = true;
                break;
            }
            body.extend_from_slice(&chunk);
        }
        return Ok(FetchedPage {
            url: current,
            content_type,
            body,
            truncated,
        });
    }
    Err(AppError::msg("重定向次数过多"))
}

fn decode_entities(text: &str) -> String {
    static NUMERIC: OnceLock<Regex> = OnceLock::new();
    let numeric = NUMERIC.get_or_init(|| Regex::new(r"&#(x?[0-9a-fA-F]+);").unwrap());
    let mut out = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&ensp;", " ")
        .replace("&emsp;", " ")
        .replace("&middot;", "·")
        .replace("&copy;", "©")
        .replace("&reg;", "®")
        .replace("&trade;", "™")
        .replace("&rsquo;", "'")
        .replace("&lsquo;", "'")
        .replace("&ldquo;", "\"")
        .replace("&rdquo;", "\"");
    out = numeric
        .replace_all(&out, |caps: &regex::Captures| {
            let raw = &caps[1];
            let value = if let Some(hex) = raw.strip_prefix('x') {
                u32::from_str_radix(hex, 16).ok()
            } else {
                raw.parse::<u32>().ok()
            };
            value
                .and_then(char::from_u32)
                .map(|ch| ch.to_string())
                .unwrap_or_default()
        })
        .to_string();
    out
}

fn strip_tags(html: &str) -> String {
    static SCRIPT: OnceLock<Regex> = OnceLock::new();
    static STYLE: OnceLock<Regex> = OnceLock::new();
    static COMMENT: OnceLock<Regex> = OnceLock::new();
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    let script = SCRIPT.get_or_init(|| Regex::new(r"(?is)<script\b[^>]*>.*?</script>").unwrap());
    let style = STYLE.get_or_init(|| Regex::new(r"(?is)<style\b[^>]*>.*?</style>").unwrap());
    let comment = COMMENT.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").unwrap());
    let block = BLOCK.get_or_init(|| {
        Regex::new(r"(?i)</?(p|div|section|article|header|footer|li|ul|ol|h[1-6]|tr|br|hr)\b[^>]*>")
            .unwrap()
    });
    let tag = TAG.get_or_init(|| Regex::new(r"(?s)<[^>]+>").unwrap());
    let text = script.replace_all(html, " ");
    let text = style.replace_all(&text, " ");
    let text = comment.replace_all(&text, " ");
    let text = block.replace_all(&text, "\n");
    let text = tag.replace_all(&text, " ");
    let text = decode_entities(&text);
    let mut lines = Vec::new();
    for line in text.lines() {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !collapsed.is_empty() {
            lines.push(collapsed);
        }
    }
    lines.join("\n")
}

fn page_title(html: &str) -> Option<String> {
    static TITLE: OnceLock<Regex> = OnceLock::new();
    let title = TITLE.get_or_init(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap());
    title
        .captures(html)
        .map(|caps| decode_entities(&caps[1]).trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn fetch_page(
    state: &AppState,
    args: &Value,
    cancel: &CancellationToken,
) -> AppResult<ToolResult> {
    let raw_url = args
        .get("url")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    if raw_url.is_empty() {
        return Err(AppError::msg("fetch_url 需要 url"));
    }
    if raw_url.len() > 4096 {
        return Err(AppError::msg("URL 过长"));
    }
    let max_chars = args
        .get("max_chars")
        .and_then(|value| value.as_u64())
        .unwrap_or(12_000)
        .clamp(1_000, 50_000) as usize;
    let page = fetch_public(state, raw_url, cancel).await?;
    let raw = String::from_utf8_lossy(&page.body);
    let is_html = page.content_type.contains("text/html") || raw.trim_start().starts_with('<');
    let title = is_html.then(|| page_title(&raw)).flatten();
    let text = if is_html { strip_tags(&raw) } else { raw.to_string() };
    let (content, cut) = crate::files::truncate_chars(&text, max_chars);
    let summary = format!(
        "读取网页：{}（{} 字符{}）",
        page.url.host_str().unwrap_or("网页"),
        content.chars().count(),
        if cut || page.truncated { "，已截断" } else { "" }
    );
    Ok(ToolResult {
        data: json!({
            "url": page.url.as_str(),
            "title": title,
            "contentType": page.content_type,
            "content": content,
            "truncated": cut || page.truncated
        }),
        summary,
    })
}

fn unwrap_ddg_link(href: &str) -> String {
    let normalized = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href.to_string()
    };
    let Ok(parsed) = Url::parse(&normalized) else {
        return normalized;
    };
    if let Some((_, value)) = parsed.query_pairs().find(|(key, _)| key == "uddg") {
        return value.to_string();
    }
    normalized
}

fn search_duckduckgo(html: &str, limit: usize) -> Vec<Value> {
    static LINK: OnceLock<Regex> = OnceLock::new();
    static SNIPPET: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| {
        Regex::new(r#"(?is)<a[^>]*class="[^"]*result__a[^"]*"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#)
            .unwrap()
    });
    let snippet = SNIPPET.get_or_init(|| {
        Regex::new(r#"(?is)<a[^>]*class="[^"]*result__snippet[^"]*"[^>]*>(.*?)</a>"#).unwrap()
    });
    let snippets: Vec<String> = snippet
        .captures_iter(html)
        .map(|caps| strip_tags(&caps[1]))
        .collect();
    let mut results = Vec::new();
    for (index, caps) in link.captures_iter(html).enumerate() {
        if results.len() >= limit {
            break;
        }
        let url = unwrap_ddg_link(&caps[1]);
        if url.is_empty() {
            continue;
        }
        results.push(json!({
            "title": strip_tags(&caps[2]),
            "url": url,
            "snippet": snippets.get(index).cloned().unwrap_or_default()
        }));
    }
    results
}

fn search_bing(html: &str, limit: usize) -> Vec<Value> {
    static LINK: OnceLock<Regex> = OnceLock::new();
    static SNIPPET: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| {
        Regex::new(r#"(?is)<h2[^>]*>.*?<a[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#).unwrap()
    });
    let snippet = SNIPPET.get_or_init(|| Regex::new(r"(?is)<p[^>]*>(.*?)</p>").unwrap());
    let mut results = Vec::new();
    for chunk in html.split(r#"<li class="b_algo"#).skip(1) {
        if results.len() >= limit {
            break;
        }
        let Some(caps) = link.captures(chunk) else {
            continue;
        };
        let url = decode_entities(&caps[1]);
        if !url.starts_with("http") {
            continue;
        }
        let snippet_text: String = snippet
            .captures(chunk)
            .map(|caps| strip_tags(&caps[1]))
            .unwrap_or_default()
            .chars()
            .take(500)
            .collect();
        results.push(json!({
            "title": strip_tags(&caps[2]),
            "url": url,
            "snippet": snippet_text
        }));
    }
    results
}

async fn search(
    state: &AppState,
    args: &Value,
    cancel: &CancellationToken,
) -> AppResult<ToolResult> {
    let query = args
        .get("query")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if query.is_empty() {
        return Err(AppError::msg("web_search 需要非空 query"));
    }
    if query.chars().count() > 400 {
        return Err(AppError::msg("搜索词过长"));
    }
    let limit = args
        .get("max_results")
        .and_then(|value| value.as_u64())
        .unwrap_or(6)
        .clamp(1, 10) as usize;
    let mut targets = Vec::new();
    let mut bing = Url::parse("https://www.bing.com/search").unwrap();
    bing.query_pairs_mut()
        .append_pair("q", &query)
        .append_pair("count", &limit.to_string());
    targets.push(("bing", bing));
    let mut ddg = Url::parse("https://html.duckduckgo.com/html/").unwrap();
    ddg.query_pairs_mut().append_pair("q", &query);
    targets.push(("duckduckgo", ddg));

    let mut provider = "bing";
    let mut results = Vec::new();
    let mut last_error: Option<AppError> = None;
    for (name, url) in targets {
        let page = match fetch_public(state, url.as_str(), cancel).await {
            Ok(page) => page,
            Err(err) => {
                last_error = Some(err);
                continue;
            }
        };
        let html = String::from_utf8_lossy(&page.body);
        let found = if name == "duckduckgo" {
            search_duckduckgo(&html, limit)
        } else {
            search_bing(&html, limit)
        };
        if !found.is_empty() {
            provider = name;
            results = found;
            break;
        }
    }
    if results.is_empty() {
        if let Some(err) = last_error {
            return Err(AppError::msg(format!(
                "联网搜索失败：{}",
                err.user_message()
            )));
        }
    }

    Ok(ToolResult {
        data: json!({
            "query": query,
            "provider": provider,
            "results": results,
            "count": results.len()
        }),
        summary: format!("联网搜索「{query}」：{} 条结果", results.len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_decodes_entities() {
        let text = strip_tags(
            "<html><head><title>T</title></head><body><h1>A &amp; B</h1><p>Hello<br>world</p><script>x()</script></body></html>",
        );
        assert!(text.contains("A & B"));
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("x()"));
    }

    #[test]
    fn unwraps_ddg_redirect() {
        assert_eq!(
            unwrap_ddg_link("//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa%3Fb%3D1"),
            "https://example.com/a?b=1"
        );
    }

    #[test]
    fn parses_ddg_results() {
        let html = r#"
        <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com">Example</a>
        <a class="result__snippet">An example site</a>
        "#;
        let results = search_duckduckgo(html, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["url"], "https://example.com");
        assert_eq!(results[0]["title"], "Example");
        assert_eq!(results[0]["snippet"], "An example site");
    }

    #[test]
    fn parses_bing_results() {
        let html = r#"
        <li class="b_algo"><h2><a href="https://tauri.app/">Tauri</a></h2><p>Build apps</p></li>
        <li class="b_algo"><h2><a href="https://example.com/">Example</a></h2><p>Example site</p></li>
        "#;
        let results = search_bing(html, 5);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["url"], "https://tauri.app/");
        assert_eq!(results[0]["title"], "Tauri");
        assert_eq!(results[0]["snippet"], "Build apps");
    }

    #[tokio::test]
    async fn rejects_non_public_urls() {
        assert!(validate_public_url("http://example.com").await.is_err());
        assert!(validate_public_url("https://localhost/").await.is_err());
        assert!(validate_public_url("https://127.0.0.1/").await.is_err());
        assert!(validate_public_url("https://[::1]/").await.is_err());
        assert!(validate_public_url("https://user:pass@example.com/")
            .await
            .is_err());
    }
}
