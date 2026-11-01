import { useMemo, useState } from "react";
import {
  CheckCircle2,
  ChevronRight,
  Circle,
  Loader2,
  TestTube2,
  XCircle,
} from "lucide-react";
import clsx from "clsx";
import { toolLabel, toolTarget } from "../lib/format";
import type { TaskEvent } from "../lib/types";
import { Badge } from "./ui";

interface ToolView {
  callId: string;
  name: string;
  args: Record<string, unknown>;
  status: string;
  summary: string;
  output: string;
  durationMs?: number;
}

const ACTIVITY_TYPES = new Set([
  "ToolCallStarted",
  "ToolCallCompleted",
  "TerminalOutput",
  "TestCompleted",
]);

export function isTaskActivityEvent(event: TaskEvent): boolean {
  return ACTIVITY_TYPES.has(event.type);
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

export function TaskActivity({ events }: { events: TaskEvent[] }) {
  const [open, setOpen] = useState(false);
  const tools = useMemo(() => buildToolViews(events), [events]);
  const tests = useMemo(
    () => events.filter((event) => event.type === "TestCompleted"),
    [events],
  );
  if (!tools.length && !tests.length) return null;

  const failed = tools.filter((tool) =>
    ["failed", "denied"].includes(tool.status),
  ).length;
  const cancelled = tools.filter((tool) => tool.status === "cancelled").length;
  const running = tools.some((tool) =>
    ["running", "pending"].includes(tool.status),
  );

  return (
    <div className="task-activity">
      {tools.length > 0 ? (
        <div className="tool-run">
          <button
            type="button"
            className="tool-run-summary"
            onClick={() => setOpen((value) => !value)}
            aria-expanded={open}
          >
            <span className={clsx("tool-run-chevron", open && "open")}>
              <ChevronRight size={12} />
            </span>
            <span className="tool-run-label">
              {`执行工具 ${tools.length} 次${
                failed > 0
                  ? `，其中 ${failed} 次失败`
                  : cancelled > 0
                    ? `，${cancelled} 次取消`
                    : ""
              }${running ? "，正在执行" : ""}`}
            </span>
            {running ? <span className="spinner" style={{ width: 10, height: 10 }} /> : null}
          </button>
          {open ? (
            <div className="tool-run-body">
              {tools.map((tool, index) => {
                const showOutput =
                  tool.output &&
                  (["failed", "denied"].includes(tool.status) ||
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
      {tests.map((event) => {
        const payload = (event.payload ?? {}) as Record<string, any>;
        const ok = Boolean(payload.success);
        return (
          <div className="task-test-row" key={event.id}>
            <TestTube2
              size={12}
              className={ok ? "text-ok" : "text-err"}
              style={{ flex: "none" }}
            />
            <span className="task-test-label">
              {ok ? "测试通过" : "测试未通过"}
            </span>
            <span className="task-test-command">
              {String(payload.command ?? "")}
            </span>
            <Badge tone={ok ? "ok" : "err"}>{ok ? "PASS" : "FAIL"}</Badge>
            <span className="task-test-exit">
              exit {String(payload.exitCode ?? "?")}
            </span>
          </div>
        );
      })}
    </div>
  );
}
