import { useState } from "react";
import {
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
  const openTaskChangeDiff = useAppStore((state) => state.openTaskChangeDiff);
  const setRightTab = useAppStore((state) => state.setRightTab);
  const notify = useAppStore((state) => state.notify);
  const [showAll, setShowAll] = useState(false);
  const [reviewOpen, setReviewOpen] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const [reviewText, setReviewText] = useState("");
  const [reviewError, setReviewError] = useState<string | null>(null);

  if (!task) return null;

  const steps = task.plan ?? [];
  if (steps.length === 0 && changes.length === 0) return null;
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
    </>
  );
}
