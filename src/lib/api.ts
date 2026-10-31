import { invoke } from "@tauri-apps/api/core";
import type {
  Attachment,
  Bootstrap,
  Change,
  FileContent,
  FileDiff,
  GitStatus,
  ImageCapabilities,
  ImageGeneration,
  Message,
  Project,
  Provider,
  ProviderInput,
  ProviderTestResult,
  Session,
  Settings,
  Task,
  TaskDetail,
} from "./types";

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  settingsGet: () => invoke<Settings>("settings_get"),
  settingsSet: (settings: Settings) => invoke<Settings>("settings_set", { settings }),
  appInfo: () =>
    invoke<{ version: string; dataDir: string; os: string; arch: string }>("app_info"),
  openPath: (path: string) => invoke<void>("open_path", { path }),
  resolveApproval: (callId: string, approved: boolean) =>
    invoke<boolean>("resolve_approval", { callId, approved }),

  providerList: () => invoke<Provider[]>("provider_list"),
  providerGet: (providerId: string) => invoke<Provider>("provider_get", { providerId }),
  providerCreate: (input: ProviderInput) => invoke<Provider>("provider_create", { input }),
  providerUpdate: (input: ProviderInput) => invoke<Provider>("provider_update", { input }),
  providerDelete: (providerId: string) => invoke<void>("provider_delete", { providerId }),
  providerDuplicate: (providerId: string) =>
    invoke<Provider>("provider_duplicate", { providerId }),
  providerTest: (providerId: string) =>
    invoke<ProviderTestResult>("provider_test", { providerId }),
  providerSetEnabled: (providerId: string, enabled: boolean) =>
    invoke<Provider>("provider_set_enabled", { providerId, enabled }),
  providerSetCapability: (
    providerId: string,
    modelId: string,
    capability: string,
    value: string,
  ) =>
    invoke<Provider>("provider_set_capability", {
      providerId,
      modelId,
      capability,
      value,
    }),
  providerModelsFromDiscovered: (providerId: string, models: Provider["models"]) =>
    invoke<Provider>("provider_models_from_discovered", { providerId, models }),

  projectList: () => invoke<Project[]>("project_list"),
  projectAdd: (path: string, name?: string) =>
    invoke<Project>("project_add", { input: { path, name } }),
  projectOpen: (projectId: string) => invoke<Project>("project_open", { projectId }),
  projectRemove: (projectId: string) => invoke<void>("project_remove", { projectId }),
  projectReadFile: (projectId: string, path: string) =>
    invoke<FileContent>("project_read_file", { projectId, path }),
  projectWriteFile: (
    projectId: string,
    path: string,
    content: string,
    expectedHash?: string,
  ) =>
    invoke<FileContent>("project_write_file", {
      input: { projectId, path, content, expectedHash },
    }),
  gitStatus: (projectId: string) => invoke<GitStatus>("git_status", { projectId }),
  gitBranches: (projectId: string) =>
    invoke<string[]>("git_branches", { projectId }),
  gitCheckoutBranch: (projectId: string, branch: string) =>
    invoke<GitStatus>("git_checkout_branch", { projectId, branch }),
  gitCreateBranch: (projectId: string, branch: string) =>
    invoke<GitStatus>("git_create_branch", { projectId, branch }),
  gitFileDiff: (projectId: string, path: string) =>
    invoke<FileDiff>("git_file_diff", { projectId, path }),

  sessionList: () => invoke<Session[]>("session_list"),
  sessionCreate: (input: {
    projectId?: string | null;
    title?: string;
    providerId?: string | null;
    model?: string | null;
    mode?: string;
  }) => invoke<Session>("session_create", { input }),
  sessionUpdate: (
    sessionId: string,
    fields: {
      title?: string;
      providerId?: string;
      model?: string;
      reasoningEffort?: string;
    },
  ) => invoke<Session>("session_update", { sessionId, ...fields }),
  sessionDelete: (sessionId: string) => invoke<void>("session_delete", { sessionId }),
  messageList: (sessionId: string) =>
    invoke<Message[]>("message_list", { sessionId }),

  taskCreate: (input: {
    projectId: string;
    sessionId: string;
    title?: string;
    goal: string;
    providerId: string;
    model: string;
    reasoningEffort?: string;
  }) => invoke<Task>("task_create", { input }),
  taskList: () => invoke<Task[]>("task_list"),
  taskGet: (taskId: string) => invoke<TaskDetail>("task_get", { taskId }),
  taskStart: (taskId: string) => invoke<void>("task_start", { taskId }),
  taskCancel: (taskId: string) => invoke<boolean>("task_cancel", { taskId }),
  taskDelete: (taskId: string) => invoke<void>("task_delete", { taskId }),
  reviewTask: (taskId: string) => invoke<string>("review_task", { taskId }),

  changeList: (filter: { taskId?: string; projectId?: string }) =>
    invoke<Change[]>("change_list", filter),
  changeDiff: (changeId: string) =>
    invoke<FileDiff>("change_diff_cmd", { changeId }),
  changeResolve: (changeId: string, action: "accept" | "revert", force = false) =>
    invoke<Change>("change_resolve", { input: { changeId, action, force } }),

  attachmentImport: (path: string) => invoke<Attachment>("attachment_import", { path }),
  attachmentImportBytes: (name: string, mime: string | null, dataBase64: string) =>
    invoke<Attachment>("attachment_import_bytes", { name, mime, dataBase64 }),
  attachmentDelete: (path: string) => invoke<void>("attachment_delete", { path }),
  readMedia: (path: string) => invoke<string>("read_media", { path }),

  chatSend: (input: {
    requestId: string;
    sessionId: string;
    providerId: string;
    model: string;
    text: string;
    attachments: Attachment[];
    reuseLastUser?: boolean;
    reasoningEffort?: string;
  }) => invoke<Message>("chat_send", { input }),
  chatCancel: (requestId: string) => invoke<boolean>("chat_cancel", { requestId }),

  terminalCreate: (input: {
    projectId?: string | null;
    shell?: string;
    cols?: number;
    rows?: number;
  }) => invoke<string>("terminal_create", input),
  terminalWrite: (terminalId: string, data: string) =>
    invoke<void>("terminal_write", { terminalId, data }),
  terminalResize: (terminalId: string, cols: number, rows: number) =>
    invoke<void>("terminal_resize", { terminalId, cols, rows }),
  terminalKill: (terminalId: string) =>
    invoke<void>("terminal_kill", { terminalId }),

  imageCapabilities: (providerId: string, model: string) =>
    invoke<ImageCapabilities>("image_capabilities", { providerId, model }),
  imageGenerate: (input: {
    providerId: string;
    model: string;
    prompt: string;
    mode: "generate" | "edit";
    size?: string | null;
    quality?: string | null;
    count?: number;
    background?: string | null;
    referencePaths?: string[];
    maskPath?: string | null;
    sessionId?: string | null;
  }) => invoke<ImageGeneration>("image_generate", { input }),
  imageHistory: (sessionId?: string, limit?: number) =>
    invoke<ImageGeneration[]>("image_history", {
      sessionId: sessionId ?? null,
      limit: limit ?? 100,
    }),
  imageDelete: (generationId: string) =>
    invoke<void>("image_delete", { generationId }),
  imageSaveAs: (sourcePath: string, targetPath: string) =>
    invoke<string>("image_save_as", { sourcePath, targetPath }),
  chatImageGenerate: (sessionId: string, prompt: string) =>
    invoke<{
      generation: ImageGeneration;
      userMessage: Message;
      assistantMessage: Message;
    }>("chat_image_generate", { sessionId, prompt }),
};
