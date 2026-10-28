import { useEffect, useMemo, useRef } from "react";
import {
  AlertTriangle,
  Check,
  FolderOpen,
  Image as ImageIcon,
  MessageSquarePlus,
  Plus,
  Server,
  Sparkles,
  X,
} from "lucide-react";
import { useAppStore, currentProject, currentSession } from "../lib/store";
import { Button } from "./ui";
import { MessageItem, StreamingItem } from "./MessageItem";
import { TaskTimeline } from "./TaskTimeline";
import { Composer } from "./Composer";

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
  const tasks = useAppStore((state) => state.tasks);
  const scrollRef = useRef<HTMLDivElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const stick = useRef(true);

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
            {messages.map((message, index) => (
              <MessageItem
                key={message.id}
                message={message}
                canRetry={canRetry && index === messages.length - 1}
                onRetry={() => void retryChat()}
              />
            ))}
            {currentStreams.map((stream) => (
              <StreamingItem
                key={stream.messageId}
                text={stream.text}
                model={session?.model}
                task={Boolean(stream.taskId)}
              />
            ))}
            {activeTaskId ? <TaskTimeline taskId={activeTaskId} /> : null}
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
