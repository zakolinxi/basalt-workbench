import { useEffect, useMemo, useState } from "react";
import {
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  Clock,
  FileCode,
  Loader2,
  Play,
  ShieldCheck,
  Square,
  TestTube2,
  Wrench,
  XCircle,
} from "lucide-react";
import clsx from "clsx";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../lib/store";
import {
  TASK_STATUS_LABEL,
  formatDateTime,
  toolLabel,
  toolTarget,
} from "../lib/format";
import type { TaskEvent, TaskStatus } from "../lib/types";
import { Badge, Button, Modal, TaskStatusBadge } from "./ui";


import { Markdown } from "./Markdown";

interface ToolView {
  callId: string;
  name: string;
  args: Record<string, unknown>;
  status: string;
  summary: string;
  output: string;
  durationMs?: number;
}

function buildToolViews(events: TaskEvent[]): ToolView[] {
  const map = new Map<string, ToolView>();
  let lastCommand: string | null = null;
  for (const event of events) {
    const payload = (event.payload ?? {}) as Record<string, any>;
    if (event.type === "ToolCallStarted") {
      const callId = String(payload.callId ?? "");
      if (!callId) continue;
      map.set(callId, {
        callId,
        name: String(payload.name ?? "tool"),
        args: (payload.args ?? {}) as Record<string, unknown>,
        status: "running",
        summary: "执行中",
        output: "",
      });
      if (["run_command", "run_tests"].includes(String(payload.name))) {
        lastCommand = callId;
      }
    } else if (event.type === "ToolCallCompleted") {
      const callId = String(payload.callId ?? "");
      const tool = map.get(callId);
      if (!tool) continue;
      tool.status = String(payload.status ?? "completed");
      tool.summary = String(payload.summary ?? "");
      const result = payload.result as Record<string, any> | undefined;
      if (result) {
        if (result.error) tool.output = String(result.error);
        else if (
          typeof result.data === "string" &&
          ["run_command", "run_tests"].includes(tool.name) &&
          !tool.output
        ) {
          tool.output = result.data;
        } else if (
          typeof result.data === "string" &&
          ["web_search", "fetch_url", "spawn_subagent"].includes(tool.name)
        ) {
          try {
            const parsed = JSON.parse(result.data);
            tool.output =
              tool.name === "spawn_subagent" && typeof parsed.report === "string"
                ? parsed.report
                : result.data;
          } catch {
            tool.output = result.data;
          }
        }
        if (result.durationMs) tool.durationMs = Number(result.durationMs);
      }
    } else if (event.type === "TerminalOutput") {
      if (lastCommand) {
        const tool = map.get(lastCommand);
        if (tool) tool.output += String(payload.data ?? "");
      }
    }
  }
  return [...map.values()];
}

function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "";
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)}s`;
}

function toolStatusLabel(status: string): string {
  switch (status) {
    case "running":
      return "执行中";
    case "pending":
      return "等待";
    case "failed":
      return "失败";
    case "denied":
      return "已拒绝";
    case "cancelled":
      return "已取消";
    case "completed":
      return "完成";
    default:
      return status;
  }
}

function ToolStatusIcon({ status }: { status: string }) {
  if (status === "running" || status === "pending") {
    return (
      <Loader2
        size={12}
        style={{ animation: "spin .8s linear infinite", color: "var(--warn)" }}
      />
    );
  }
  if (status === "completed") {
    return <CheckCircle2 size={12} className="text-ok" />;
  }
  if (status === "failed" || status === "denied") {
    return <XCircle size={12} className="text-err" />;
  }
  return <Circle size={10} style={{ color: "var(--fg-3)" }} />;
}

export function TaskTimeline({ taskId }: { taskId: string }) {
  const task = useAppStore((state) => state.tasks.find((item) => item.id === taskId));
  const detail = useAppStore((state) => state.taskDetails[taskId]);
  const stopTask = useAppStore((state) => state.stopTask);
  const resumeTask = useAppStore((state) => state.resumeTask);
  const openTaskChangeDiff = useAppStore((state) => state.openTaskChangeDiff);
  const notify = useAppStore((state) => state.notify);
  const [expanded, setExpanded] = useState(true);
  const [toolsOpen, setToolsOpen] = useState<boolean | null>(null);
  const [reviewOpen, setReviewOpen] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const [reviewText, setReviewText] = useState("");
  const [reviewError, setReviewError] = useState<string | null>(null);

  useEffect(() => {
    setExpanded(true);
    setToolsOpen(null);
  }, [taskId]);

  const events = detail?.events ?? [];
  const toolViews = useMemo(() => buildToolViews(events), [events]);
  const toolStats = useMemo(() => {
    const counts = new Map<string, number>();
    let failed = 0;
    let runningCount = 0;
    let cancelled = 0;
    let totalMs = 0;
    for (const tool of toolViews) {
      counts.set(tool.name, (counts.get(tool.name) ?? 0) + 1);
      if (tool.status === "running" || tool.status === "pending") runningCount += 1;
      if (tool.status === "failed" || tool.status === "denied") failed += 1;
      if (tool.status === "cancelled") cancelled += 1;
      totalMs += tool.durationMs ?? 0;
    }
    return {
      counts: [...counts.entries()].sort(
        (a, b) => b[1] - a[1] || a[0].localeCompare(b[0]),
      ),
      failed,
      running: runningCount,
      cancelled,
      totalMs,
    };
  }, [toolViews]);
  const tests = useMemo(
    () => events.filter((event) => event.type === "TestCompleted"),
    [events],
  );
  const changes = detail?.changes ?? [];
  const steps = task?.plan ?? [];
  const status = (task?.status ?? "queued") as TaskStatus;
  const running = status === "running" || status === "waitingApproval";
  const toolsExpanded = toolsOpen ?? running;
  const canResume = ["interrupted", "failed", "cancelled"].includes(status);
  const active = running || status === "queued";

  if (!task) return null;

  return (
    <section className="timeline">
      <div className="timeline-head" onClick={() => setExpanded((value) => !value)}>
        <span style={{ color: "var(--fg-3)", display: "flex" }}>
          {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        </span>
        <span className="title">{task.title}</span>
        <TaskStatusBadge status={task.status} />
        {running ? <span className="spinner" style={{ width: 11, height: 11 }} /> : null}
        <span className="sub" style={{ marginLeft: "auto" }}>
          {task.providerId} · {task.model}
        </span>
        {!running && changes.length > 0 ? (
          <Button
            size="xs"
            variant="ghost"
            loading={reviewing}
            onClick={async (event) => {
              event.stopPropagation();
              setReviewing(true);
              setReviewOpen(true);
              setReviewText("");
              setReviewError(null);
              try {
                setReviewText(await api.reviewTask(task.id));
              } catch (error) {
                const message = errorMessage(error);
                setReviewError(message);
                notify("error", "代码审查失败", message);
              } finally {
                setReviewing(false);
              }
            }}
          >
            <ShieldCheck size={9} /> AI 审查
          </Button>
        ) : null}
        {running ? (
          <Button
            size="xs"
            variant="ghost"
            onClick={(event) => {
              event.stopPropagation();
              void stopTask(task.id);
            }}
          >
            <Square size={9} /> 停止
          </Button>
        ) : canResume ? (
          <Button
            size="xs"
            variant="ghost"
            onClick={(event) => {
              event.stopPropagation();
              void resumeTask(task.id);
            }}
          >
            <Play size={9} /> 继续
          </Button>
        ) : null}
      </div>
      {expanded ? (
        <div className="timeline-body">
          <div className="row" style={{ gap: 10 }}>
            <span className="section-label">GOAL</span>
            <span className="mono" style={{ fontSize: 9, color: "var(--fg-3)" }}>
              {formatDateTime(task.createdAt)}
            </span>
            <span className="row" style={{ marginLeft: "auto", gap: 5 }}>
              <Clock size={10} style={{ color: "var(--fg-3)" }} />
              <span className="mono" style={{ fontSize: 9, color: "var(--fg-3)" }}>
                {TASK_STATUS_LABEL[status] ?? status}
              </span>
            </span>
          </div>
          <div className="timeline-goal">{task.goal}</div>

          {steps.length > 0 ? (
            <div>
              <div className="section-label" style={{ marginBottom: 5 }}>
                PLAN
              </div>
              <div className="plan-steps">
                {steps.map((step, index) => {
                  const displayStatus =
                    task.status === "completed" && step.status !== "failed"
                      ? "done"
                      : step.status;
                  return (
                    <div
                      className={clsx("plan-step", displayStatus)}
                      key={`${step.title}-${index}`}
                    >
                      <span className="icon">
                        {displayStatus === "done" ? (
                          <CheckCircle2 size={12} className="text-ok" />
                        ) : displayStatus === "in_progress" ? (
                          <Loader2
                            size={12}
                            style={{ animation: "spin .8s linear infinite" }}
                          />
                        ) : displayStatus === "failed" ? (
                          <XCircle size={12} className="text-err" />
                        ) : (
                          <Circle size={10} />
                        )}
                      </span>
                      <span>
                        <span className="text">{step.title}</span>
                        {step.detail ? <div className="detail">{step.detail}</div> : null}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          ) : null}

          {toolViews.length > 0 ? (
            <div className="tool-run">
              <button
                type="button"
                className="tool-run-summary"
                onClick={() => setToolsOpen(!toolsExpanded)}
                aria-expanded={toolsExpanded}
              >
                <span className="tool-run-icon">
                  <Wrench size={13} />
                </span>
                <span className="tool-run-main">
                  <span className="tool-run-top">
                    <span className="tool-run-title">工具调用</span>
                    <span className="tool-run-count">{toolViews.length} 次</span>
                    <span
                      className={clsx(
                        "tool-run-state",
                        toolStats.running > 0 && "running",
                        toolStats.failed > 0 && "failed",
                      )}
                    >
                      {toolStats.running > 0 ? (
                        <>
                          <Loader2
                            size={10}
                            style={{ animation: "spin .8s linear infinite" }}
                          />
                          执行中
                        </>
                      ) : toolStats.failed > 0 ? (
                        `${toolStats.failed} 个失败`
                      ) : toolStats.cancelled > 0 ? (
                        "部分取消"
                      ) : (
                        "全部完成"
                      )}
                    </span>
                    {toolStats.totalMs > 0 ? (
                      <span className="tool-run-time">
                        {formatDuration(toolStats.totalMs)}
                      </span>
                    ) : null}
                  </span>
                  <span className="tool-run-chips">
                    {toolStats.counts.map(([name, count]) => (
                      <span className="tool-chip" key={name}>
                        {toolLabel(name)}
                        <span className="n">×{count}</span>
                      </span>
                    ))}
                  </span>
                </span>
                <ChevronDown
                  size={14}
                  className={clsx(
                    "tool-run-chevron",
                    toolsExpanded && "open",
                  )}
                />
              </button>
              {toolsExpanded ? (
                <div className="tool-run-body">
                  {toolViews.map((tool, index) => {
                    const failed = ["failed", "denied"].includes(tool.status);
                    const showOutput =
                      tool.output &&
                      (failed ||
                        [
                          "run_command",
                          "run_tests",
                          "web_search",
                          "fetch_url",
                          "spawn_subagent",
                        ].includes(tool.name));
                    return (
                      <details className="tool-step" key={tool.callId}>
                        <summary className="tool-step-head">
                          <span className="tool-step-index">
                            {String(index + 1).padStart(2, "0")}
                          </span>
                          <span className="tool-step-icon">
                            <ToolStatusIcon status={tool.status} />
                          </span>
                          <span className="tool-step-name" title={tool.name}>
                            {toolLabel(tool.name)}
                          </span>
                          <span className="tool-step-target">
                            {toolTarget(tool.name, tool.args)}
                          </span>
                          {tool.status !== "completed" ? (
                            <span
                              className={clsx(
                                "tool-step-state",
                                tool.status,
                              )}
                            >
                              {toolStatusLabel(tool.status)}
                            </span>
                          ) : null}
                          {tool.durationMs ? (
                            <span className="tool-step-time">
                              {formatDuration(tool.durationMs)}
                            </span>
                          ) : null}
                          <ChevronRight
                            size={10}
                            className="tool-step-chevron"
                          />
                        </summary>
                        <div className="tool-step-body">
                          <div className="tool-step-args">
                            <span className="k">参数</span>
                            <code title={JSON.stringify(tool.args)}>
                              {JSON.stringify(tool.args)}
                            </code>
                          </div>
                          {tool.summary ? (
                            <div className="tool-step-summary">
                              {tool.summary}
                            </div>
                          ) : null}
                          {showOutput ? (
                            <div className="tool-output">
                              {tool.output.length > 8000
                                ? `${tool.output.slice(0, 8000)}\n…（输出已截断）`
                                : tool.output}
                            </div>
                          ) : null}
                        </div>
                      </details>
                    );
                  })}
                </div>
              ) : null}
            </div>
          ) : null}

          {tests.length > 0 ? (
            <div>
              <div className="section-label" style={{ marginBottom: 5 }}>
                TESTS
              </div>
              {tests.map((event) => {
                const payload = (event.payload ?? {}) as Record<string, any>;
                const ok = Boolean(payload.success);
                return (
                  <div className="tool-card" key={event.id} style={{ marginBottom: 4 }}>
                    <div className="tool-card-head" style={{ cursor: "default" }}>
                      <TestTube2
                        size={12}
                        className={ok ? "text-ok" : "text-err"}
                        style={{ flex: "none" }}
                      />
                      <span className="tool-name">
                        {ok ? "测试通过" : "测试未通过"}
                      </span>
                      <span className="tool-target">
                        {String(payload.command ?? "")}
                      </span>
                      <span className="mono" style={{ fontSize: 8, color: "var(--fg-3)" }}>
                        exit {String(payload.exitCode ?? "?")}
                      </span>
                      <Badge tone={ok ? "ok" : "err"}>{ok ? "PASS" : "FAIL"}</Badge>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : null}

          {changes.length > 0 ? (
            <div>
              <div className="section-label" style={{ marginBottom: 5 }}>
                FILE CHANGES · {changes.length}
              </div>
              <div className="timeline-files">
                {changes.map((change) => (
                  <div
                    className="timeline-file"
                    key={change.id}
                    onClick={() => void openTaskChangeDiff(change)}
                    title="在右侧打开 Diff"
                  >
                    <FileCode size={12} style={{ color: "var(--fg-3)" }} />
                    <span className="truncate">{change.path}</span>
                    <span className="st">{change.status}</span>
                  </div>
                ))}
              </div>
            </div>
          ) : null}

          {task.status === "completed" && task.report ? (
            <div className="report-box">
              <div className="section-label">FINAL REPORT</div>
              <div style={{ marginTop: 4 }}>
                <Markdown content={task.report} />
              </div>
            </div>
          ) : null}
          {task.status === "failed" && task.error ? (
            <div className="timeline-error">{task.error}</div>
          ) : null}
          {status === "interrupted" ? (
            <div className="timeline-interrupted">
              {task.error ||
                "任务已中断，进度与变更已保留。点击「继续」接着执行。"}
            </div>
          ) : null}
          {status === "queued" ? (
            <div style={{ fontSize: 11, color: "var(--fg-3)" }}>
              任务已加入队列，等待前一个任务结束…
            </div>
          ) : null}
          {status === "waitingApproval" ? (
            <div style={{ fontSize: 11, color: "var(--warn)" }}>
              任务已暂停，等待你在下方批准或拒绝工具操作。
            </div>
          ) : null}
          {active && toolViews.length === 0 && steps.length === 0 ? (
            <div className="streaming-hint">
              <span className="spinner" style={{ width: 10, height: 10 }} />
              等待模型与工具事件…
            </div>
          ) : null}
        </div>
      ) : null}
      <Modal
        open={reviewOpen}
        onClose={() => setReviewOpen(false)}
        kicker="AI CODE REVIEW"
        title={`审查 · ${task.title}`}
        size="lg"
      >
        {reviewing ? (
          <div className="inline-flex" style={{ padding: 16 }}>
            <span className="spinner" /> 正在请求审查模型…
          </div>
        ) : reviewError ? (
          <div className="timeline-error">{reviewError}</div>
        ) : (
          <Markdown content={reviewText} />
        )}
      </Modal>
    </section>
  );
}
