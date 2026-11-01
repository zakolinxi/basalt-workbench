import { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  Check,
  FolderOpen,
  Image as ImageIcon,
  MessageSquarePlus,
  Plus,
  Server,
  Sparkles,
  Square,
  X,
} from "lucide-react";
import { useAppStore, currentProject, currentSession } from "../lib/store";
import { api } from "../lib/api";
import type { RunMetrics, TaskEvent } from "../lib/types";
import { Button } from "./ui";
import { MessageItem, StreamingItem } from "./MessageItem";
import { TaskActivity, isTaskActivityEvent } from "./TaskActivity";
import { Composer } from "./Composer";

function formatRunTime(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "";
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(1).replace(/\.0$/, "")}s`;
}

function formatRunRate(tokens: number, durationMs: number): string {
  if (!Number.isFinite(tokens) || !Number.isFinite(durationMs) || durationMs <= 0) {
    return "";
  }
  return `${(tokens / (durationMs / 1000)).toFixed(1).replace(/\.0$/, "")}`;
}

function formatApprovalArgs(name: string, args: Record<string, unknown>): string {
  if (name === "write_file") {
    const content = String(args.content ?? "");
    return `PATH ${String(args.path ?? "")}\n\n${
      content.length > 500 ? `${content.slice(0, 500)}…` : content
    }`;
  }
  if (name === "apply_patch") {
    const shorten = (value: unknown) => {
      const text = String(value ?? "");
      return text.length > 180 ? `${text.slice(0, 180)}…` : text;
    };
    return `PATH ${String(args.path ?? "")}\n\n- ${shorten(args.old_string)}\n+ ${shorten(
      args.new_string,
    )}`;
  }
  if (name === "run_command" || name === "run_tests") {
    return `$ ${String(args.command ?? "")}\nCWD ${String(args.cwd ?? ".")}`;
  }
  const text = JSON.stringify(args, null, 2);
  return text.length > 500 ? `${text.slice(0, 500)}…` : text;
}

export function ChatView() {
  const project = useAppStore(currentProject);
  const session = useAppStore(currentSession);
  const messages = useAppStore((state) => state.messages);
  const streams = useAppStore((state) => state.streams);
  const activeTaskId = useAppStore((state) => state.activeTaskId);
  const taskDetail = useAppStore((state) =>
    state.activeTaskId ? state.taskDetails[state.activeTaskId] : undefined,
  );
  const activeChat = useAppStore((state) => state.activeChat);
  const pendingApprovals = useAppStore((state) => state.pendingApprovals);
  const providers = useAppStore((state) => state.providers);
  const newSession = useAppStore((state) => state.newSession);
  const setView = useAppStore((state) => state.setView);
  const retryChat = useAppStore((state) => state.retryChat);
  const resolveApproval = useAppStore((state) => state.resolveApproval);
  const stopTask = useAppStore((state) => state.stopTask);
  const resumeTask = useAppStore((state) => state.resumeTask);
  const tasks = useAppStore((state) => state.tasks);
  const task = tasks.find((item) => item.id === activeTaskId) ?? null;
  const scrollRef = useRef<HTMLDivElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const [runMetrics, setRunMetrics] = useState<RunMetrics | null>(null);

  const currentStreams = useMemo(
    () =>
      Object.values(streams).filter(
        (stream) => stream.sessionId === session?.id,
      ),
    [session?.id, streams],
  );
  const streamLength = currentStreams.reduce((total, stream) => total + stream.text.length, 0);
  const eventCount = taskDetail?.events.length ?? 0;
  const chatProviders = providers.filter(
    (provider) => provider.category === "chat" && provider.enabled && !provider.isTemplate,
  );
  const lastMessage = messages[messages.length - 1];
  const canRetry =
    Boolean(lastMessage) &&
    lastMessage.role === "assistant" &&
    !lastMessage.taskId &&
    !activeChat;
  const taskStatus = task?.status ?? null;
  const taskActive = Boolean(
    taskStatus && ["queued", "running", "waitingApproval"].includes(taskStatus),
  );
  const streamActive = currentStreams.length > 0 || Boolean(activeChat);
  const metricsTaskId =
    activeTaskId && lastMessage?.taskId === activeTaskId ? activeTaskId : null;
  const taskMessages = useMemo(
    () => (activeTaskId ? messages.filter((message) => message.taskId === activeTaskId) : []),
    [messages, activeTaskId],
  );
  const taskActivityEvents = useMemo(
    () => (taskDetail?.events ?? []).filter(isTaskActivityEvent),
    [taskDetail],
  );
  const activityByMessage = useMemo(() => {
    const map = new Map<string, TaskEvent[]>();
    if (!activeTaskId) return map;
    taskMessages.forEach((message, index) => {
      const next = taskMessages[index + 1];
      const slice = taskActivityEvents.filter(
        (event) =>
          event.createdAt >= message.createdAt &&
          (!next || event.createdAt <= next.createdAt),
      );
      if (slice.length > 0) map.set(message.id, slice);
    });
    return map;
  }, [activeTaskId, taskMessages, taskActivityEvents]);
  const trailingActivity =
    taskMessages.length === 0 ? taskActivityEvents : [];
  const metricsForTask = Boolean(
    task && metricsTaskId && metricsTaskId === task.id,
  );
  const canResume = Boolean(
    task &&
      metricsForTask &&
      ["interrupted", "failed", "cancelled"].includes(task.status),
  );
  const runNotice =
    task && metricsForTask && !taskActive
      ? task.status === "failed"
        ? task.error || "任务失败"
        : task.status === "interrupted"
          ? "任务已中断，进度与变更已保留。"
          : task.status === "cancelled"
            ? "任务已取消。"
            : null
      : null;
  const showRunStats = Boolean(
    runMetrics &&
      runMetrics.outputTokens > 0 &&
      runMetrics.durationMs > 0 &&
      !taskActive &&
      !streamActive,
  );
  const metricsProvider = providers.find(
    (provider) => provider.id === runMetrics?.providerId,
  );
  const metricsModel =
    metricsProvider?.models.find((model) => model.id === runMetrics?.model)?.name ??
    runMetrics?.model ??
    "";

  useEffect(() => {
    if (!session?.id) {
      setRunMetrics(null);
      return;
    }
    if (taskActive || streamActive) {
      setRunMetrics(null);
      return;
    }
    setRunMetrics(null);
    let cancelled = false;
    const timer = window.setTimeout(() => {
      api
        .runMetrics(session.id, metricsTaskId)
        .then((metrics) => {
          if (!cancelled) setRunMetrics(metrics);
        })
        .catch(() => {
          if (!cancelled) setRunMetrics(null);
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [
    session?.id,
    metricsTaskId,
    taskActive,
    streamActive,
    messages.length,
  ]);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element || !stick.current) return;
    const toBottom = () => {
      element.scrollTop = element.scrollHeight;
    };
    toBottom();
    const first = setTimeout(toBottom, 90);
    const second = setTimeout(toBottom, 320);
    return () => {
      clearTimeout(first);
      clearTimeout(second);
    };
  }, [messages.length, streamLength, eventCount, activeTaskId]);

  const onScroll = () => {
    const element = scrollRef.current;
    if (!element) return;
    stick.current =
      element.scrollHeight - element.scrollTop - element.clientHeight < 90;
  };

  const prompt = (text: string, mode: "chat" | "agent") => {
    window.dispatchEvent(
      new CustomEvent("basalt:set-composer", { detail: { text, mode } }),
    );
  };

  const hasContent = messages.length > 0 || currentStreams.length > 0 || activeTaskId;

  return (
    <div className="chat-col">
      <div className="chat-scroll" ref={scrollRef} onScroll={onScroll}>
        {hasContent ? (
          <div className="messages">
            {messages.map((message, index) => {
              const activity = activityByMessage.get(message.id);
              return (
                <MessageItem
                  key={message.id}
                  message={message}
                  canRetry={canRetry && index === messages.length - 1}
                  onRetry={() => void retryChat()}
                  footer={
                    activity ? <TaskActivity events={activity} /> : undefined
                  }
                />
              );
            })}
            {currentStreams.map((stream) => (
              <StreamingItem
                key={stream.messageId}
                text={stream.text}
                model={session?.model}
                task={Boolean(stream.taskId)}
              />
            ))}
            {trailingActivity.length > 0 ? (
              <div className="trailing-task-activity">
                <TaskActivity events={trailingActivity} />
              </div>
            ) : null}
            <div ref={endRef} />
          </div>
        ) : (
          <div className="chat-empty">
            <div className="chat-empty-inner">
              <div className="brand-line">
                <span className="brand-mark">
                  <span />
                  <span />
                  <span />
                </span>
                <div>
                  <h1>{session ? "开始一段新的工作" : project?.name ?? "Basalt"}</h1>
                  <div className="sub">BASALT · AI ENGINEERING WORKBENCH</div>
                </div>
              </div>
              <p>
                对话用于分析和讨论；切换到 Agent 后，模型可以在授权项目内读取文件、执行命令、
                修改代码，并对每一次写入和命令请求你的审批。
              </p>
              {chatProviders.length === 0 ? (
                <div className="warning-box" style={{ marginBottom: 18 }}>
                  尚未配置可用的对话供应商。请先到供应商页面配置真实 API。
                </div>
              ) : null}
              <div className="empty-actions">
                {session ? (
                  <Button onClick={() => void newSession()}>
                    <MessageSquarePlus size={13} /> 新会话
                  </Button>
                ) : (
                  <Button variant="primary" onClick={() => void newSession()}>
                    <Plus size={13} /> 新建会话
                  </Button>
                )}
                <Button onClick={() => setView("providers")}>
                  <Server size={13} /> 配置供应商
                </Button>
                {project ? (
                  <Button
                    onClick={() =>
                      prompt("请先概览项目结构，再为我总结关键模块与入口文件。", "agent")
                    }
                  >
                    <Sparkles size={13} /> 让 Agent 理解项目
                  </Button>
                ) : null}
              </div>
              <div className="empty-grid">
                <button
                  className="empty-cell"
                  style={{ textAlign: "left" }}
                  onClick={() =>
                    prompt("解释一下这个项目的架构和主要入口。", "chat")
                  }
                >
                  <MessageSquarePlus size={14} />
                  <span>
                    <span className="t">与模型讨论</span>
                    <span className="d">多轮对话、Markdown、流式输出与附件。</span>
                  </span>
                </button>
                <button
                  className="empty-cell"
                  style={{ textAlign: "left" }}
                  onClick={() =>
                    prompt("检查项目当前 Git 变更，指出潜在风险。", "agent")
                  }
                >
                  <Sparkles size={14} />
                  <span>
                    <span className="t">派发 Agent 任务</span>
                    <span className="d">计划、工具调用、真实 Diff 与测试结果。</span>
                  </span>
                </button>
                <button
                  className="empty-cell"
                  style={{ textAlign: "left" }}
                  onClick={() => prompt("/img 一张极简的黑白玄武岩纹理海报", "chat")}
                >
                  <ImageIcon size={14} />
                  <span>
                    <span className="t">对话内生图</span>
                    <span className="d">使用独立 Image Provider，不依赖聊天模型。</span>
                  </span>
                </button>
                <button
                  className="empty-cell"
                  style={{ textAlign: "left" }}
                  onClick={() =>
                    prompt("为项目补充一个可运行的测试并执行失败分析。", "agent")
                  }
                >
                  <FolderOpen size={14} />
                  <span>
                    <span className="t">端到端修改</span>
                    <span className="d">修改 → Diff → 终端 → 验证 → 审查。</span>
                  </span>
                </button>
              </div>
            </div>
          </div>
        )}
      </div>

      {taskActive && task ? (
        <div className="chat-runbar">
          <div className="run-status">
            <span className="spinner" style={{ width: 10, height: 10 }} />
            <span className="run-status-label">
              {task.status === "queued"
                ? "任务排队中"
                : task.status === "waitingApproval"
                  ? "等待审批"
                  : "Agent 正在执行"}
            </span>
            <span className="run-status-spacer" />
            <Button
              size="xs"
              variant="ghost"
              onClick={() => void stopTask(task.id)}
            >
              <Square size={9} /> 停止
            </Button>
          </div>
        </div>
      ) : null}
      {runNotice || (showRunStats && runMetrics) ? (
        <div className="chat-runbar">
          <div className="run-footer">
            {runNotice ? <div className="run-notice">{runNotice}</div> : null}
            {showRunStats && runMetrics ? (
              <div className="run-stats">
                <span className="run-stats-model">{metricsModel}</span>
                <span className="run-stats-sep">·</span>
                <span className="run-stats-metric">
                  {formatRunTime(runMetrics.durationMs)}
                </span>
                <span className="run-stats-sep">·</span>
                <span className="run-stats-metric">
                  {formatRunRate(runMetrics.outputTokens, runMetrics.durationMs)} tok/s
                </span>
                {canResume && task ? (
                  <Button
                    size="xs"
                    variant="ghost"
                    onClick={() => void resumeTask(task.id)}
                  >
                    继续
                  </Button>
                ) : null}
              </div>
            ) : null}
            {canResume && task && !(showRunStats && runMetrics) ? (
              <div className="run-actions">
                <Button
                  size="xs"
                  variant="ghost"
                  onClick={() => void resumeTask(task.id)}
                >
                  继续
                </Button>
              </div>
            ) : null}
          </div>
        </div>
      ) : null}

      <div className="composer-wrap">
        {pendingApprovals.length > 0 ? (
          <div className="approval-bar">
            <div className="approval-bar-head">
              <AlertTriangle size={12} />
              需要你的授权 · {pendingApprovals.length} 项操作暂停中
            </div>
            {pendingApprovals.slice(0, 3).map((approval) => {
              const task = tasks.find((item) => item.id === approval.taskId);
              return (
                <div className="approval-item" key={approval.callId}>
                  <div className="info">
                    <div className="tool">
                      {approval.name}
                      {task ? (
                        <span
                          className="muted"
                          style={{ fontFamily: "var(--font-ui)", fontSize: 10, marginLeft: 8 }}
                        >
                          {task.title}
                        </span>
                      ) : null}
                    </div>
                    <div className="reason">{approval.reason}</div>
                    <div className="args">
                      {formatApprovalArgs(approval.name, approval.args)}
                    </div>
                  </div>
                  <div className="row" style={{ gap: 5, flex: "none" }}>
                    <Button
                      size="xs"
                      variant="primary"
                      onClick={() => void resolveApproval(approval.callId, true)}
                    >
                      <Check size={10} /> 批准
                    </Button>
                    <Button
                      size="xs"
                      onClick={() => void resolveApproval(approval.callId, false)}
                    >
                      <X size={10} /> 拒绝
                    </Button>
                  </div>
                </div>
              );
            })}
          </div>
        ) : null}
        <Composer />
      </div>
    </div>
  );
}
