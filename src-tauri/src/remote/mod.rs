//! Remote gateway: lets a paired phone drive this desktop app over the LAN.
//!
//! The desktop stays the single source of truth — projects, providers, keys,
//! SQLite and the Agent runtime all keep running here. The phone is a thin
//! control surface that speaks JSON-RPC over HTTP and receives the same event
//! stream the desktop UI gets, delivered as Server-Sent Events.
//!
//! Security model:
//! - Off by default; nothing listens until the user enables it in Settings.
//! - Every call needs a bearer token that only reaches a device after the user
//!   types the 6-digit pairing code shown on the desktop.
//! - The pairing code expires after ten minutes and repeated wrong guesses
//!   lock pairing out for a while.
//! - The listener binds to the LAN, never to a public interface by itself.

mod dispatch;

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream;
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Listener, Manager};
use tokio::sync::{broadcast, oneshot};

use crate::commands::remote::{read_setting, write_setting};
use crate::events::{
    EVENT_CHAT, EVENT_IMAGE, EVENT_TASK, EVENT_TERMINAL, EVENT_TERMINAL_EXIT,
};
use crate::state::AppState;

pub const DEFAULT_PORT: u16 = 7717;
const CODE_TTL: Duration = Duration::from_secs(600);
const MAX_PAIR_FAILURES: u32 = 6;
const LOCKOUT: Duration = Duration::from_secs(60);

static ASSETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../dist");
static GATEWAY: OnceLock<Mutex<Option<Arc<Gateway>>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Arc<Gateway>>> {
    GATEWAY.get_or_init(|| Mutex::new(None))
}

struct Pairing {
    code: String,
    issued: Instant,
    failures: u32,
    locked_until: Option<Instant>,
}

pub struct Gateway {
    app: AppHandle,
    port: u16,
    token: Arc<RwLock<String>>,
    pairing: Mutex<Pairing>,
    clients: Arc<AtomicUsize>,
    events: broadcast::Sender<String>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

/// Bridges desktop events into whichever gateway is currently listening.
/// Registered once at startup so starting and stopping the gateway never has to
/// juggle listener handles.
static EVENT_BUS: OnceLock<Mutex<Option<broadcast::Sender<String>>>> = OnceLock::new();

fn bus() -> &'static Mutex<Option<broadcast::Sender<String>>> {
    EVENT_BUS.get_or_init(|| Mutex::new(None))
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    /// Whether the user wants the gateway on (persisted), not just running.
    pub enabled: bool,
    /// Whether a listener is actually accepting connections right now.
    pub running: bool,
    pub port: u16,
    /// `host:port` the phone should connect to, using the LAN address.
    pub address: String,
    pub code: String,
    pub code_expires_in: u64,
    pub clients: usize,
    /// Set when the last start attempt failed, e.g. the port is taken.
    pub error: Option<String>,
}

fn six_digit_code() -> String {
    let raw = uuid::Uuid::new_v4().into_bytes();
    let value = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
    format!("{:06}", value % 1_000_000)
}

fn new_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Best-effort LAN address. Opening a UDP socket and "connecting" it does not
/// send packets, it only asks the OS which local interface would be used.
pub fn lan_address() -> String {
    let probe = || -> Option<String> {
        let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect("8.8.8.8:80").ok()?;
        match socket.local_addr().ok()? {
            SocketAddr::V4(address) => Some(address.ip().to_string()),
            SocketAddr::V6(address) => Some(address.ip().to_string()),
        }
    };
    probe().unwrap_or_else(|| "127.0.0.1".to_string())
}

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "webmanifest" => "application/manifest+json",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "ok": false, "error": "未授权：请在桌面端完成配对" })),
    )
        .into_response()
}

fn tokens_match(expected: &str, presented: &str) -> bool {
    if expected.is_empty() || expected.len() != presented.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in expected.bytes().zip(presented.bytes()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn presented_token(headers: &HeaderMap, uri: &Uri) -> Option<String> {
    if let Some(value) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        let value = value.trim();
        let token = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
            .unwrap_or(value);
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    uri.query().and_then(|query| {
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "token" && !value.is_empty()).then(|| value.to_string())
        })
    })
}

impl Gateway {
    fn authorized(&self, headers: &HeaderMap, uri: &Uri) -> bool {
        let expected = self.token.read().map(|t| t.clone()).unwrap_or_default();
        presented_token(headers, uri)
            .map(|token| tokens_match(&expected, &token))
            .unwrap_or(false)
    }

    /// Returns the current pairing code, minting a new one when it has expired.
    fn current_code(&self) -> (String, u64) {
        let mut pairing = self
            .pairing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let elapsed = pairing.issued.elapsed();
        if elapsed >= CODE_TTL {
            pairing.code = six_digit_code();
            pairing.issued = Instant::now();
            pairing.failures = 0;
            return (pairing.code.clone(), CODE_TTL.as_secs());
        }
        (pairing.code.clone(), (CODE_TTL - elapsed).as_secs())
    }

    fn rotate_code(&self) -> (String, u64) {
        let mut pairing = self
            .pairing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pairing.code = six_digit_code();
        pairing.issued = Instant::now();
        pairing.failures = 0;
        pairing.locked_until = None;
        (pairing.code.clone(), CODE_TTL.as_secs())
    }
}

// ---------------------------------------------------------------- handlers

async fn info(State(gateway): State<Arc<Gateway>>) -> Response {
    Json(json!({
        "name": "Basalt",
        "version": env!("CARGO_PKG_VERSION"),
        "port": gateway.port,
        "host": lan_address(),
        "requirePairing": true,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct PairRequest {
    code: String,
}

async fn pair(State(gateway): State<Arc<Gateway>>, Json(body): Json<PairRequest>) -> Response {
    {
        let mut pairing = gateway
            .pairing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(until) = pairing.locked_until {
            if Instant::now() < until {
                let wait = until.saturating_duration_since(Instant::now()).as_secs() + 1;
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({ "ok": false, "error": format!("尝试过于频繁，请 {wait} 秒后再试") })),
                )
                    .into_response();
            }
            pairing.locked_until = None;
        }
        if pairing.issued.elapsed() >= CODE_TTL {
            pairing.code = six_digit_code();
            pairing.issued = Instant::now();
            pairing.failures = 0;
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": "配对码已过期，请在电脑上查看新配对码" })),
            )
                .into_response();
        }
        let submitted = body.code.trim().replace(' ', "");
        if !tokens_match(&pairing.code, &submitted) {
            pairing.failures += 1;
            if pairing.failures >= MAX_PAIR_FAILURES {
                pairing.locked_until = Some(Instant::now() + LOCKOUT);
                pairing.failures = 0;
            }
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "ok": false, "error": "配对码不正确" })),
            )
                .into_response();
        }
        pairing.failures = 0;
    }

    let token = gateway.token.read().map(|t| t.clone()).unwrap_or_default();
    let _ = gateway.events.send(": paired\n\n".to_string());
    Json(json!({
        "ok": true,
        "token": token,
        "port": gateway.port,
        "host": lan_address(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
    .into_response()
}

#[derive(Deserialize)]
struct RpcRequest {
    cmd: String,
    #[serde(default)]
    args: Value,
}

async fn rpc(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    uri: Uri,
    Json(request): Json<RpcRequest>,
) -> Response {
    if !gateway.authorized(&headers, &uri) {
        return unauthorized();
    }
    let app = match current_app() {
        Some(app) => app,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "ok": false, "error": "桌面端尚未就绪" })),
            )
                .into_response()
        }
    };

    // Dispatch decides for itself whether a command belongs on the blocking
    // pool, so it can simply be awaited here.
    match dispatch::call(&app, &request.cmd, request.args).await {
        Ok(data) => Json(json!({ "ok": true, "data": data })).into_response(),
        Err(error) => Json(json!({ "ok": false, "error": error })).into_response(),
    }
}

struct SseState {
    receiver: broadcast::Receiver<String>,
    clients: Arc<AtomicUsize>,
}

impl Drop for SseState {
    fn drop(&mut self) {
        self.clients.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn sse(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    if !gateway.authorized(&headers, &uri) {
        return unauthorized();
    }
    gateway.clients.fetch_add(1, Ordering::SeqCst);
    let state = SseState {
        receiver: gateway.events.subscribe(),
        clients: gateway.clients.clone(),
    };

    let stream = stream::unfold(state, |mut state| async move {
        let frame = tokio::select! {
            received = state.receiver.recv() => match received {
                Ok(frame) => frame,
                Err(broadcast::error::RecvError::Lagged(_)) => ": lagged\n\n".to_string(),
                Err(broadcast::error::RecvError::Closed) => return None,
            },
            _ = tokio::time::sleep(Duration::from_secs(15)) => ": ping\n\n".to_string(),
        };
        Some((
            Ok::<_, std::convert::Infallible>(Bytes::from(frame)),
            state,
        ))
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache, no-transform")
        .header(header::CONNECTION, "keep-alive")
        .header("x-accel-buffering", "no")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn assets(uri: Uri) -> Response {
    let requested = uri.path().trim_start_matches('/');
    let requested = if requested.is_empty() { "index.html" } else { requested };

    let file = ASSETS.get_file(requested).or_else(|| {
        // Unknown path without a file extension: let the SPA router handle it.
        if requested.contains('.') {
            None
        } else {
            ASSETS.get_file("index.html")
        }
    });

    let Some(file) = file else {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    };

    let path = file.path().to_string_lossy().replace('\\', "/");
    let cache = if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime_for(&path)),
            (header::CACHE_CONTROL, cache),
        ],
        file.contents().to_vec(),
    )
        .into_response()
}

// ---------------------------------------------------------------- lifecycle

fn current_app() -> Option<AppHandle> {
    slot()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|gateway| gateway.app.clone()))
}

impl Gateway {
    fn log(&self, message: &str) {
        if let Some(state) = self.app.try_state::<AppState>() {
            state.log("ERROR", message);
        }
    }
}

/// Forwards the same live events the desktop window receives to paired phones.
/// Called once at startup; it stays registered and becomes a no-op while the
/// gateway is off.
pub fn install_event_forwarder(app: &AppHandle) {
    for name in [
        EVENT_CHAT,
        EVENT_TASK,
        EVENT_IMAGE,
        EVENT_TERMINAL,
        EVENT_TERMINAL_EXIT,
    ] {
        app.listen(name, move |event: tauri::Event| {
            let sender = bus().lock().ok().and_then(|guard| guard.clone());
            let Some(sender) = sender else {
                return;
            };
            // The payload is already JSON; splicing it in avoids a
            // serialize/deserialize round trip on every streamed token.
            let frame = format!(
                "data: {{\"event\":\"{name}\",\"payload\":{}}}\n\n",
                event.payload()
            );
            // A send error only means no phone is listening right now.
            let _ = sender.send(frame);
        });
    }
}

/// Starts the listener if it is not already running on `port`.
pub fn start(app: &AppHandle, port: u16, token: String) -> Result<RemoteStatus, String> {
    {
        let guard = slot().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(existing) = guard.as_ref() {
            if existing.port == port {
                drop(guard);
                return status_from_app(app);
            }
        }
    }
    stop();

    // Binding synchronously lets a busy port be reported straight back to the
    // settings panel. The listener is only handed to Tokio inside the task,
    // because `from_std` needs a reactor and `start` runs on the UI thread.
    let std_listener = std::net::TcpListener::bind(("0.0.0.0", port))
        .map_err(|error| format!("端口 {port} 无法监听：{error}"))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|error| format!("无法配置监听端口：{error}"))?;

    let (events, _) = broadcast::channel::<String>(4096);
    let (stop_tx, stop_rx) = oneshot::channel::<()>();

    let gateway = Arc::new(Gateway {
        app: app.clone(),
        port,
        token: Arc::new(RwLock::new(token)),
        pairing: Mutex::new(Pairing {
            code: six_digit_code(),
            issued: Instant::now(),
            failures: 0,
            locked_until: None,
        }),
        clients: Arc::new(AtomicUsize::new(0)),
        events: events.clone(),
        shutdown: Mutex::new(Some(stop_tx)),
    });

    *bus().lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(events.clone());

    let router = Router::new()
        .route("/api/info", get(info))
        .route("/api/pair", post(pair))
        // Attachments travel as base64 inside the JSON envelope, and the
        // backend accepts up to 25 MB per file, so the default 2 MB axum body
        // limit is far too small here.
        .route(
            "/api/rpc",
            post(rpc).layer(DefaultBodyLimit::max(48 * 1024 * 1024)),
        )
        .route("/api/events", get(sse))
        .fallback(get(assets))
        .with_state(gateway.clone());

    let serving = gateway.clone();
    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(listener) => listener,
            Err(error) => {
                serving.log(&format!("无法创建监听器：{error}"));
                return;
            }
        };
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = stop_rx.await;
            })
            .await;
        drop(serving);
    });

    *slot().lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(gateway);
    status_from_app(app)
}

/// Stops the listener and drops every paired SSE connection.
pub fn stop() {
    let existing = slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    *bus().lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    if let Some(gateway) = existing {
        if let Some(sender) = gateway
            .shutdown
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = sender.send(());
        }
    }
}

pub fn is_running() -> bool {
    slot()
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

pub fn rotate_pairing_code() -> Option<String> {
    slot()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|gateway| gateway.rotate_code().0))
}

pub fn status(conn: &rusqlite::Connection) -> RemoteStatus {
    let enabled = read_setting(conn, "remoteEnabled")
        .map(|value| value == "1")
        .unwrap_or(false);
    let port = read_setting(conn, "remotePort")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    let guard = slot().lock().ok();
    let running = guard.as_ref().map(|gateway| gateway.as_ref().cloned());

    match running.flatten() {
        Some(gateway) => {
            let (code, expires) = gateway.current_code();
            RemoteStatus {
                enabled,
                running: true,
                port: gateway.port,
                address: format!("{}:{}", lan_address(), gateway.port),
                code,
                code_expires_in: expires,
                clients: gateway.clients.load(Ordering::SeqCst),
                error: None,
            }
        }
        None => RemoteStatus {
            enabled,
            running: false,
            port,
            address: format!("{}:{}", lan_address(), port),
            code: String::new(),
            code_expires_in: 0,
            clients: 0,
            error: None,
        },
    }
}

/// Starts the gateway using the persisted configuration.
pub fn start_from_settings(app: &AppHandle, state: &AppState) -> Result<RemoteStatus, String> {
    let (enabled, port, token) = {
        let conn = state.db();
        let enabled = read_setting(&conn, "remoteEnabled")
            .map(|value| value == "1")
            .unwrap_or(false);
        let port = read_setting(&conn, "remotePort")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT);
        let token = match read_setting(&conn, "remoteToken") {
            Some(token) if token.len() >= 32 => token,
            _ => {
                let token = new_token();
                let _ = write_setting(&conn, "remoteToken", &token);
                token
            }
        };
        (enabled, port, token)
    };

    if !enabled {
        stop();
        let conn = state.db();
        return Ok(status(&conn));
    }
    match start(app, port, token) {
        Ok(status) => {
            state.log("INFO", &format!("远程访问网关已启动：{}", status.address));
            Ok(status)
        }
        Err(error) => {
            state.log("ERROR", &format!("远程访问网关启动失败：{error}"));
            Err(error)
        }
    }
}

fn status_from_app(app: &AppHandle) -> Result<RemoteStatus, String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "应用尚未初始化完成".to_string())?;
    let conn = state.db();
    Ok(status(&conn))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_codes_are_six_digits() {
        for _ in 0..50 {
            let code = six_digit_code();
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn token_comparison_rejects_mismatches() {
        assert!(tokens_match("abc123", "abc123"));
        assert!(!tokens_match("abc123", "abc124"));
        assert!(!tokens_match("abc123", "abc12"));
        assert!(!tokens_match("", ""));
    }

    #[test]
    fn bearer_header_wins_over_query() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer token-a".parse().unwrap());
        let uri: Uri = "/api/events?token=token-b".parse().unwrap();
        assert_eq!(presented_token(&headers, &uri).as_deref(), Some("token-a"));
    }

    #[test]
    fn query_token_is_accepted_for_streams() {
        let headers = HeaderMap::new();
        let uri: Uri = "/api/events?token=token-b".parse().unwrap();
        assert_eq!(presented_token(&headers, &uri).as_deref(), Some("token-b"));
    }

    #[test]
    fn asset_mime_types_are_explicit() {
        assert_eq!(mime_for("assets/index-abc.js"), "text/javascript; charset=utf-8");
        assert_eq!(mime_for("index.html"), "text/html; charset=utf-8");
        assert_eq!(mime_for("manifest.webmanifest"), "application/manifest+json");
    }
}
