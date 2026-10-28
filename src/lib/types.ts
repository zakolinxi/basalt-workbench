export type CapState =
  | "verified"
  | "unverified"
  | "unsupported"
  | "failed"
  | "unavailable";

export interface ModelCapabilities {
  chat: CapState;
  stream: CapState;
  tools: CapState;
  vision: CapState;
  imageGen: CapState;
  imageEdit: CapState;
  json: CapState;
  longContext: CapState;
  multiTool: CapState;
  reasoning: CapState;
}

export interface ProviderModel {
  id: string;
  name: string;
  contextLength?: number | null;
  capabilities: ModelCapabilities;
  source: string;
}

export type ProviderCategory = "chat" | "image";

export interface Provider {
  id: string;
  name: string;
  category: ProviderCategory;
  protocol: string;
  baseUrl: string;
  hasKey: boolean;
  models: ProviderModel[];
  defaultChatModel?: string | null;
  defaultVisionModel?: string | null;
  defaultImageModel?: string | null;
  headers: Record<string, unknown>;
  params: Record<string, unknown>;
  timeoutSecs: number;
  stream: boolean;
  enabled: boolean;
  isTemplate: boolean;
  lastStatus?: string | null;
  lastError?: string | null;
  lastTestedAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ProviderInput {
  id?: string;
  name: string;
  category: ProviderCategory;
  protocol: string;
  baseUrl: string;
  apiKey?: string | null;
  clearApiKey?: boolean;
  models: ProviderModel[];
  defaultChatModel?: string | null;
  defaultVisionModel?: string | null;
  defaultImageModel?: string | null;
  headers: Record<string, unknown>;
  params: Record<string, unknown>;
  timeoutSecs: number;
  stream: boolean;
  enabled: boolean;
}

export interface ProviderTestResult {
  ok: boolean;
  message: string;
  latencyMs: number;
  models?: ProviderModel[] | null;
}

export interface Project {
  id: string;
  name: string;
  path: string;
  createdAt: string;
  lastOpenedAt: string;
}

export interface Session {
  id: string;
  projectId?: string | null;
  title: string;
  providerId?: string | null;
  model?: string | null;
  mode: string;
  reasoningEffort?: ReasoningEffort | null;
  createdAt: string;
  updatedAt: string;
}

export type ReasoningEffort =
  | "off"
  | "minimal"
  | "low"
  | "medium"
  | "high"
  | "xhigh"
  | "max"
  | "ultra";

export interface Attachment {
  name: string;
  mime: string;
  size: number;
  kind: "image" | "file" | string;
  path: string;
}

export interface Message {
  id: string;
  sessionId: string;
  role: "user" | "assistant" | "system" | string;
  content: string;
  kind: "text" | "image" | "error" | string;
  attachments: Attachment[];
  taskId?: string | null;
  model?: string | null;
  createdAt: string;
}

export type TaskStatus =
  | "queued"
  | "running"
  | "waitingApproval"
  | "completed"
  | "failed"
  | "cancelled"
  | "interrupted";

export interface TaskStep {
  title: string;
  detail?: string | null;
  status: string;
}

export interface Task {
  id: string;
  projectId: string;
  sessionId: string;
  title: string;
  goal: string;
  status: TaskStatus;
  providerId: string;
  model: string;
  reasoningEffort?: ReasoningEffort | null;
  plan: TaskStep[];
  report?: string | null;
  error?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ToolCall {
  id: string;
  taskId: string;
  name: string;
  args: Record<string, unknown>;
  result?: unknown;
  status: string;
  requiresApproval: boolean;
  createdAt: string;
  completedAt?: string | null;
}

export interface TaskEvent {
  id: number;
  taskId: string;
  type: string;
  payload: Record<string, any>;
  createdAt: string;
}

export interface TaskEventEnvelope {
  taskId: string;
  type: string;
  payload: Record<string, any>;
  at: string;
}

export interface Change {
  id: string;
  taskId: string;
  projectId: string;
  path: string;
  oldExists: boolean;
  newExists: boolean;
  oldHash?: string | null;
  newHash?: string | null;
  status: "pending" | "accepted" | "reverted" | "conflict" | string;
  createdAt: string;
}

export interface TaskDetail {
  task: Task;
  events: TaskEvent[];
  toolCalls: ToolCall[];
  changes: Change[];
}

export interface FileDiff {
  path: string;
  oldText: string;
  newText: string;
  oldLabel: string;
  newLabel: string;
  binary: boolean;
  truncated: boolean;
  source: string;
}

export interface FileContent {
  path: string;
  content: string;
  size: number;
  isBinary: boolean;
  truncated: boolean;
  language: string;
  hash: string;
}

export interface DirEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
}

export interface GitEntry {
  path: string;
  status: string;
  staged: boolean;
  untracked: boolean;
}

export interface GitStatus {
  isRepo: boolean;
  branch?: string | null;
  entries: GitEntry[];
  error?: string | null;
}

export interface Route {
  providerId: string;
  model: string;
}

export interface Routing {
  chat: Route;
  agentPlan: Route;
  agentExecute: Route;
  review: Route;
  vision: Route;
  image: Route;
  imageEdit: Route;
}

export interface Settings {
  theme: "dark" | "light" | "system";
  routing: Routing;
  agentMaxIterations: number;
  autoApproveWrites: boolean;
  autoApproveCommands: boolean;
  terminalShell: "powershell" | "cmd" | "wsl" | string;
  editorFontSize: number;
  chatFontSize: number;
  showHiddenFiles: boolean;
  userAgent: string;
}

export interface Bootstrap {
  settings: Settings;
  providers: Provider[];
  projects: Project[];
  sessions: Session[];
  tasks: Task[];
  appVersion: string;
  dataDir: string;
}

export interface ChatEvent {
  requestId: string;
  sessionId: string;
  taskId?: string | null;
  kind: "delta" | "message" | "error" | "cancelled" | "userMessage" | string;
  text?: string | null;
  messageId?: string | null;
  message?: Message | null;
  error?: string | null;
}

export interface ImageFile {
  path: string;
  mime: string;
  name: string;
  mock: boolean;
  width?: number | null;
  height?: number | null;
}

export interface ImageGeneration {
  id: string;
  providerId: string;
  providerName: string;
  model: string;
  mode: "generate" | "edit" | string;
  prompt: string;
  params: Record<string, unknown>;
  files: ImageFile[];
  status: "running" | "completed" | "failed" | string;
  error?: string | null;
  sessionId?: string | null;
  createdAt: string;
}

export interface ImageCapabilities {
  generate: boolean;
  edit: boolean;
  referenceImages: boolean;
  mask: boolean;
  multiple: boolean;
  maxCount: number;
  sizes: string[];
  qualities: string[];
  transparentBackground: boolean;
  asyncTasks: boolean;
  notes: string[];
}

export interface ImageEvent {
  generationId: string;
  status: string;
  generation?: ImageGeneration | null;
  error?: string | null;
}

export interface TerminalEvent {
  terminalId: string;
  data: string;
}

export interface ApprovalRequest {
  taskId: string;
  callId: string;
  toolCallId: string;
  name: string;
  args: Record<string, unknown>;
  reason: string;
}

export type ThemeMode = "dark" | "light" | "system";

export type CornerStyle = "sharp" | "round";

export type ApprovalMode = "ask" | "auto" | "full";

export interface Toast {
  id: string;
  kind: "info" | "success" | "error" | "warning";
  title: string;
  message?: string;
}

export interface StreamingMessage {
  messageId: string;
  sessionId: string;
  taskId?: string | null;
  text: string;
}

export interface OpenFileState {
  projectId: string;
  path: string;
  content: string;
  savedContent: string;
  hash: string;
  language: string;
  isBinary: boolean;
  truncated: boolean;
  size: number;
}

export interface DiffState {
  path: string;
  oldText: string;
  newText: string;
  oldLabel: string;
  newLabel: string;
  binary: boolean;
  truncated: boolean;
  source: string;
  changeId?: string | null;
  changeStatus?: string | null;
  projectId?: string | null;
}

export interface TerminalTab {
  id: string;
  title: string;
  shell: string;
  alive: boolean;
}
