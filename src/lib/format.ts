import type { Provider, TaskStatus } from "./types";

export function formatTime(value?: string | null): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatDateTime(value?: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function relativeTime(value?: string | null): string {
  if (!value) return "";
  const date = new Date(value);
  const diff = Date.now() - date.getTime();
  if (Number.isNaN(diff)) return "";
  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days} 天前`;
  return date.toLocaleDateString("zh-CN", { month: "2-digit", day: "2-digit" });
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function basename(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export function dirname(path: string): string {
  const parts = path.split(/[\\/]/);
  parts.pop();
  return parts.join("\\");
}

export const TASK_STATUS_LABEL: Record<TaskStatus, string> = {
  queued: "排队中",
  running: "执行中",
  waitingApproval: "等待审批",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
  interrupted: "已中断",
};

export function taskStatusClass(status: string): string {
  switch (status) {
    case "running":
      return "warn";
    case "waitingApproval":
      return "info";
    case "completed":
      return "ok";
    case "failed":
      return "err";
    default:
      return "";
  }
}

export function providerProtocolLabel(protocol: string): string {
  switch (protocol) {
    case "openai_chat":
      return "OpenAI Chat Completions";
    case "openai_responses":
      return "OpenAI Responses";
    case "anthropic":
      return "Anthropic Messages";
    case "gemini":
      return "Google Gemini";
    case "ollama":
      return "Ollama";
    case "openai_images":
      return "OpenAI Images";
    case "gemini_images":
      return "Gemini Images";
    default:
      return protocol;
  }
}

export function capabilityLabel(key: string): string {
  switch (key) {
    case "chat":
      return "对话";
    case "stream":
      return "流式";
    case "tools":
      return "工具调用";
    case "vision":
      return "视觉";
    case "imageGen":
      return "文本生图";
    case "imageEdit":
      return "图像编辑";
    case "json":
      return "JSON 输出";
    case "longContext":
      return "长上下文";
    case "multiTool":
      return "多轮工具";
    case "reasoning":
      return "思考强度";
    default:
      return key;
  }
}

export function capabilityStateLabel(state: string): string {
  switch (state) {
    case "verified":
      return "已验证";
    case "unverified":
      return "未验证";
    case "unsupported":
      return "不支持";
    case "failed":
      return "验证失败";
    case "unavailable":
      return "服务不可用";
    default:
      return state;
  }
}

export function defaultModelFor(provider: Provider): string {
  if (provider.category === "image") {
    return (
      provider.defaultImageModel ??
      provider.models[0]?.id ??
      ""
    );
  }
  return (
    provider.defaultChatModel ??
    provider.defaultVisionModel ??
    provider.models[0]?.id ??
    ""
  );
}

export function toolTarget(name: string, args: Record<string, unknown>): string {
  if (name === "web_search") {
    const query = String(args.query ?? "");
    return query.length > 70 ? `${query.slice(0, 70)}…` : query;
  }
  if (name === "fetch_url") {
    const url = String(args.url ?? "");
    return url.length > 70 ? `${url.slice(0, 70)}…` : url;
  }
  if (name === "spawn_subagent") {
    const role = String(args.role ?? "researcher");
    const task = String(args.task ?? "");
    return `${role} · ${task.length > 56 ? `${task.slice(0, 56)}…` : task}`;
  }
  const value =
    (args.path as string | undefined) ??
    (args.command as string | undefined) ??
    (args.query as string | undefined) ??
    "";
  if (!value) {
    if (name === "git_status") return "工作区";
    if (name === "update_plan") return "计划";
    return "";
  }
  if (name === "run_command" || name === "run_tests") {
    return value.length > 70 ? `${value.slice(0, 70)}…` : value;
  }
  return value;
}

export function summarizeArgs(args: Record<string, unknown>, max = 160): string {
  try {
    const text = JSON.stringify(args);
    return text.length > max ? `${text.slice(0, max)}…` : text;
  } catch {
    return "";
  }
}

export function estimateTokens(text: string): number {
  if (!text) return 0;
  let cjk = 0;
  for (const ch of text) {
    if (/[\u3040-\u30ff\u3400-\u9fff]/.test(ch)) cjk += 1;
  }
  const other = text.length - cjk;
  return Math.ceil(cjk * 0.9 + other / 4);
}
