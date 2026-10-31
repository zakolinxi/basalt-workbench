import { create } from "zustand";
import { api, errorMessage } from "./api";
import { applyCornerStyle, applyPalette, applyTheme, loadCornerStyle } from "./theme";
import { approvalPatch } from "./permissions";
import type {
  ApprovalMode,
  ApprovalRequest,
  Attachment,
  Change,
  ChatEvent,
  CornerStyle,
  DiffState,
  FileDiff,
  GitStatus,
  ImageEvent,
  ImageGeneration,
  Message,
  OpenFileState,
  Project,
  Provider,
  ReasoningEffort,
  Session,
  Settings,
  StreamingMessage,
  Task,
  TaskDetail,
  TaskEventEnvelope,
  TaskStatus,
  TerminalTab,
  ThemeMode,
  ThemePalette,
  Toast,
} from "./types";

export interface LayoutState {
  sidebarWidth: number;
  rightWidth: number;
  terminalHeight: number;
  sidebarCollapsed: boolean;
  rightCollapsed: boolean;
  terminalCollapsed: boolean;
}

type View = "workbench" | "providers" | "images" | "settings";

interface ActiveChat {
  requestId: string;
  sessionId: string;
}

interface AppStore {
  booted: boolean;
  bootError: string | null;
  view: View;
  appVersion: string;
  dataDir: string;
  settings: Settings;
  cornerStyle: CornerStyle;
  providers: Provider[];
  projects: Project[];
  sessions: Session[];
  tasks: Task[];
  currentProjectId: string | null;
  currentSessionId: string | null;
  messages: Message[];
  streams: Record<string, StreamingMessage>;
  activeTaskId: string | null;
  taskDetails: Record<string, TaskDetail>;
  pendingApprovals: ApprovalRequest[];
  activeChat: ActiveChat | null;
  imageChatBusy: boolean;
  projectGit: GitStatus | null;
  changes: Change[];
  openFile: OpenFileState | null;
  diff: DiffState | null;
  rightTab: "code" | "changes" | "diff";
  terminals: TerminalTab[];
  activeTerminalId: string | null;
  imageHistory: ImageGeneration[];
  imageProgress: Record<string, ImageGeneration>;
  toasts: Toast[];
  layout: LayoutState;

  boot: () => Promise<void>;
  setView: (view: View) => void;
  notify: (kind: Toast["kind"], title: string, message?: string) => void;
  dismissToast: (id: string) => void;
  setLayout: (patch: Partial<LayoutState>) => void;
  setTheme: (theme: ThemeMode) => Promise<void>;
  setPalette: (palette: ThemePalette) => Promise<void>;
  setCornerStyle: (style: CornerStyle) => void;
  setApprovalMode: (mode: ApprovalMode) => Promise<void>;
  updateSettings: (patch: Partial<Settings>) => void;
  saveSettings: () => Promise<void>;
  refreshProviders: () => Promise<void>;
  refreshProjects: () => Promise<void>;
  refreshSessions: () => Promise<void>;
  refreshTasks: () => Promise<void>;
  setCurrentProject: (projectId: string) => Promise<void>;
  addProject: (projectId: string) => Promise<void>;
  removeProject: (projectId: string) => Promise<void>;
  newSession: (projectId?: string | null) => Promise<Session | null>;
  selectSession: (sessionId: string) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  updateSessionModel: (providerId: string, model: string) => Promise<void>;
  updateSessionReasoning: (effort: ReasoningEffort) => Promise<void>;
  sendMessage: (input: {
    text: string;
    attachments: Attachment[];
    mode: "chat" | "agent";
    providerId: string;
    model: string;
    reasoningEffort?: ReasoningEffort;
  }) => Promise<void>;
  retryChat: () => Promise<void>;
  cancelChat: () => Promise<void>;
  selectTask: (taskId: string) => Promise<void>;
  resumeTask: (taskId: string) => Promise<void>;
  refreshTaskDetail: (taskId: string) => Promise<void>;
  stopTask: (taskId: string) => Promise<void>;
  deleteTask: (taskId: string) => Promise<void>;
  resolveApproval: (callId: string, approved: boolean) => Promise<void>;
  refreshChanges: () => Promise<void>;
  refreshGit: () => Promise<void>;
  openFileAt: (path: string) => Promise<void>;
  saveOpenFile: (content: string) => Promise<void>;
  closeOpenFile: () => void;
  openTaskChangeDiff: (change: Change) => Promise<void>;
  openGitDiff: (path: string) => Promise<void>;
  setDiff: (diff: DiffState | null) => void;
  setRightTab: (tab: "code" | "changes" | "diff") => void;
  setRightCollapsed: (collapsed: boolean) => void;
  resolveChange: (changeId: string, action: "accept" | "revert", force?: boolean) => Promise<void>;
  newTerminal: (shell?: string) => Promise<void>;
  closeTerminal: (terminalId: string) => Promise<void>;
  setActiveTerminal: (terminalId: string | null) => void;
  markTerminalExit: (terminalId: string) => void;
  refreshImageHistory: () => Promise<void>;
  onChatEvent: (event: ChatEvent) => void;
  onTaskEvent: (event: TaskEventEnvelope) => void;
  onTerminalExit: (terminalId: string) => void;
  onImageEvent: (event: ImageEvent) => void;
}

const LAYOUT_KEY = "basalt.layout.v1";
const PROJECT_KEY = "basalt.project.v1";

function loadLayout(): LayoutState {
  const fallback: LayoutState = {
    sidebarWidth: 264,
    rightWidth: 520,
    terminalHeight: 230,
    sidebarCollapsed: false,
    rightCollapsed: false,
    terminalCollapsed: true,
  };
  try {
    const raw = localStorage.getItem(LAYOUT_KEY);
    if (!raw) return fallback;
    return { ...fallback, ...(JSON.parse(raw) as Partial<LayoutState>) };
  } catch {
    return fallback;
  }
}

function persistLayout(layout: LayoutState) {
  localStorage.setItem(LAYOUT_KEY, JSON.stringify(layout));
}

const defaultSettings: Settings = {
  theme: "dark",
  palette: "basalt",
  routing: {
    chat: { providerId: "", model: "" },
    agentPlan: { providerId: "", model: "" },
    agentExecute: { providerId: "", model: "" },
    review: { providerId: "", model: "" },
    vision: { providerId: "", model: "" },
    image: { providerId: "", model: "" },
    imageEdit: { providerId: "", model: "" },
  },
  autoApproveWrites: false,
  autoApproveCommands: false,
  terminalShell: "powershell",
  editorFontSize: 13,
  chatFontSize: 14,
  showHiddenFiles: false,
  userAgent:
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36 Basalt/0.1.16",
};

function upsertMessage(messages: Message[], message: Message): Message[] {
  const index = messages.findIndex((item) => item.id === message.id);
  if (index >= 0) {
    const copy = messages.slice();
    copy[index] = message;
    return copy;
  }
  return [...messages, message];
}

function taskStatusForEvent(type: string): TaskStatus | null {
  switch (type) {
    case "TaskQueued":
      return "queued";
    case "TaskStarted":
      return "running";
    case "ApprovalRequired":
      return "waitingApproval";
    case "ApprovalResolved":
      return "running";
    case "TaskCompleted":
      return "completed";
    case "TaskFailed":
      return "failed";
    case "TaskCancelled":
      return "cancelled";
    case "TaskInterrupted":
      return "interrupted";
    default:
      return null;
  }
}

function deriveApprovals(detail: TaskDetail): ApprovalRequest[] {
  const required = new Map<string, ApprovalRequest>();
  for (const event of detail.events) {
    if (event.type === "ApprovalRequired") {
      const payload = event.payload ?? {};
      const callId = String(payload.callId ?? "");
      if (!callId) continue;
      required.set(callId, {
        taskId: detail.task.id,
        callId,
        toolCallId: String(payload.toolCallId ?? ""),
        name: String(payload.name ?? "tool"),
        args: (payload.args ?? {}) as Record<string, unknown>,
        reason: String(payload.reason ?? ""),
      });
    } else if (event.type === "ApprovalResolved") {
      const callId = String(event.payload?.callId ?? "");
      required.delete(callId);
    }
  }
  if (!["running", "waitingApproval"].includes(detail.task.status)) {
    return [];
  }
  return [...required.values()];
}

let changesTimer: ReturnType<typeof setTimeout> | null = null;
let bootStarted = false;
let pendingSessionCreation:
  | { key: string; promise: Promise<Session | null> }
  | null = null;

export const useAppStore = create<AppStore>((set, get) => ({
  booted: false,
  bootError: null,
  view: "workbench",
  appVersion: "",
  dataDir: "",
  settings: defaultSettings,
  cornerStyle: loadCornerStyle(),
  providers: [],
  projects: [],
  sessions: [],
  tasks: [],
  currentProjectId: null,
  currentSessionId: null,
  messages: [],
  streams: {},
  activeTaskId: null,
  taskDetails: {},
  pendingApprovals: [],
  activeChat: null,
  imageChatBusy: false,
  projectGit: null,
  changes: [],
  openFile: null,
  diff: null,
  rightTab: "code",
  terminals: [],
  activeTerminalId: null,
  imageHistory: [],
  imageProgress: {},
  toasts: [],
  layout: loadLayout(),

  notify: (kind, title, message) => {
    const id = crypto.randomUUID();
    set((state) => ({ toasts: [...state.toasts.slice(-4), { id, kind, title, message }] }));
    setTimeout(() => get().dismissToast(id), 6500);
  },

  dismissToast: (id) =>
    set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) })),

  setView: (view) => set({ view }),

  setLayout: (patch) => {
    const layout = { ...get().layout, ...patch };
    persistLayout(layout);
    set({ layout });
  },

  boot: async () => {
    if (get().booted || bootStarted) return;
    bootStarted = true;
    try {
      const data = await api.bootstrap();
      applyTheme(data.settings.theme);
      applyPalette(data.settings.palette);
      const lastProject = localStorage.getItem(PROJECT_KEY);
      const project =
        data.projects.find((item) => item.id === lastProject) ?? data.projects[0] ?? null;
      const projectSessions = data.sessions.filter(
        (session) => session.projectId === project?.id,
      );
      const session = projectSessions[0] ?? null;
      set({
        booted: true,
        appVersion: data.appVersion,
        dataDir: data.dataDir,
        settings: data.settings,
        providers: data.providers,
        projects: data.projects,
        sessions: data.sessions,
        tasks: data.tasks,
        currentProjectId: project?.id ?? null,
        currentSessionId: session?.id ?? null,
      });
      if (project) {
        localStorage.setItem(PROJECT_KEY, project.id);
        void api.projectOpen(project.id);
      }
      if (session) {
        await get().selectSession(session.id);
      } else if (project) {
        await Promise.all([get().refreshGit(), get().refreshChanges()]);
      }
    } catch (error) {
      set({ booted: true, bootError: errorMessage(error) });
    }
  },

  setTheme: async (theme) => {
    const settings = { ...get().settings, theme };
    applyTheme(theme);
    set({ settings });
    try {
      await api.settingsSet(settings);
    } catch (error) {
      get().notify("error", "主题设置保存失败", errorMessage(error));
    }
  },

  setPalette: async (palette) => {
    const previous = get().settings;
    if (previous.palette === palette) return;
    const settings = { ...previous, palette };
    applyPalette(palette);
    set({ settings });
    try {
      const saved = await api.settingsSet(settings);
      set({ settings: saved });
    } catch (error) {
      applyPalette(previous.palette);
      set({ settings: previous });
      get().notify("error", "配色保存失败", errorMessage(error));
    }
  },

  setCornerStyle: (cornerStyle) => {
    applyCornerStyle(cornerStyle);
    set({ cornerStyle });
  },

  setApprovalMode: async (mode) => {
    const previous = get().settings;
    const settings = { ...previous, ...approvalPatch(mode) };
    set({ settings });
    try {
      const saved = await api.settingsSet(settings);
      set({ settings: saved });
    } catch (error) {
      // Never leave the UI claiming a looser/tighter policy than the backend.
      set({ settings: previous });
      get().notify("error", "审批策略保存失败", errorMessage(error));
    }
  },

  updateSettings: (patch) => set({ settings: { ...get().settings, ...patch } }),

  saveSettings: async () => {
    try {
      const saved = await api.settingsSet(get().settings);
      set({ settings: saved });
      applyTheme(saved.theme);
      applyPalette(saved.palette);
      get().notify("success", "设置已保存");
    } catch (error) {
      get().notify("error", "设置保存失败", errorMessage(error));
      throw error;
    }
  },

  refreshProviders: async () => {
    try {
      set({ providers: await api.providerList() });
    } catch (error) {
      get().notify("error", "供应商列表加载失败", errorMessage(error));
    }
  },

  refreshProjects: async () => {
    try {
      set({ projects: await api.projectList() });
    } catch (error) {
      get().notify("error", "项目列表加载失败", errorMessage(error));
    }
  },

  refreshSessions: async () => {
    try {
      set({ sessions: await api.sessionList() });
    } catch {
      /* non-critical */
    }
  },

  refreshTasks: async () => {
    try {
      set({ tasks: await api.taskList() });
    } catch {
      /* non-critical */
    }
  },

  setCurrentProject: async (projectId) => {
    const project = get().projects.find((item) => item.id === projectId);
    if (!project) return;
    set({
      currentProjectId: projectId,
      currentSessionId: null,
      messages: [],
      streams: {},
      activeTaskId: null,
      changes: [],
      projectGit: null,
      openFile: null,
      diff: null,
    });
    localStorage.setItem(PROJECT_KEY, projectId);
    void api.projectOpen(projectId);
    await get().refreshGit();
    const session = get().sessions.find((item) => item.projectId === projectId);
    if (session) {
      await get().selectSession(session.id);
    } else {
      await get().refreshChanges();
    }
  },

  addProject: async (projectId) => {
    await get().refreshProjects();
    await get().setCurrentProject(projectId);
  },

  removeProject: async (projectId) => {
    try {
      await api.projectRemove(projectId);
      await get().refreshProjects();
      if (get().currentProjectId === projectId) {
        const next = get().projects[0];
        if (next) await get().setCurrentProject(next.id);
        else
          set({
            currentProjectId: null,
            currentSessionId: null,
            messages: [],
            projectGit: null,
            changes: [],
            openFile: null,
            diff: null,
          });
      }
      get().notify("success", "已从工作区移除项目（磁盘文件未删除）");
    } catch (error) {
      get().notify("error", "移除项目失败", errorMessage(error));
    }
  },

  newSession: async (projectId) => {
    const targetProject = projectId ?? get().currentProjectId;
    const key = targetProject ?? "";
    if (pendingSessionCreation && pendingSessionCreation.key === key) {
      return pendingSessionCreation.promise;
    }
    const promise = (async () => {
      const route = get().settings.routing.chat;
      try {
        const session = await api.sessionCreate({
          projectId: targetProject ?? null,
          providerId: route.providerId || null,
          model: route.model || null,
          mode: "agent",
        });
        set((state) => ({ sessions: [session, ...state.sessions] }));
        await get().selectSession(session.id);
        return session;
      } catch (error) {
        get().notify("error", "创建会话失败", errorMessage(error));
        return null;
      }
    })();
    pendingSessionCreation = { key, promise };
    try {
      return await promise;
    } finally {
      if (pendingSessionCreation?.promise === promise) {
        pendingSessionCreation = null;
      }
    }
  },

  selectSession: async (sessionId) => {
    const session = get().sessions.find((item) => item.id === sessionId);
    if (!session) return;
    const projectId = session.projectId ?? get().currentProjectId;
    const tasks = get().tasks.filter((task) => task.sessionId === sessionId);
    const latest = tasks[0] ?? null;
    set({
      currentSessionId: sessionId,
      currentProjectId: projectId,
      messages: [],
      streams: {},
      activeTaskId: latest?.id ?? null,
      diff: null,
    });
    if (projectId) localStorage.setItem(PROJECT_KEY, projectId);
    try {
      const messages = await api.messageList(sessionId);
      if (get().currentSessionId === sessionId) set({ messages });
    } catch (error) {
      get().notify("error", "会话消息加载失败", errorMessage(error));
    }
    if (projectId) void get().refreshGit();
    if (latest) {
      await Promise.all([get().refreshTaskDetail(latest.id), get().refreshChanges()]);
    } else {
      await get().refreshChanges();
    }
  },

  deleteSession: async (sessionId) => {
    try {
      await api.sessionDelete(sessionId);
      await get().refreshSessions();
      await get().refreshTasks();
      if (get().currentSessionId === sessionId) {
        const next = get().sessions.find(
          (session) => session.projectId === get().currentProjectId,
        );
        if (next) await get().selectSession(next.id);
        else
          set({
            currentSessionId: null,
            messages: [],
            activeTaskId: null,
            streams: {},
            pendingApprovals: [],
          });
      }
      get().notify("success", "会话已删除");
    } catch (error) {
      get().notify("error", "删除会话失败", errorMessage(error));
    }
  },

  updateSessionModel: async (providerId, model) => {
    const sessionId = get().currentSessionId;
    if (!sessionId) return;
    set((state) => ({
      sessions: state.sessions.map((session) =>
        session.id === sessionId
          ? { ...session, providerId, model, updatedAt: new Date().toISOString() }
          : session,
      ),
    }));
    try {
      await api.sessionUpdate(sessionId, { providerId, model });
    } catch (error) {
      get().notify("error", "模型设置保存失败", errorMessage(error));
    }
  },

  updateSessionReasoning: async (effort) => {
    const sessionId = get().currentSessionId;
    if (!sessionId) return;
    set((state) => ({
      sessions: state.sessions.map((session) =>
        session.id === sessionId
          ? {
              ...session,
              reasoningEffort: effort,
              updatedAt: new Date().toISOString(),
            }
          : session,
      ),
    }));
    try {
      await api.sessionUpdate(sessionId, { reasoningEffort: effort });
    } catch (error) {
      get().notify("error", "思考强度保存失败", errorMessage(error));
    }
  },

  sendMessage: async ({ text, attachments, mode, providerId, model, reasoningEffort }) => {
    const trimmed = text.trim();
    if (!trimmed && attachments.length === 0) return;
    if (!get().currentProjectId) {
      get().notify("warning", "请先打开一个项目");
      return;
    }
    let sessionId = get().currentSessionId;
    if (!sessionId) {
      const session = await get().newSession(get().currentProjectId);
      if (!session) return;
      sessionId = session.id;
    }
    const session = get().sessions.find((item) => item.id === sessionId);
    if (!session) return;
    if (reasoningEffort && session.reasoningEffort !== reasoningEffort) {
      void api.sessionUpdate(sessionId, { reasoningEffort });
    }

    if (mode === "chat" && /^\/img\s+/i.test(trimmed)) {
      const prompt = trimmed.replace(/^\/img\s+/i, "").trim();
      if (!prompt) {
        get().notify("warning", "请在 /img 后输入图像提示词");
        return;
      }
      set({ imageChatBusy: true });
      try {
        await api.chatImageGenerate(sessionId, prompt);
        const messages = await api.messageList(sessionId);
        if (get().currentSessionId === sessionId) set({ messages });
        await get().refreshSessions();
        get().notify("success", "图像生成完成");
      } catch (error) {
        get().notify("error", "图像生成失败", errorMessage(error));
      } finally {
        set({ imageChatBusy: false });
      }
      return;
    }

    const provider = get().providers.find(
      (item) =>
        item.id === providerId &&
        item.category === "chat" &&
        item.enabled &&
        !item.isTemplate,
    );
    if (!provider) {
      get().notify("warning", "请选择一个可用的对话供应商");
      return;
    }
    const selectedModel = model || provider.defaultChatModel || "";
    if (!selectedModel) {
      get().notify("warning", "请选择或填写模型 ID");
      return;
    }
    if (session.providerId !== providerId || session.model !== selectedModel) {
      void api.sessionUpdate(sessionId, { providerId, model: selectedModel });
      set((state) => ({
        sessions: state.sessions.map((item) =>
          item.id === sessionId
            ? { ...item, providerId, model: selectedModel }
            : item,
        ),
      }));
    }

    if (mode === "agent") {
      if (attachments.length > 0) {
        get().notify("warning", "Agent 模式暂不支持附件，请让 Agent 直接读取项目文件");
        return;
      }
      try {
        const task = await api.taskCreate({
          projectId: get().currentProjectId!,
          sessionId,
          goal: trimmed,
          providerId,
          model: selectedModel,
          reasoningEffort,
        });
        set((state) => ({
          tasks: [task, ...state.tasks.filter((item) => item.id !== task.id)],
          activeTaskId: task.id,
        }));
        const messages = await api.messageList(sessionId);
        if (get().currentSessionId === sessionId) set({ messages });
        await get().refreshSessions();
        await get().refreshTaskDetail(task.id);
        await api.taskStart(task.id);
        await get().refreshTasks();
      } catch (error) {
        get().notify("error", "任务启动失败", errorMessage(error));
        await get().refreshTasks();
      }
      return;
    }

    const requestId = crypto.randomUUID();
    set({ activeChat: { requestId, sessionId } });
    try {
      const message = await api.chatSend({
        requestId,
        sessionId,
        providerId,
        model: selectedModel,
        text: trimmed,
        attachments,
        reasoningEffort,
      });
      if (get().currentSessionId === sessionId) {
        set((state) => ({ messages: upsertMessage(state.messages, message) }));
      }
    } catch (error) {
      const message = errorMessage(error);
      const stillActive = get().activeChat?.requestId === requestId;
      if (stillActive && !/取消|cancel/i.test(message)) {
        get().notify("error", "请求失败", message);
      }
    } finally {
      if (get().activeChat?.requestId === requestId) set({ activeChat: null });
      await get().refreshSessions();
      try {
        const messages = await api.messageList(sessionId);
        if (get().currentSessionId === sessionId) set({ messages });
      } catch {
        /* ignore */
      }
    }
  },

  retryChat: async () => {
    const { currentSessionId, sessions, messages, settings, providers } = get();
    if (!currentSessionId) return;
    const session = sessions.find((item) => item.id === currentSessionId);
    if (!session) return;
    const lastUser = [...messages]
      .reverse()
      .find((message) => message.role === "user");
    if (!lastUser) {
      get().notify("warning", "没有可重试的用户消息");
      return;
    }
    const providerId =
      session.providerId || settings.routing.chat.providerId || providers.find((p) => p.category === "chat" && p.enabled && !p.isTemplate)?.id;
    const provider = providers.find((item) => item.id === providerId);
    const model = session.model || settings.routing.chat.model || provider?.defaultChatModel || "";
    if (!providerId || !provider || !model) {
      get().notify("warning", "当前会话没有可用的聊天模型");
      return;
    }
    const requestId = crypto.randomUUID();
    set({ activeChat: { requestId, sessionId: currentSessionId } });
    try {
      const message = await api.chatSend({
        requestId,
        sessionId: currentSessionId,
        providerId,
        model,
        text: lastUser.content,
        attachments: [],
        reuseLastUser: true,
        reasoningEffort: session.reasoningEffort ?? undefined,
      });
      if (get().currentSessionId === currentSessionId) {
        set((state) => ({ messages: upsertMessage(state.messages, message) }));
      }
    } catch (error) {
      const message = errorMessage(error);
      if (!/取消|cancel/i.test(message)) {
        get().notify("error", "重试失败", message);
      }
    } finally {
      if (get().activeChat?.requestId === requestId) set({ activeChat: null });
      try {
        const next = await api.messageList(currentSessionId);
        if (get().currentSessionId === currentSessionId) set({ messages: next });
      } catch {
        /* ignore */
      }
    }
  },

  cancelChat: async () => {
    const active = get().activeChat;
    if (!active) return;
    try {
      const cancelled = await api.chatCancel(active.requestId);
      if (cancelled) get().notify("info", "正在停止生成…");
    } catch (error) {
      get().notify("error", "停止失败", errorMessage(error));
    }
  },

  resumeTask: async (taskId) => {
    try {
      await api.taskStart(taskId);
      await get().refreshTasks();
      await get().refreshTaskDetail(taskId);
      get().notify("info", "任务已重新排队");
    } catch (error) {
      get().notify("error", "继续任务失败", errorMessage(error));
    }
  },

  selectTask: async (taskId) => {
    const task = get().tasks.find((item) => item.id === taskId);
    if (!task) return;
    if (get().currentSessionId !== task.sessionId) {
      await get().selectSession(task.sessionId);
    }
    set({ activeTaskId: taskId });
    await Promise.all([get().refreshTaskDetail(taskId), get().refreshChanges()]);
  },

  refreshTaskDetail: async (taskId) => {
    try {
      const detail = await api.taskGet(taskId);
      set((state) => {
        const otherApprovals = state.pendingApprovals.filter(
          (approval) => approval.taskId !== taskId,
        );
        return {
          taskDetails: { ...state.taskDetails, [taskId]: detail },
          tasks: upsertTask(state.tasks, detail.task),
          pendingApprovals: [...otherApprovals, ...deriveApprovals(detail)],
        };
      });
    } catch {
      /* task may have been deleted */
    }
  },

  stopTask: async (taskId) => {
    try {
      await api.taskCancel(taskId);
      get().notify("info", "正在停止任务…");
    } catch (error) {
      get().notify("error", "停止任务失败", errorMessage(error));
    }
  },

  deleteTask: async (taskId) => {
    try {
      await api.taskDelete(taskId);
      set((state) => ({
        tasks: state.tasks.filter((task) => task.id !== taskId),
        activeTaskId: state.activeTaskId === taskId ? null : state.activeTaskId,
      }));
      await get().refreshChanges();
      get().notify("success", "任务记录已删除");
    } catch (error) {
      get().notify("error", "删除任务失败", errorMessage(error));
    }
  },

  resolveApproval: async (callId, approved) => {
    try {
      await api.resolveApproval(callId, approved);
      set((state) => ({
        pendingApprovals: state.pendingApprovals.filter(
          (approval) => approval.callId !== callId,
        ),
      }));
    } catch (error) {
      get().notify("error", "审批处理失败", errorMessage(error));
    }
  },

  refreshChanges: async () => {
    const projectId = get().currentProjectId;
    const taskId = get().activeTaskId;
    try {
      if (taskId) {
        set({ changes: await api.changeList({ taskId }) });
      } else if (projectId) {
        set({ changes: await api.changeList({ projectId }) });
      } else {
        set({ changes: [] });
      }
    } catch {
      /* ignore */
    }
  },

  refreshGit: async () => {
    const projectId = get().currentProjectId;
    if (!projectId) {
      set({ projectGit: null });
      return;
    }
    try {
      set({ projectGit: await api.gitStatus(projectId) });
    } catch (error) {
      set({
        projectGit: {
          isRepo: false,
          entries: [],
          error: errorMessage(error),
        },
      });
    }
  },

  openFileAt: async (path) => {
    const projectId = get().currentProjectId;
    if (!projectId) return;
    try {
      const file = await api.projectReadFile(projectId, path);
      set({
        openFile: {
          projectId,
          path: file.path,
          content: file.content,
          savedContent: file.content,
          hash: file.hash,
          language: file.language,
          isBinary: file.isBinary,
          truncated: file.truncated,
          size: file.size,
        },
        rightTab: "code",
        layout: { ...get().layout, rightCollapsed: false },
      });
      persistLayout(get().layout);
    } catch (error) {
      get().notify("error", "无法打开文件", errorMessage(error));
    }
  },

  saveOpenFile: async (content) => {
    const openFile = get().openFile;
    if (!openFile) return;
    try {
      const saved = await api.projectWriteFile(
        openFile.projectId,
        openFile.path,
        content,
        openFile.hash,
      );
      set({
        openFile: {
          ...openFile,
          content: saved.content,
          savedContent: saved.content,
          hash: saved.hash,
          size: saved.size,
        },
      });
      get().notify("success", "文件已保存");
      void get().refreshGit();
    } catch (error) {
      get().notify("error", "保存失败", errorMessage(error));
      throw error;
    }
  },

  closeOpenFile: () => set({ openFile: null }),

  openTaskChangeDiff: async (change) => {
    try {
      const diff = await api.changeDiff(change.id);
      set({
        diff: {
          path: diff.path,
          oldText: diff.oldText,
          newText: diff.newText,
          oldLabel: diff.oldLabel,
          newLabel: diff.newLabel,
          binary: diff.binary,
          truncated: diff.truncated,
          source: diff.source,
          changeId: change.id,
          changeStatus: change.status,
          projectId: change.projectId,
        },
        rightTab: "diff",
        layout: { ...get().layout, rightCollapsed: false },
      });
      persistLayout(get().layout);
    } catch (error) {
      get().notify("error", "加载变更 Diff 失败", errorMessage(error));
    }
  },

  openGitDiff: async (path) => {
    const projectId = get().currentProjectId;
    if (!projectId) return;
    try {
      const diff = await api.gitFileDiff(projectId, path);
      set({
        diff: {
          path: diff.path,
          oldText: diff.oldText,
          newText: diff.newText,
          oldLabel: diff.oldLabel,
          newLabel: diff.newLabel,
          binary: diff.binary,
          truncated: diff.truncated,
          source: diff.source,
          changeId: null,
          changeStatus: null,
          projectId,
        },
        rightTab: "diff",
        layout: { ...get().layout, rightCollapsed: false },
      });
      persistLayout(get().layout);
    } catch (error) {
      get().notify("error", "加载 Git Diff 失败", errorMessage(error));
    }
  },

  setDiff: (diff) => set({ diff }),

  setRightTab: (tab) => set({ rightTab: tab }),

  setRightCollapsed: (collapsed) => {
    const layout = { ...get().layout, rightCollapsed: collapsed };
    persistLayout(layout);
    set({ layout });
  },

  resolveChange: async (changeId, action, force = false) => {
    try {
      await api.changeResolve(changeId, action, force);
      await Promise.all([get().refreshChanges(), get().refreshGit()]);
      if (get().activeTaskId) void get().refreshTaskDetail(get().activeTaskId!);
      if (get().diff?.changeId === changeId) {
        set((state) => ({
          diff: state.diff
            ? { ...state.diff, changeStatus: action === "accept" ? "accepted" : "reverted" }
            : null,
        }));
      }
      get().notify(
        "success",
        action === "accept" ? "已接受变更" : "已撤销变更",
      );
    } catch (error) {
      get().notify("error", action === "accept" ? "接受失败" : "撤销失败", errorMessage(error));
      throw error;
    }
  },

  newTerminal: async (shell) => {
    const projectId = get().currentProjectId;
    const targetShell = shell ?? get().settings.terminalShell;
    try {
      const id = await api.terminalCreate({
        projectId,
        shell: targetShell,
        cols: 100,
        rows: 28,
      });
      const name =
        targetShell === "cmd" ? "CMD" : targetShell === "wsl" ? "WSL" : "PowerShell";
      const count = get().terminals.filter((tab) => tab.shell === targetShell).length + 1;
      const tab: TerminalTab = {
        id,
        title: `${name} ${count}`,
        shell: targetShell,
        alive: true,
      };
      const layout = { ...get().layout, terminalCollapsed: false };
      persistLayout(layout);
      set((state) => ({
        terminals: [...state.terminals, tab],
        activeTerminalId: id,
        layout,
      }));
    } catch (error) {
      get().notify("error", "创建终端失败", errorMessage(error));
    }
  },

  closeTerminal: async (terminalId) => {
    try {
      await api.terminalKill(terminalId);
    } catch {
      /* terminal may already be closed */
    }
    set((state) => {
      const terminals = state.terminals.filter((tab) => tab.id !== terminalId);
      return {
        terminals,
        activeTerminalId:
          state.activeTerminalId === terminalId
            ? terminals[terminals.length - 1]?.id ?? null
            : state.activeTerminalId,
      };
    });
  },

  setActiveTerminal: (terminalId) => set({ activeTerminalId: terminalId }),

  markTerminalExit: (terminalId) =>
    set((state) => ({
      terminals: state.terminals.map((tab) =>
        tab.id === terminalId ? { ...tab, alive: false } : tab,
      ),
    })),

  refreshImageHistory: async () => {
    try {
      set({ imageHistory: await api.imageHistory(undefined, 100) });
    } catch {
      /* ignore */
    }
  },

  onChatEvent: (event) => {
    const state = get();
    if (event.kind === "delta" && event.messageId) {
      const text = event.text ?? "";
      if (!text) return;
      set((current) => {
        const existing = current.streams[event.messageId!];
        return {
          streams: {
            ...current.streams,
            [event.messageId!]: {
              messageId: event.messageId!,
              sessionId: event.sessionId,
              taskId: event.taskId ?? null,
              text: (existing?.text ?? "") + text,
            },
          },
        };
      });
    } else if (event.kind === "userMessage" || event.kind === "message") {
      if (event.messageId) {
        set((current) => {
          const { [event.messageId!]: _removed, ...rest } = current.streams;
          return { streams: rest };
        });
      }
      if (event.message && event.sessionId === state.currentSessionId) {
        set((current) => ({ messages: upsertMessage(current.messages, event.message!) }));
      }
      if (
        state.activeChat &&
        (event.kind === "message" || event.kind === "userMessage") &&
        event.requestId === state.activeChat.requestId &&
        event.kind === "message"
      ) {
        set({ activeChat: null });
      }
    } else if (event.kind === "error") {
      if (event.messageId) {
        set((current) => {
          const { [event.messageId!]: _removed, ...rest } = current.streams;
          return { streams: rest };
        });
      }
      if (state.activeChat && event.requestId === state.activeChat.requestId) {
        set({ activeChat: null });
      }
      if (event.error) {
        get().notify("error", "生成失败", event.error);
      }
    } else if (event.kind === "cancelled") {
      if (event.messageId) {
        set((current) => {
          const { [event.messageId!]: _removed, ...rest } = current.streams;
          return { streams: rest };
        });
      }
      if (state.activeChat && event.requestId === state.activeChat.requestId) {
        set({ activeChat: null });
      }
    }
  },

  onTaskEvent: (event) => {
    const status = taskStatusForEvent(event.type);
    const plan =
      event.type === "PlanUpdated"
        ? (event.payload?.steps as Task["plan"] | undefined)
        : undefined;
    set((state) => {
      const tasks = state.tasks.map((task) =>
        task.id === event.taskId
          ? {
              ...task,
              status: status ?? task.status,
              plan: plan ?? task.plan,
              updatedAt: event.at,
            }
          : task,
      );
      const detail = state.taskDetails[event.taskId];
      let taskDetails = state.taskDetails;
      if (detail) {
        const nextTask = {
          ...detail.task,
          status: status ?? detail.task.status,
          plan: plan ?? detail.task.plan,
          updatedAt: event.at,
        };
        const lastId = detail.events.length
          ? detail.events[detail.events.length - 1].id
          : 0;
        taskDetails = {
          ...state.taskDetails,
          [event.taskId]: {
            ...detail,
            task: nextTask,
            events: [
              ...detail.events,
              {
                id: lastId + 1,
                taskId: event.taskId,
                type: event.type,
                payload: event.payload ?? {},
                createdAt: event.at,
              },
            ],
          },
        };
      }
      return { tasks, taskDetails };
    });

    if (event.type === "ApprovalRequired") {
      const payload = event.payload ?? {};
      const callId = String(payload.callId ?? "");
      if (callId) {
        const approval: ApprovalRequest = {
          taskId: event.taskId,
          callId,
          toolCallId: String(payload.toolCallId ?? ""),
          name: String(payload.name ?? "tool"),
          args: (payload.args ?? {}) as Record<string, unknown>,
          reason: String(payload.reason ?? ""),
        };
        set((state) => ({
          pendingApprovals: [
            ...state.pendingApprovals.filter((item) => item.callId !== callId),
            approval,
          ],
        }));
      }
    } else if (event.type === "ApprovalResolved") {
      const callId = String(event.payload?.callId ?? "");
      set((state) => ({
        pendingApprovals: state.pendingApprovals.filter(
          (item) => item.callId !== callId,
        ),
      }));
    } else if (event.type === "FileChanged" || event.type === "DiffUpdated") {
      if (changesTimer) clearTimeout(changesTimer);
      changesTimer = setTimeout(() => {
        void get().refreshChanges();
        void get().refreshGit();
      }, 250);
    } else if (
      ["TaskCompleted", "TaskFailed", "TaskCancelled", "TaskInterrupted"].includes(
        event.type,
      )
    ) {
      set((state) => ({
        pendingApprovals: state.pendingApprovals.filter(
          (approval) => approval.taskId !== event.taskId,
        ),
      }));
      void get().refreshTasks();
      void get().refreshSessions();
      if (get().taskDetails[event.taskId]) {
        void get().refreshTaskDetail(event.taskId);
      }
      void get().refreshChanges();
      void get().refreshGit();
      if (get().currentSessionId) {
        const sessionId = get().currentSessionId!;
        void api.messageList(sessionId).then((messages) => {
          if (get().currentSessionId === sessionId) set({ messages });
        });
      }
      const task = get().tasks.find((item) => item.id === event.taskId);
      if (task) {
        if (event.type === "TaskCompleted") {
          get().notify("success", "任务完成", task.title);
        } else if (event.type === "TaskFailed") {
          get().notify(
            "error",
            "任务失败",
            String(event.payload?.error ?? task.error ?? ""),
          );
        } else if (event.type === "TaskCancelled") {
          get().notify("info", "任务已取消");
        }
      }
    }
  },

  onTerminalExit: (terminalId) => {
    get().markTerminalExit(terminalId);
  },

  onImageEvent: (event) => {
    if (!event.generation) return;
    const generation = event.generation;
    set((state) => ({
      imageProgress: { ...state.imageProgress, [generation.id]: generation },
      imageHistory: state.imageHistory.some((item) => item.id === generation.id)
        ? state.imageHistory.map((item) =>
            item.id === generation.id ? generation : item,
          )
        : generation.status === "completed"
          ? [generation, ...state.imageHistory].slice(0, 100)
          : state.imageHistory,
    }));
  },
}));

function upsertTask(tasks: Task[], task: Task): Task[] {
  const index = tasks.findIndex((item) => item.id === task.id);
  if (index >= 0) {
    const copy = tasks.slice();
    copy[index] = task;
    return copy;
  }
  return [task, ...tasks];
}

export function currentTask(state: AppStore): Task | null {
  return (
    state.tasks.find((task) => task.id === state.activeTaskId) ??
    state.tasks.find((task) => task.sessionId === state.currentSessionId) ??
    null
  );
}

export function currentSession(state: AppStore): Session | null {
  return state.sessions.find((session) => session.id === state.currentSessionId) ?? null;
}

export function currentProject(state: AppStore): Project | null {
  return state.projects.find((project) => project.id === state.currentProjectId) ?? null;
}

export function chatProviders(state: AppStore): Provider[] {
  return state.providers.filter(
    (provider) => provider.category === "chat" && provider.enabled && !provider.isTemplate,
  );
}

export function imageProviders(state: AppStore): Provider[] {
  return state.providers.filter(
    (provider) => provider.category === "image" && provider.enabled && !provider.isTemplate,
  );
}
