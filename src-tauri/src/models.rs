use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAP_VERIFIED: &str = "verified";
pub const CAP_UNVERIFIED: &str = "unverified";
pub const CAP_UNSUPPORTED: &str = "unsupported";
pub const CAP_FAILED: &str = "failed";
pub const CAP_UNAVAILABLE: &str = "unavailable";

pub const TASK_QUEUED: &str = "queued";
pub const TASK_RUNNING: &str = "running";
pub const TASK_WAITING: &str = "waitingApproval";
pub const TASK_COMPLETED: &str = "completed";
pub const TASK_FAILED: &str = "failed";
pub const TASK_CANCELLED: &str = "cancelled";
pub const TASK_INTERRUPTED: &str = "interrupted";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    pub chat: String,
    pub stream: String,
    pub tools: String,
    pub vision: String,
    pub image_gen: String,
    pub image_edit: String,
    pub json: String,
    pub long_context: String,
    pub multi_tool: String,
    #[serde(default = "default_cap_state")]
    pub reasoning: String,
}

fn default_cap_state() -> String {
    CAP_UNVERIFIED.into()
}

impl ModelCapabilities {
    pub fn chat_baseline(stream: bool) -> Self {
        Self {
            chat: CAP_VERIFIED.into(),
            stream: if stream { CAP_UNVERIFIED } else { CAP_UNSUPPORTED }.into(),
            tools: CAP_UNVERIFIED.into(),
            vision: CAP_UNVERIFIED.into(),
            image_gen: CAP_UNSUPPORTED.into(),
            image_edit: CAP_UNSUPPORTED.into(),
            json: CAP_UNVERIFIED.into(),
            long_context: CAP_UNVERIFIED.into(),
            multi_tool: CAP_UNVERIFIED.into(),
            reasoning: CAP_UNVERIFIED.into(),
        }
    }

    pub fn image_baseline(generate: bool, edit: bool) -> Self {
        Self {
            chat: CAP_UNSUPPORTED.into(),
            stream: CAP_UNSUPPORTED.into(),
            tools: CAP_UNSUPPORTED.into(),
            vision: CAP_UNVERIFIED.into(),
            image_gen: if generate { CAP_UNVERIFIED } else { CAP_UNSUPPORTED }.into(),
            image_edit: if edit { CAP_UNVERIFIED } else { CAP_UNSUPPORTED }.into(),
            json: CAP_UNSUPPORTED.into(),
            long_context: CAP_UNSUPPORTED.into(),
            multi_tool: CAP_UNSUPPORTED.into(),
            reasoning: CAP_UNSUPPORTED.into(),
        }
    }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub context_length: Option<u64>,
    pub capabilities: ModelCapabilities,
    #[serde(default = "default_model_source")]
    pub source: String,
}

fn default_model_source() -> String {
    "manual".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub category: String,
    pub protocol: String,
    pub base_url: String,
    pub has_key: bool,
    pub models: Vec<ProviderModel>,
    pub default_chat_model: Option<String>,
    pub default_vision_model: Option<String>,
    pub default_image_model: Option<String>,
    pub headers: Value,
    pub params: Value,
    pub timeout_secs: u64,
    pub stream: bool,
    pub enabled: bool,
    pub is_template: bool,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub last_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub category: String,
    pub protocol: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    #[serde(default)]
    pub models: Vec<ProviderModel>,
    #[serde(default)]
    pub default_chat_model: Option<String>,
    #[serde(default)]
    pub default_vision_model: Option<String>,
    #[serde(default)]
    pub default_image_model: Option<String>,
    #[serde(default)]
    pub headers: Value,
    #[serde(default)]
    pub params: Value,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_true")]
    pub stream: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_timeout() -> u64 {
    120
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: u64,
    #[serde(default)]
    pub models: Option<Vec<ProviderModel>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub context_length: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub last_opened_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub mode: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub kind: String,
    pub attachments: Vec<Attachment>,
    pub task_id: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStep {
    pub title: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default = "default_step_status")]
    pub status: String,
}

fn default_step_status() -> String {
    "pending".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub session_id: String,
    pub title: String,
    pub goal: String,
    pub status: String,
    pub provider_id: String,
    pub model: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    pub plan: Vec<TaskStep>,
    pub report: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub task_id: String,
    pub name: String,
    pub args: Value,
    pub result: Option<Value>,
    pub status: String,
    pub requires_approval: bool,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEvent {
    pub id: i64,
    pub task_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    pub task: Task,
    pub events: Vec<TaskEvent>,
    pub tool_calls: Vec<ToolCall>,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub path: String,
    pub old_exists: bool,
    pub new_exists: bool,
    pub old_hash: Option<String>,
    pub new_hash: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub old_text: String,
    pub new_text: String,
    pub old_label: String,
    pub new_label: String,
    pub binary: bool,
    pub truncated: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub size: u64,
    pub is_binary: bool,
    pub truncated: bool,
    pub language: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitEntry {
    pub path: String,
    pub status: String,
    pub staged: bool,
    pub untracked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub entries: Vec<GitEntry>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageFile {
    pub path: String,
    pub mime: String,
    pub name: String,
    pub mock: bool,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageCapabilities {
    pub generate: bool,
    pub edit: bool,
    pub reference_images: bool,
    pub mask: bool,
    pub multiple: bool,
    pub max_count: u32,
    pub sizes: Vec<String>,
    pub qualities: Vec<String>,
    pub transparent_background: bool,
    pub async_tasks: bool,
    #[serde(default)]
    pub notes: Vec<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGeneration {
    pub id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model: String,
    pub mode: String,
    pub prompt: String,
    pub params: Value,
    pub files: Vec<ImageFile>,
    pub status: String,
    pub error: Option<String>,
    pub session_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
}

impl Default for Route {
    fn default() -> Self {
        Self {
            provider_id: String::new(),
            model: String::new(),
        }
    }
}

impl Route {
    pub fn is_set(&self) -> bool {
        !self.provider_id.is_empty() && !self.model.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Routing {
    pub chat: Route,
    pub agent_plan: Route,
    pub agent_execute: Route,
    pub review: Route,
    pub vision: Route,
    pub image: Route,
    pub image_edit: Route,
}

impl Default for Routing {
    fn default() -> Self {
        Self {
            chat: Route::default(),
            agent_plan: Route::default(),
            agent_execute: Route::default(),
            review: Route::default(),
            vision: Route::default(),
            image: Route::default(),
            image_edit: Route::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: String,
    pub routing: Routing,
    pub agent_max_iterations: u32,
    pub auto_approve_writes: bool,
    pub auto_approve_commands: bool,
    pub terminal_shell: String,
    pub editor_font_size: u32,
    pub chat_font_size: u32,
    pub show_hidden_files: bool,
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
}

pub fn default_user_agent() -> String {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36 Basalt/0.1.2".into()
}

pub const REASONING_EFFORTS: [&str; 8] = [
    "off", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
];

pub fn normalize_reasoning_effort(value: Option<&str>) -> Option<String> {
    let value = value.unwrap_or("").trim().to_ascii_lowercase();
    if value.is_empty() || value == "auto" {
        return None;
    }
    REASONING_EFFORTS
        .contains(&value.as_str())
        .then_some(value)
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            routing: Routing::default(),
            agent_max_iterations: 20,
            auto_approve_writes: false,
            auto_approve_commands: false,
            terminal_shell: "powershell".into(),
            editor_font_size: 13,
            chat_font_size: 14,
            show_hidden_files: false,
            user_agent: default_user_agent(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub settings: Settings,
    pub providers: Vec<Provider>,
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub tasks: Vec<Task>,
    pub app_version: String,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    pub request_id: String,
    pub session_id: String,
    pub task_id: Option<String>,
    pub kind: String,
    pub text: Option<String>,
    pub message_id: Option<String>,
    pub message: Option<Message>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEventEnvelope {
    pub task_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Value,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEvent {
    pub generation_id: String,
    pub status: String,
    pub generation: Option<ImageGeneration>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEvent {
    pub terminal_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub task_id: String,
    pub call_id: String,
    pub tool_call_id: String,
    pub name: String,
    pub args: Value,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_reasoning_effort() {
        assert_eq!(
            normalize_reasoning_effort(Some("HIGH")),
            Some("high".into())
        );
        assert_eq!(
            normalize_reasoning_effort(Some("ultra")),
            Some("ultra".into())
        );
        assert_eq!(
            normalize_reasoning_effort(Some("xhigh")),
            Some("xhigh".into())
        );
        assert_eq!(normalize_reasoning_effort(Some("auto")), None);
        assert_eq!(normalize_reasoning_effort(Some("bogus")), None);
        assert_eq!(normalize_reasoning_effort(None), None);
    }
}
