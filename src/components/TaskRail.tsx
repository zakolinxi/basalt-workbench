import { useMemo, useState } from "react";
import {
  Bot,
  CheckCircle2,
  Circle,
  FileCode,
  Loader2,
  ShieldCheck,
  XCircle,
} from "lucide-react";
import clsx from "clsx";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../lib/store";
import { Modal } from "./ui";
import { Markdown } from "./Markdown";

function StepIcon({ status }: { status: string }) {
  if (status === "done") {
    return <CheckCircle2 size={12} className="text-ok" />;
  }
  if (status === "in_progress") {
    return (
      <Loader2
        size={12}
        style={{ animation: "spin .8s linear infinite" }}
      />
    );
  }
  if (status === "failed") {
    return <XCircle size={12} className="text-err" />;
  }
  return <Circle size={10} />;
}

export function TaskRail() {
  const task = useAppStore(
    (state) => state.tasks.find((item) => item.id === state.activeTaskId) ?? null,
  );
  const changes = useAppStore((state) => state.changes);
  const taskDetail = useAppStore((state) =>
    state.activeTaskId ? state.taskDetails[state.activeTaskId] : undefined,
  );
  const openTaskChangeDiff = useAppStore((state) => state.openTaskChangeDiff);
  const setRightTab = useAppStore((state) => state.setRightTab);
  const notify = useAppStore((state) => state.notify);
  const [showAll, setShowAll] = useState(false);
  const [reviewOpen, setReviewOpen] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const [reviewText, setReviewText] = useState("");
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [agentOpen, setAgentOpen] = useState<{
    id: string;
    role: string;
    task: string;
    status: string;
    report?: string;
    error?: string;
    rounds: number;
    toolCalls: number;
  } | null>(null);
  const [agentsOpen, setAgentsOpen] = useState(false);

  const subagents = useMemo(() => {
    const map = new Map<
      string,
      {
        id: string;
        role: string;
        task: string;
        status: string;
        report?: string;
        error?: string;
        rounds: number;
        toolCalls: number;
      }
    >();
    for (const event of taskDetail?.events ?? []) {
      const payload = (event.payload ?? {}) as Record<string, any>;
      const id = String(payload.runId ?? "");
      if (!id) continue;
      if (event.type === "SubagentStarted") {
        map.set(id, {
          id,
          role: String(payload.role ?? "researcher"),
          task: String(payload.task ?? ""),
          status: "running",
          rounds: 0,
          toolCalls: 0,
        });
      } else if (event.type === "SubagentCompleted") {
        const existing = map.get(id);
        map.set(id, {
          id,
          role: String(payload.role ?? existing?.role ?? "researcher"),
          task: String(payload.task ?? existing?.task ?? ""),
          status: String(payload.status ?? "completed"),
          report:
            typeof payload.report === "string" ? payload.report : undefined,
          error: typeof payload.error === "string" ? payload.error : undefined,
          rounds: Number(payload.rounds ?? existing?.rounds ?? 0),
          toolCalls: Number(payload.toolCalls ?? existing?.toolCalls ?? 0),
        });
      }
    }
    return [...map.values()];
  }, [taskDetail?.events]);

  if (!task) return null;

  const steps = task.plan ?? [];
  if (steps.length === 0 && changes.length === 0 && subagents.length === 0) {
    return null;
  }
  const visibleSteps = steps.slice(0, 3);
  const done = steps.filter(
    (step) =>
      step.status === "done" ||
      (task.status === "completed" && step.status !== "failed"),
  ).length;
  const canReview = Boolean(
    changes.length > 0 &&
      !["queued", "running", "waitingApproval"].includes(task.status),
  );

  const runReview = async () => {
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
  };

  return (
    <>
      {steps.length > 0 ? (
        <section className="rail-todo">
          <div className="rail-head">
            <span className="section-label">TODO</span>
            <span className="rail-count">
              {done}/{steps.length}
            </span>
            {steps.length > 3 ? (
              <button
                type="button"
                className="rail-more"
                onClick={() => setShowAll(true)}
              >
                显示更多
              </button>
            ) : null}
          </div>
          <div className="plan-steps rail-plan">
            {visibleSteps.map((step, index) => {
              const status =
                task.status === "completed" && step.status !== "failed"
                  ? "done"
                  : step.status;
              return (
                <div
                  className={clsx("plan-step", status)}
                  key={`${step.title}-${index}`}
                >
                  <span className="icon">
                    <StepIcon status={status} />
                  </span>
                  <span className="rail-plan-copy">
                    <span className="text">{step.title}</span>
                    {step.detail ? (
                      <span className="detail">{step.detail}</span>
                    ) : null}
                  </span>
                </div>
              );
            })}
          </div>
        </section>
      ) : null}
      {subagents.length > 0 ? (
        <section className="rail-agents">
          <div className="rail-head">
            <span className="section-label">子代理</span>
            <span className="rail-count">
              {subagents.filter((agent) => agent.status === "completed").length}/
              {subagents.length}
            </span>
            {subagents.length > 3 ? (
              <button
                type="button"
                className="rail-more"
                onClick={() => setAgentsOpen(true)}
              >
                全部
              </button>
            ) : null}
          </div>
          <div className="rail-agent-list">
            {subagents.slice(0, 3).map((agent) => (
              <button
                type="button"
                className="rail-agent"
                key={agent.id}
                onClick={() => setAgentOpen(agent)}
                title={agent.task}
              >
                <span className="rail-agent-icon">
                  {agent.status === "running" ? (
                    <Loader2
                      size={11}
                      style={{ animation: "spin .8s linear infinite" }}
                    />
                  ) : agent.status === "completed" ? (
                    <CheckCircle2 size={11} className="text-ok" />
                  ) : agent.status === "cancelled" ? (
                    <Circle size={10} />
                  ) : (
                    <XCircle size={11} className="text-err" />
                  )}
                </span>
                <span className="rail-agent-copy">
                  <span className="role">
                    <Bot size={9} /> {agent.role}
                  </span>
                  <span className="task truncate">{agent.task}</span>
                </span>
                <span className="rail-agent-meta">
                  {agent.rounds > 0 ? `${agent.rounds}R` : ""}
                  {agent.toolCalls > 0 ? ` · ${agent.toolCalls}T` : ""}
                </span>
              </button>
            ))}
          </div>
        </section>
      ) : null}
      <section className="rail-changes">
        <div className="rail-head">
          <span className="section-label">代码变更</span>
          <span className="rail-count">{changes.length}</span>
          {canReview ? (
            <button
              type="button"
              className="rail-more rail-review"
              onClick={() => void runReview()}
            >
              <ShieldCheck size={9} /> AI 审查
            </button>
          ) : null}
          {changes.length > 0 ? (
            <button
              type="button"
              className="rail-more"
              onClick={() => setRightTab("changes")}
            >
              全部
            </button>
          ) : null}
        </div>
        {changes.length === 0 ? (
          <div className="rail-empty">暂无代码变更</div>
        ) : (
          <div className="rail-change-list">
            {changes.map((change) => (
              <button
                type="button"
                className="rail-change"
                key={change.id}
                onClick={() => void openTaskChangeDiff(change)}
                title="打开 Diff"
              >
                <FileCode size={11} />
                <span className="path truncate">{change.path}</span>
                <span className="st">{change.status}</span>
              </button>
            ))}
          </div>
        )}
      </section>
      <Modal
        open={showAll}
        onClose={() => setShowAll(false)}
        kicker="TASK PLAN"
        title={`全部 TODO · ${steps.length} 项`}
        size="lg"
      >
        <div className="plan-steps modal-plan">
          {steps.map((step, index) => {
            const status =
              task.status === "completed" && step.status !== "failed"
                ? "done"
                : step.status;
            return (
              <div
                className={clsx("plan-step", status)}
                key={`${step.title}-${index}`}
              >
                <span className="icon">
                  <StepIcon status={status} />
                </span>
                <span>
                  <span className="text">{step.title}</span>
                  {step.detail ? <div className="detail">{step.detail}</div> : null}
                </span>
              </div>
            );
          })}
        </div>
      </Modal>
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
      <Modal
        open={agentOpen !== null}
        onClose={() => setAgentOpen(null)}
        kicker={`SUBAGENT · ${agentOpen?.status?.toUpperCase() ?? ""}`}
        title={`${agentOpen?.role ?? ""} 子代理`}
        size="lg"
      >
        {agentOpen ? (
          <div className="agent-detail">
            <div className="agent-detail-task">{agentOpen.task}</div>
            <div className="agent-detail-meta">
              <span>{agentOpen.rounds} 轮</span>
              <span>{agentOpen.toolCalls} 次工具调用</span>
            </div>
            {agentOpen.error ? (
              <div className="timeline-error">{agentOpen.error}</div>
            ) : agentOpen.report ? (
              <Markdown content={agentOpen.report} />
            ) : (
              <div className="rail-empty">子代理仍在执行，完成后会显示报告。</div>
            )}
          </div>
        ) : null}
      </Modal>
      <Modal
        open={agentsOpen}
        onClose={() => setAgentsOpen(false)}
        kicker="SUBAGENT TREE"
        title={`子代理 · ${subagents.length} 个`}
        size="lg"
      >
        <div className="agent-tree">
          {subagents.map((agent) => (
            <button
              type="button"
              className="rail-agent"
              key={agent.id}
              onClick={() => {
                setAgentsOpen(false);
                setAgentOpen(agent);
              }}
            >
              <span className="rail-agent-icon">
                {agent.status === "running" ? (
                  <Loader2
                    size={11}
                    style={{ animation: "spin .8s linear infinite" }}
                  />
                ) : agent.status === "completed" ? (
                  <CheckCircle2 size={11} className="text-ok" />
                ) : (
                  <XCircle size={11} className="text-err" />
                )}
              </span>
              <span className="rail-agent-copy">
                <span className="role">
                  <Bot size={9} /> {agent.role} · {agent.status}
                </span>
                <span className="task">{agent.task}</span>
              </span>
              <span className="rail-agent-meta">
                {agent.rounds}R · {agent.toolCalls}T
              </span>
            </button>
          ))}
        </div>
      </Modal>
    </>
  );
}
