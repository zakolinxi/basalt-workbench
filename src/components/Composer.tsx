import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import {
  AlertTriangle,
  ArrowUp,
  Brain,
  Check,
  ChevronDown,
  Eye,
  FileText,
  Folder,
  GitFork,
  ImagePlus,
  Paperclip,
  Settings2,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Square,
  X,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import clsx from "clsx";
import { api, errorMessage } from "../lib/api";
import { useAppStore, currentProject, currentSession } from "../lib/store";
import { formatBytes } from "../lib/format";import { APPROVAL_OPTIONS, approvalMode, approvalOption } from "../lib/permissions";
import type { Attachment, Provider, ReasoningEffort } from "../lib/types";
import { Badge, IconButton, MediaThumb, useClickOutside } from "./ui";

const REASONING_OPTIONS: {
  value: ReasoningEffort;
  label: string;
  description: string;
}[] = [
  { value: "off", label: "关闭", description: "尽可能关闭推理；不支持关闭的模型会忽略" },
  { value: "minimal", label: "极低", description: "最少思考，适合简单问答" },
  { value: "low", label: "低", description: "快速回答，减少思考开销" },
  { value: "medium", label: "中", description: "速度与推理深度平衡" },
  { value: "high", label: "高", description: "更多思考预算，适合复杂任务" },
  { value: "xhigh", label: "极高", description: "更高的推理预算" },
  { value: "max", label: "最大", description: "使用模型原生最高档" },
  { value: "ultra", label: "Ultra", description: "最高档；协议不支持时取该协议最高档" },
];

const isReasoningEffort = (value: unknown): value is ReasoningEffort =>
  REASONING_OPTIONS.some((option) => option.value === value);

const FILE_FILTERS = [
  {
    name: "图片与文本文件",
    extensions: [
      "png",
      "jpg",
      "jpeg",
      "webp",
      "gif",
      "txt",
      "md",
      "json",
      "js",
      "jsx",
      "ts",
      "tsx",
      "rs",
      "py",
      "go",
      "java",
      "c",
      "cpp",
      "h",
      "cs",
      "toml",
      "yaml",
      "yml",
      "xml",
      "html",
      "css",
      "sql",
      "sh",
      "ps1",
    ],
  },
];

function readFileBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result ?? "");
      resolve(result.split(",")[1] ?? "");
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

function compact(value: number): string {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(2).replace(/\.?0+$/, "")}M`;
  }
  if (value >= 1000) {
    return `${(value / 1000).toFixed(value >= 10000 ? 0 : 1).replace(/\.0$/, "")}k`;
  }
  return String(value);
}

export function Composer() {
  const session = useAppStore(currentSession);
  const project = useAppStore(currentProject);
  const projectGit = useAppStore((state) => state.projectGit);
  const providers = useAppStore((state) => state.providers);
  const settings = useAppStore((state) => state.settings);
  const activeChat = useAppStore((state) => state.activeChat);
  const imageChatBusy = useAppStore((state) => state.imageChatBusy);
  const sendMessage = useAppStore((state) => state.sendMessage);
  const cancelChat = useAppStore((state) => state.cancelChat);
  const updateSessionModel = useAppStore((state) => state.updateSessionModel);
  const updateSessionReasoning = useAppStore(
    (state) => state.updateSessionReasoning,
  );
  const setApprovalMode = useAppStore((state) => state.setApprovalMode);
  const setView = useAppStore((state) => state.setView);
  const notify = useAppStore((state) => state.notify);

  const chatProviders = useMemo(
    () =>
      providers.filter(
        (provider) =>
          provider.category === "chat" && provider.enabled && !provider.isTemplate,
      ),
    [providers],
  );
  const [mode, setMode] = useState<"chat" | "agent">("agent");
  const [providerId, setProviderId] = useState("");
  const [model, setModel] = useState("");
  const [text, setText] = useState("");
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [importing, setImporting] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [permissionOpen, setPermissionOpen] = useState(false);
  const [reasoning, setReasoning] = useState<ReasoningEffort>("medium");
  const [reasoningOpen, setReasoningOpen] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const modelPickerRef = useClickOutside<HTMLDivElement>(
    () => setModelPickerOpen(false),
    modelPickerOpen,
  );
  const permissionRef = useClickOutside<HTMLDivElement>(
    () => setPermissionOpen(false),
    permissionOpen,
  );
  const reasoningRef = useClickOutside<HTMLDivElement>(
    () => setReasoningOpen(false),
    reasoningOpen,
  );
  const provider = chatProviders.find((item) => item.id === providerId) ?? null;
  const modelInfo = provider?.models.find((item) => item.id === model.trim()) ?? null;
  const supportsReasoning =
    modelInfo?.capabilities.reasoning !== "unsupported";
  const busy = Boolean(activeChat) || imageChatBusy;
  const isImageCommand = mode === "chat" && /^\/img\s+/i.test(text.trim());
  const approval = approvalMode(settings);
  const approvalInfo = approvalOption(approval);
  const reasoningInfo =
    REASONING_OPTIONS.find((option) => option.value === reasoning) ??
    REASONING_OPTIONS[3];
  const stopVisible = Boolean(
    activeChat && (!session || activeChat.sessionId === session.id),
  );

  useEffect(() => {
    const preferred = session
      ? chatProviders.find((item) => item.id === session.providerId) ??
        chatProviders[0]
      : chatProviders.find((item) => item.id === settings.routing.chat.providerId) ??
        chatProviders[0];
    setProviderId(preferred?.id ?? "");
    const fromSession =
      preferred && session && preferred.id === session.providerId
        ? session.model
        : null;
    const fromRouting =
      preferred && preferred.id === settings.routing.chat.providerId
        ? settings.routing.chat.model
        : null;
    setModel(
      fromSession ||
        fromRouting ||
        preferred?.defaultChatModel ||
        preferred?.models[0]?.id ||
        "",
    );
  }, [
    chatProviders,
    session?.id,
    session?.providerId,
    session?.model,
    settings.routing.chat.providerId,
    settings.routing.chat.model,
  ]);

  useEffect(() => {
    setText("");
    setAttachments([]);
    setModelPickerOpen(false);
    setPermissionOpen(false);
    setReasoningOpen(false);
  }, [session?.id]);

  useEffect(() => {
    setReasoning(
      isReasoningEffort(session?.reasoningEffort)
        ? session.reasoningEffort
        : "medium",
    );
  }, [session?.id, session?.reasoningEffort]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setModelPickerOpen(false);
      setPermissionOpen(false);
      setReasoningOpen(false);
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);

  useEffect(() => {
    const resize = () => {
      const area = textareaRef.current;
      if (!area) return;
      area.style.height = "auto";
      area.style.height = `${Math.min(200, Math.max(38, area.scrollHeight))}px`;
    };
    resize();
  }, [text]);

  useEffect(() => {
    const focusHandler = (event: Event) => {
      const detail = (event as CustomEvent).detail ?? {};
      if (detail.mode === "chat" || detail.mode === "agent") {
        setMode(detail.mode);
        setModelPickerOpen(false);
        setPermissionOpen(false);
      }
      if (typeof detail.text === "string") setText(detail.text);
      requestAnimationFrame(() => textareaRef.current?.focus());
    };
    window.addEventListener("basalt:focus-composer", focusHandler);
    window.addEventListener("basalt:set-composer", focusHandler);
    return () => {
      window.removeEventListener("basalt:focus-composer", focusHandler);
      window.removeEventListener("basalt:set-composer", focusHandler);
    };
  }, []);

  const changeProvider = (id: string) => {
    setProviderId(id);
    const next = chatProviders.find((item) => item.id === id);
    const nextModel =
      next?.defaultChatModel ?? next?.models[0]?.id ?? "";
    setModel(nextModel);
    if (session && nextModel) void updateSessionModel(id, nextModel);
  };

  const chooseModel = (id: string) => {
    setModel(id);
    if (session && providerId) void updateSessionModel(providerId, id);
    setModelPickerOpen(false);
  };

  const chooseApproval = (next: (typeof APPROVAL_OPTIONS)[number]["value"]) => {
    setPermissionOpen(false);
    if (next !== approval) void setApprovalMode(next);
  };

  const chooseReasoning = (next: ReasoningEffort) => {
    setReasoning(next);
    setReasoningOpen(false);
    if (next !== reasoning) void updateSessionReasoning(next);
  };

  const switchMode = (next: "chat" | "agent") => {
    setMode(next);
    setModelPickerOpen(false);
    setPermissionOpen(false);
    setReasoningOpen(false);
  };

  const addAttachment = (attachment: Attachment) => {
    setAttachments((current) => {
      if (current.length >= 8) {
        notify("warning", "单条消息最多 8 个附件");
        return current;
      }
      return [...current, attachment];
    });
  };

  const importPaths = async (paths: string[]) => {
    setImporting(true);
    try {
      for (const path of paths.slice(0, 8)) {
        if (attachments.length >= 8) break;
        try {
          const attachment = await api.attachmentImport(path);
          addAttachment(attachment);
        } catch (error) {
          notify("error", "导入附件失败", errorMessage(error));
        }
      }
    } finally {
      setImporting(false);
    }
  };

  const pickAttachments = async () => {
    try {
      const selection = await open({
        multiple: true,
        title: "添加图片或文本附件",
        filters: FILE_FILTERS,
      });
      if (!selection) return;
      const paths = Array.isArray(selection) ? selection : [selection];
      await importPaths(paths);
    } catch (error) {
      notify("error", "无法打开文件选择器", errorMessage(error));
    }
  };

  const handleDrop = async (event: React.DragEvent) => {
    event.preventDefault();
    setDragging(false);
    if (mode === "agent") {
      notify("warning", "Agent 模式暂不支持附件");
      return;
    }
    const files = Array.from(event.dataTransfer.files).slice(0, 8);
    if (!files.length) return;
    setImporting(true);
    try {
      for (const file of files) {
        try {
          const base64 = await readFileBase64(file);
          const attachment = await api.attachmentImportBytes(
            file.name,
            file.type || null,
            base64,
          );
          addAttachment(attachment);
        } catch (error) {
          notify("error", `附件 ${file.name} 导入失败`, errorMessage(error));
        }
      }
    } finally {
      setImporting(false);
    }
  };

  const removeAttachment = async (attachment: Attachment) => {
    setAttachments((current) =>
      current.filter((item) => item.path !== attachment.path),
    );
    try {
      await api.attachmentDelete(attachment.path);
    } catch {
      /* already removed */
    }
  };

  const handleSend = () => {
    if (busy) return;
    if (!text.trim() && attachments.length === 0) return;
    if (!project) {
      notify("warning", "请先打开项目");
      return;
    }
    const imageCommand = mode === "chat" && /^\/img\s+/i.test(text.trim());
    if (!imageCommand && (!providerId || !model.trim())) {
      notify("warning", "请选择对话供应商与模型，或先在供应商页面完成配置");
      return;
    }
    if (mode === "agent" && modelInfo?.capabilities.tools === "unsupported") {
      notify("warning", "当前模型标记为不支持工具调用，无法运行 Agent");
      return;
    }
    if (mode === "agent" && attachments.length > 0) {
      notify("warning", "Agent 模式暂不支持直接附件，请让 Agent 读取项目文件");
      return;
    }
    const payloadText = text.trim();
    const payloadAttachments = attachments;
    setText("");
    setAttachments([]);
    void sendMessage({
      text: payloadText,
      attachments: payloadAttachments,
      mode,
      providerId,
      model: model.trim(),
      reasoningEffort: supportsReasoning ? reasoning : undefined,
    });
  };

  return (
    <div
      className={clsx("composer", dragging && "dragging")}
      onDragOver={(event) => {
        event.preventDefault();
        if (mode === "chat") setDragging(true);
      }}
      onDragEnter={(event) => {
        event.preventDefault();
        if (mode === "chat") setDragging(true);
      }}
      onDragLeave={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node)) {
          setDragging(false);
        }
      }}
      onDrop={(event) => void handleDrop(event)}
    >
      {attachments.length > 0 ? (
        <div className="composer-attachments">
          {attachments.map((attachment) =>
            attachment.kind === "image" ? (
              <div key={attachment.path} style={{ position: "relative" }}>
                <MediaThumb
                  path={attachment.path}
                  alt={attachment.name}
                  thumb
                  className="composer-thumb"
                />
                <button
                  className="icon-btn sm"
                  style={{
                    position: "absolute",
                    top: -6,
                    right: -6,
                    background: "var(--bg-3)",
                    border: "1px solid var(--line-strong)",
                  }}
                  onClick={() => void removeAttachment(attachment)}
                  title="移除附件"
                >
                  <X size={10} />
                </button>
              </div>
            ) : (
              <span className="attachment-chip" key={attachment.path} title={attachment.path}>
                <FileText size={11} />
                <span className="name">{attachment.name}</span>
                <span className="mono" style={{ fontSize: 9, color: "var(--fg-3)" }}>
                  {formatBytes(attachment.size)}
                </span>
                <button
                  onClick={() => void removeAttachment(attachment)}
                  style={{ display: "grid", placeItems: "center", color: "var(--fg-3)" }}
                  title="移除附件"
                >
                  <X size={10} />
                </button>
              </span>
            ),
          )}
        </div>
      ) : null}
      <textarea
        ref={textareaRef}
        rows={1}
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (
            event.key === "Enter" &&
            !event.shiftKey &&
            !event.nativeEvent.isComposing
          ) {
            event.preventDefault();
            handleSend();
          }
        }}
        placeholder={
          mode === "agent"
            ? "描述要完成的编程任务。Agent 会制定计划、请求审批并验证结果…"
            : "输入消息，或使用 /img 调用独立图像供应商生成图片…"
        }
        spellCheck={false}
      />
      <div className="composer-bar">
        {mode === "chat" ? (
          <IconButton
            label="添加图片或文本附件"
            onClick={() => void pickAttachments()}
            disabled={importing || busy}
          >
            {importing ? <span className="spinner" /> : <Paperclip size={15} />}
          </IconButton>
        ) : null}
        <div className="segmented">
          <button
            className={mode === "chat" ? "active" : ""}
            onClick={() => switchMode("chat")}
            title="普通对话：不会操作文件或执行命令"
          >
            对话
          </button>
          <button
            className={mode === "agent" ? "active" : ""}
            onClick={() => switchMode("agent")}
            title="Agent：在授权项目中执行编程任务"
          >
            Agent
          </button>
        </div>
        <div className="composer-model">
          <ModelPicker
            providers={chatProviders}
            provider={provider}
            model={model}
            open={modelPickerOpen}
            rootRef={modelPickerRef}
            onToggle={() => {
              setPermissionOpen(false);
              setReasoningOpen(false);
              setModelPickerOpen((value) => !value);
            }}
            onSelectProvider={changeProvider}
            onSelectModel={chooseModel}
            onManage={() => {
              setModelPickerOpen(false);
              setView("providers");
            }}
          />
        </div>
        {supportsReasoning ? (
        <div className="reasoning-picker" ref={reasoningRef}>
          <button
            type="button"
            className={clsx(
              "reasoning-trigger",
              reasoningOpen && "open",
              reasoning !== "off" && "on",
            )}
            onClick={() => {
              setModelPickerOpen(false);
              setPermissionOpen(false);
              setReasoningOpen((value) => !value);
            }}
            title={`思考强度：${reasoningInfo.label}（${reasoningInfo.description}）`}
          >
            <Brain size={12} />
            {reasoningInfo.label}
            <ChevronDown size={10} />
          </button>
          {reasoningOpen ? (
            <div className="menu permission-menu reasoning-menu">
              <div className="menu-label">思考强度</div>
              {REASONING_OPTIONS.map((option) => (
                <button
                  type="button"
                  key={option.value}
                  className={clsx(
                    "permission-option",
                    reasoning === option.value && "active",
                  )}
                  onClick={() => chooseReasoning(option.value)}
                >
                  <span className={clsx("perm-icon", option.value)}>
                    <Brain size={13} />
                  </span>
                  <span className="perm-copy">
                    <span className="t">{option.label}</span>
                    <span className="d">{option.description}</span>
                  </span>
                  {reasoning === option.value ? (
                    <Check size={11} className="perm-check" />
                  ) : null}
                </button>
              ))}
              <div className="permission-foot">
                仅对支持思考的模型生效 · 各协议自动映射
              </div>
            </div>
          ) : null}
        </div>
        ) : null}
        {mode === "agent" ? (
          <div className="permission-picker" ref={permissionRef}>
            <button
              type="button"
              className={clsx("permission-trigger", approval)}
              onClick={() => {
                setModelPickerOpen(false);
                setReasoningOpen(false);
                setPermissionOpen((value) => !value);
              }}
              title={`审批策略：${approvalInfo.label}（对后续 Agent 任务生效）`}
            >
              {approval === "ask" ? (
                <Shield size={12} />
              ) : approval === "auto" ? (
                <ShieldCheck size={12} />
              ) : (
                <ShieldAlert size={12} />
              )}
              {approvalInfo.label}
              <ChevronDown size={10} />
            </button>
            {permissionOpen ? (
              <div className="menu permission-menu">
                <div className="menu-label">Agent 审批策略</div>
                {APPROVAL_OPTIONS.map((option) => (
                  <button
                    type="button"
                    key={option.value}
                    className={clsx(
                      "permission-option",
                      approval === option.value && "active",
                    )}
                    onClick={() => chooseApproval(option.value)}
                  >
                    <span className={clsx("perm-icon", option.value)}>
                      {option.value === "ask" ? (
                        <Shield size={13} />
                      ) : option.value === "auto" ? (
                        <ShieldCheck size={13} />
                      ) : (
                        <ShieldAlert size={13} />
                      )}
                    </span>
                    <span className="perm-copy">
                      <span className="t">{option.label}</span>
                      <span className="d">{option.description}</span>
                    </span>
                    {approval === option.value ? (
                      <Check size={11} className="perm-check" />
                    ) : null}
                  </button>
                ))}
                <div className="permission-foot">新任务生效 · 危险命令始终由策略拦截</div>
              </div>
            ) : null}
          </div>
        ) : null}
        {modelInfo?.capabilities.tools === "unverified" ? (
          <Badge title="工具调用能力尚未经过真实测试">TOOLS?</Badge>
        ) : modelInfo?.capabilities.tools === "unsupported" ? (
          <Badge tone="err">NO TOOLS</Badge>
        ) : null}
        {mode === "agent" && !modelInfo ? (
          <Badge tone="warn">TOOLS 未验证</Badge>
        ) : null}
        <span className="spacer" />
        {isImageCommand ? (
          <span className="composer-hint">
            <ImagePlus size={11} /> IMAGE PROVIDER
          </span>
        ) : null}
        <button
          className={clsx("send-btn", stopVisible && "stop")}
          onClick={() => {
            if (stopVisible) void cancelChat();
            else handleSend();
          }}
          disabled={
            stopVisible
              ? false
              : busy ||
                (!text.trim() && attachments.length === 0) ||
                (!isImageCommand && (!providerId || !model.trim())) ||
                (mode === "agent" && modelInfo?.capabilities.tools === "unsupported")
          }
          title={stopVisible ? "停止生成" : "发送"}
          aria-label={stopVisible ? "停止生成" : "发送"}
        >
          {imageChatBusy && !stopVisible ? (
            <span className="spinner" style={{ borderTopColor: "var(--accent-fg)" }} />
          ) : stopVisible ? (
            <Square size={13} fill="currentColor" />
          ) : (
            <ArrowUp size={16} />
          )}
        </button>
      </div>
      {project ? (
        <div className="composer-meta">
          <div className="composer-workspace">
            <span className="composer-workspace-item" title={project.path}>
              <Folder size={12} />
              <span className="name">{project.name || project.path}</span>
            </span>
            {projectGit?.isRepo && projectGit.branch ? (
              <span
                className="composer-workspace-item branch"
                title={`Git 分支：${projectGit.branch}`}
              >
                <GitFork size={12} />
                <span className="name">{projectGit.branch}</span>
              </span>
            ) : null}
          </div>
          <div className="composer-alerts">
            {chatProviders.length === 0 && !isImageCommand ? (
              <button
                className="composer-alert"
                onClick={() => setView("providers")}
              >
                <AlertTriangle size={10} /> 未配置对话供应商
              </button>
            ) : null}
            {provider && !provider.hasKey && provider.protocol !== "ollama" ? (
              <span className="composer-alert warn">未设置 API Key</span>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function ModelPicker({
  providers,
  provider,
  model,
  open,
  rootRef,
  onToggle,
  onSelectProvider,
  onSelectModel,
  onManage,
}: {
  providers: Provider[];
  provider: Provider | null;
  model: string;
  open: boolean;
  rootRef: RefObject<HTMLDivElement | null>;
  onToggle: () => void;
  onSelectProvider: (id: string) => void;
  onSelectModel: (id: string) => void;
  onManage: () => void;
}) {
  const selected = provider?.models.find((item) => item.id === model) ?? null;
  const custom = Boolean(model) && Boolean(provider) && !selected;
  const displayModel = selected?.name ?? (model || "选择模型");

  return (
    <div className="model-picker" ref={rootRef}>
      <button
        type="button"
        className={clsx("model-trigger", open && "open")}
        onClick={onToggle}
        title={
          provider
            ? `${provider.name} · ${selected?.name ?? model ?? "未选择模型"}`
            : "选择对话供应商与模型"
        }
      >
        <span className="provider-name">{provider?.name ?? "选择供应商"}</span>
        <span className="model-slash">/</span>
        <span className="model-name">{displayModel}</span>
        <ChevronDown size={12} />
      </button>
      {open ? (
        <div className="model-picker-pop">
          <div className="mp-head">
            <span className="t">PROVIDER / MODEL</span>
            <button type="button" className="mp-manage" onClick={onManage}>
              <Settings2 size={10} /> 管理供应商
            </button>
          </div>
          <div className="mp-body">
            <div className="mp-providers">
              {providers.length === 0 ? (
                <div className="mp-empty">还没有可用的对话供应商</div>
              ) : null}
              {providers.map((item) => (
                <button
                  type="button"
                  key={item.id}
                  className={clsx(
                    "mp-provider",
                    item.id === provider?.id && "active",
                  )}
                  onClick={() => onSelectProvider(item.id)}
                  title={item.baseUrl}
                >
                  <span className="mp-provider-name">{item.name}</span>
                  <span className="mp-provider-meta">
                    {item.protocol}
                    {!item.hasKey && item.protocol !== "ollama"
                      ? " · 未设 KEY"
                      : ""}
                  </span>
                  {item.id === provider?.id ? (
                    <Check size={11} className="check" />
                  ) : null}
                </button>
              ))}
            </div>
            <div className="mp-models">
              <div className="mp-subhead">
                <span>模型</span>
                <span>{provider?.models.length ?? 0}</span>
              </div>
              {custom ? (
                <div className="mp-model active">
                  <span className="mp-model-main">
                    <span className="mp-model-name">{model}</span>
                    <span className="mp-model-id">自定义模型 ID</span>
                  </span>
                  <Check size={11} className="check" />
                </div>
              ) : null}
              {provider?.models.length ? (
                provider.models.map((item) => (
                  <button
                    type="button"
                    key={item.id}
                    className={clsx("mp-model", item.id === model && "active")}
                    onClick={() => onSelectModel(item.id)}
                    title={item.id}
                  >
                    <span className="mp-model-main">
                      <span className="mp-model-name">{item.name}</span>
                      {item.id !== item.name ? (
                        <span className="mp-model-id">{item.id}</span>
                      ) : null}
                    </span>
                    <span className="mp-model-meta">
                      <span className="mp-model-flags">
                        {item.capabilities.vision !== "unsupported" ? (
                          <Eye size={9} aria-label="支持图片输入" />
                        ) : null}
                        {item.capabilities.reasoning !== "unsupported" ? (
                          <Brain size={9} aria-label="支持思考强度" />
                        ) : null}
                      </span>
                      {item.contextLength ? `${compact(item.contextLength)} ctx` : ""}
                    </span>
                    {item.id === model ? (
                      <Check size={11} className="check" />
                    ) : null}
                  </button>
                ))
              ) : (
                <div className="mp-empty">
                  {provider
                    ? "该供应商还没有模型，请先到供应商页添加。"
                    : "先选择一个供应商。"}
                </div>
              )}
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
