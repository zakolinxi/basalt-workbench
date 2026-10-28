import { useMemo, useState } from "react";
import {
  Check,
  GitBranch,
  RefreshCw,
  RotateCcw,
} from "lucide-react";
import { useAppStore } from "../lib/store";
import { errorMessage } from "../lib/api";
import { computeSplit, computeUnified } from "../lib/diff";
import type { Change } from "../lib/types";
import { Badge, Button, ConfirmDialog, IconButton, Modal } from "./ui";

export function DiffView() {
  const diff = useAppStore((state) => state.diff);
  const changes = useAppStore((state) => state.changes);
  const resolveChange = useAppStore((state) => state.resolveChange);
  const openTaskChangeDiff = useAppStore((state) => state.openTaskChangeDiff);
  const openGitDiff = useAppStore((state) => state.openGitDiff);
  const [viewMode, setViewMode] = useState<"unified" | "split">("unified");
  const [revertOpen, setRevertOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [conflict, setConflict] = useState<string | null>(null);

  const unified = useMemo(
    () => (diff ? computeUnified(diff.oldText, diff.newText) : null),
    [diff],
  );
  const split = useMemo(
    () => (diff ? computeSplit(diff.oldText, diff.newText) : null),
    [diff],
  );

  if (!diff || !unified || !split) {
    return (
      <div className="diff-placeholder">
        <GitBranch size={24} strokeWidth={1.2} />
        <div>选择一条变更记录或 Git 文件查看 Diff</div>
        <div className="mono" style={{ fontSize: 9 }}>
          变更面板 · 任务时间线 · 文件变更
        </div>
      </div>
    );
  }

  const change: Change | undefined = diff.changeId
    ? changes.find((item) => item.id === diff.changeId)
    : undefined;
  const actionable =
    Boolean(change) && (change?.status === "pending" || change?.status === "conflict");

  const resolve = async (action: "accept" | "revert", force = false) => {
    if (!change) return;
    setBusy(true);
    try {
      await resolveChange(change.id, action, force);
      setRevertOpen(false);
      setConflict(null);
    } catch (error) {
      const message = errorMessage(error);
      if (message.includes("冲突")) setConflict(message);
    } finally {
      setBusy(false);
    }
  };

  const reload = () => {
    if (change) {
      const latest = changes.find((item) => item.id === change.id);
      if (latest) void openTaskChangeDiff(latest);
    } else if (diff.projectId && diff.source === "git") {
      void openGitDiff(diff.path);
    }
  };

  return (
    <>
      <div className="diff-toolbar">
        <span className="file" title={diff.path}>
          {diff.path}
        </span>
        <Badge tone={diff.source === "git" ? "info" : "warn"}>{diff.source}</Badge>
        {change ? <Badge>{change.status}</Badge> : null}
        <span className="diff-stats">
          <span className="add">+{unified.additions}</span>
          <span className="del">-{unified.deletions}</span>
        </span>
        <div className="segmented">
          <button
            className={viewMode === "unified" ? "active" : ""}
            onClick={() => setViewMode("unified")}
          >
            统一
          </button>
          <button
            className={viewMode === "split" ? "active" : ""}
            onClick={() => setViewMode("split")}
          >
            并排
          </button>
        </div>
        <IconButton label="刷新 Diff" onClick={reload}>
          <RefreshCw size={12} />
        </IconButton>
        {actionable ? (
          <>
            <Button
              size="xs"
              variant="primary"
              loading={busy}
              onClick={() => void resolve("accept")}
            >
              <Check size={10} /> 接受
            </Button>
            <Button size="xs" variant="danger" onClick={() => setRevertOpen(true)}>
              <RotateCcw size={10} /> 撤销
            </Button>
          </>
        ) : null}
      </div>
      {diff.binary ? (
        <div className="diff-empty">二进制文件无法显示文本 Diff。</div>
      ) : null}
      {diff.truncated ? (
        <div className="code-file-note">文件较大，Diff 内容已截断。</div>
      ) : null}
      {!diff.binary ? (
        <div className="diff-scroll">
          {viewMode === "unified" ? (
            <table className="diff-table">
              <tbody>
                {unified.rows.map((row, index) =>
                  row.kind === "hunk" ? (
                    <tr className="diff-row hunk" key={index}>
                      <td colSpan={4}>{row.text}</td>
                    </tr>
                  ) : (
                    <tr className={`diff-row ${row.kind}`} key={index}>
                      <td className="diff-line-no">{row.oldLine ?? ""}</td>
                      <td className="diff-line-no">{row.newLine ?? ""}</td>
                      <td className="diff-sign">
                        {row.kind === "add" ? "+" : row.kind === "del" ? "−" : ""}
                      </td>
                      <td className="diff-code">{row.text || " "}</td>
                    </tr>
                  ),
                )}
              </tbody>
            </table>
          ) : (
            <div className="diff-split">
              <div className="side">
                <div className="side-head">
                  {diff.oldLabel} · {unified.deletions} 删除
                </div>
                {split.rows.map((row, index) => (
                  <div className={`split-row ${row.left.kind}`} key={`l-${index}`}>
                    <span className="ln">{row.left.line ?? ""}</span>
                    <span className="code">{row.left.text || " "}</span>
                  </div>
                ))}
              </div>
              <div className="side">
                <div className="side-head">
                  {diff.newLabel} · {unified.additions} 新增
                </div>
                {split.rows.map((row, index) => (
                  <div className={`split-row ${row.right.kind}`} key={`r-${index}`}>
                    <span className="ln">{row.right.line ?? ""}</span>
                    <span className="code">{row.right.text || " "}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
          {unified.rows.length === 0 ? (
            <div className="diff-empty">内容没有差异。</div>
          ) : null}
        </div>
      ) : null}

      <ConfirmDialog
        open={revertOpen}
        title="撤销文件变更"
        message={
          <>
            将恢复 <strong>{diff.path}</strong> 到该变更写入前的内容。
            如果文件已被外部修改，系统会阻止操作并提示冲突。
          </>
        }
        confirmLabel="撤销变更"
        danger
        loading={busy}
        onCancel={() => setRevertOpen(false)}
        onConfirm={() => void resolve("revert")}
      />

      <Modal
        open={conflict !== null}
        onClose={() => setConflict(null)}
        kicker="CONFLICT DETECTED"
        title="检测到文件冲突"
        footer={
          <>
            <Button onClick={() => setConflict(null)}>取消</Button>
            <Button variant="danger" onClick={() => void resolve("revert", true)}>
              强制恢复
            </Button>
          </>
        }
      >
        <div style={{ fontSize: 12, lineHeight: 1.65, color: "var(--fg-1)" }}>
          {conflict}
        </div>
        <div className="warning-box">
          强制恢复会覆盖文件当前内容。请先确认当前内容没有需要保留的用户修改。
        </div>
      </Modal>
    </>
  );
}
