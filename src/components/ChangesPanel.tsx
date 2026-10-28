import { useState } from "react";
import {
  Check,
  FileCode,
  GitBranch,
  RefreshCw,
  RotateCcw,
} from "lucide-react";
import { useAppStore } from "../lib/store";
import { errorMessage } from "../lib/api";
import type { Change } from "../lib/types";
import { Badge, Button, ConfirmDialog, IconButton, Modal } from "./ui";

function changeTone(status: string): "ok" | "warn" | "err" | undefined {
  if (status === "accepted") return "ok";
  if (status === "conflict") return "err";
  if (status === "pending") return "warn";
  return undefined;
}

export function ChangesPanel() {
  const changes = useAppStore((state) => state.changes);
  const git = useAppStore((state) => state.projectGit);
  const tasks = useAppStore((state) => state.tasks);
  const activeTaskId = useAppStore((state) => state.activeTaskId);
  const detail = useAppStore((state) =>
    state.activeTaskId ? state.taskDetails[state.activeTaskId] : undefined,
  );
  const tests = (detail?.events ?? [])
    .filter((event) => event.type === "TestCompleted")
    .slice(-4);
  const openTaskChangeDiff = useAppStore((state) => state.openTaskChangeDiff);
  const openGitDiff = useAppStore((state) => state.openGitDiff);
  const resolveChange = useAppStore((state) => state.resolveChange);
  const refreshChanges = useAppStore((state) => state.refreshChanges);
  const refreshGit = useAppStore((state) => state.refreshGit);
  const [revertTarget, setRevertTarget] = useState<Change | null>(null);
  const [conflict, setConflict] = useState<{
    change: Change;
    action: "accept" | "revert";
    message: string;
  } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const task = tasks.find((item) => item.id === activeTaskId);

  const handle = async (
    change: Change,
    action: "accept" | "revert",
    force = false,
  ) => {
    setBusy(change.id);
    try {
      await resolveChange(change.id, action, force);
      setConflict(null);
      setRevertTarget(null);
    } catch (error) {
      const message = errorMessage(error);
      if (message.includes("冲突")) {
        setConflict({ change, action, message });
      }
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="changes-scroll">
      <div className="changes-head">
        <span>{task ? "任务变更" : "变更记录"}</span>
        {task ? (
          <span className="truncate" style={{ textTransform: "none", letterSpacing: 0, color: "var(--fg-3)", maxWidth: 160 }}>
            {task.title}
          </span>
        ) : null}
        <span className="count">{changes.length} FILES</span>
        <IconButton
          label="刷新变更"
          size="sm"
          onClick={() => {
            void refreshChanges();
            void refreshGit();
          }}
        >
          <RefreshCw size={11} />
        </IconButton>
      </div>
      {changes.length === 0 ? (
        <div className="git-note">
          当前没有由 Agent 记录的文件变更。每次写入都会创建可审查、可撤销的快照。
        </div>
      ) : (
        changes.map((change) => {
          const actionable = change.status === "pending" || change.status === "conflict";
          return (
            <div
              className="change-row"
              key={change.id}
              onClick={() => void openTaskChangeDiff(change)}
              title="查看 Diff"
            >
              <Badge tone={changeTone(change.status)}>{change.status}</Badge>
              <span className="path">{change.path}</span>
              <span className="actions">
                <IconButton
                  label="查看 Diff"
                  size="sm"
                  onClick={(event) => {
                    event.stopPropagation();
                    void openTaskChangeDiff(change);
                  }}
                >
                  <FileCode size={11} />
                </IconButton>
                {actionable ? (
                  <>
                    <IconButton
                      label="接受变更"
                      size="sm"
                      disabled={busy === change.id}
                      onClick={(event) => {
                        event.stopPropagation();
                        void handle(change, "accept");
                      }}
                    >
                      <Check size={11} />
                    </IconButton>
                    <IconButton
                      label="撤销变更"
                      size="sm"
                      disabled={busy === change.id}
                      onClick={(event) => {
                        event.stopPropagation();
                        setRevertTarget(change);
                      }}
                    >
                      <RotateCcw size={11} />
                    </IconButton>
                  </>
                ) : null}
              </span>
            </div>
          );
        })
      )}

      {tests.length > 0 ? (
        <>
          <div className="changes-head" style={{ top: 31 }}>
            TEST RESULTS
            <span className="count">{tests.length}</span>
          </div>
          {tests.map((event) => {
            const payload = (event.payload ?? {}) as Record<string, any>;
            const ok = Boolean(payload.success);
            return (
              <div className="git-row" key={event.id} style={{ cursor: "default" }}>
                <span
                  className="mono"
                  style={{ fontSize: 9, color: ok ? "var(--ok)" : "var(--err)" }}
                >
                  {ok ? "PASS" : "FAIL"}
                </span>
                <span className="path">{String(payload.command ?? "")}</span>
                <Badge tone={ok ? "ok" : "err"}>
                  exit {String(payload.exitCode ?? "?")}
                </Badge>
              </div>
            );
          })}
        </>
      ) : null}

      <div className="changes-head" style={{ top: tests.length > 0 ? 62 : 31 }}>
        <GitBranch size={11} />
        GIT
        {git?.isRepo && git.branch ? (
          <span className="badge plain" style={{ textTransform: "none" }}>
            {git.branch}
          </span>
        ) : null}
        <span className="count">{git?.entries.length ?? 0} FILES</span>
        <IconButton label="刷新 Git 状态" size="sm" onClick={() => void refreshGit()}>
          <RefreshCw size={11} />
        </IconButton>
      </div>
      {!git ? (
        <div className="git-note">正在读取 Git 状态…</div>
      ) : !git.isRepo ? (
        <div className="git-note">
          当前项目不是 Git 仓库。你仍然可以通过任务变更快照审查 Agent 的修改。
          {git.error ? <div style={{ marginTop: 4, color: "var(--err)" }}>{git.error}</div> : null}
        </div>
      ) : git.entries.length === 0 ? (
        <div className="git-note">工作区干净，没有未提交的 Git 变更。</div>
      ) : (
        git.entries.map((entry) => (
          <div
            className="git-row"
            key={`${entry.path}-${entry.status}`}
            onClick={() => void openGitDiff(entry.path.split(" -> ").pop() ?? entry.path)}
            title="查看 HEAD 与工作区的 Diff"
          >
            <span
              className="mono"
              style={{
                fontSize: 9,
                width: 20,
                color:
                  entry.status === "deleted"
                    ? "var(--err)"
                    : entry.status === "untracked" || entry.status === "added"
                      ? "var(--ok)"
                      : "var(--warn)",
              }}
            >
              {entry.untracked
                ? "??"
                : entry.status === "modified"
                  ? "M"
                  : entry.status === "deleted"
                    ? "D"
                    : entry.status === "renamed"
                      ? "R"
                      : entry.status === "added"
                        ? "A"
                        : "•"}
            </span>
            <span className="path">{entry.path}</span>
            {entry.staged ? <Badge>STAGED</Badge> : null}
            <span className="actions">
              <IconButton
                label="查看 Git Diff"
                size="sm"
                onClick={(event) => {
                  event.stopPropagation();
                  void openGitDiff(entry.path.split(" -> ").pop() ?? entry.path);
                }}
              >
                <FileCode size={11} />
              </IconButton>
            </span>
          </div>
        ))
      )}

      <ConfirmDialog
        open={revertTarget !== null}
        title="撤销文件变更"
        message={
          <>
            将把 <strong>{revertTarget?.path}</strong> 恢复到该变更写入前的内容。
            如果文件在此之后被修改，Basalt 会检测到冲突并阻止直接覆盖。
          </>
        }
        detail={
          revertTarget?.oldExists
            ? "该文件将恢复为变更前的快照。"
            : "该文件是 Agent 新建的，撤销将删除它。"
        }
        confirmLabel="撤销变更"
        danger
        onCancel={() => setRevertTarget(null)}
        onConfirm={() => {
          if (revertTarget) void handle(revertTarget, "revert");
        }}
      />

      <Modal
        open={conflict !== null}
        onClose={() => setConflict(null)}
        kicker="CONFLICT DETECTED"
        title="文件在变更后被修改"
        footer={
          <>
            <Button onClick={() => setConflict(null)}>取消</Button>
            {conflict?.action === "revert" ? (
              <Button
                variant="danger"
                onClick={() => {
                  if (conflict) void handle(conflict.change, "revert", true);
                }}
              >
                强制恢复到变更前
              </Button>
            ) : (
              <Button
                variant="danger"
                onClick={() => {
                  if (conflict) void handle(conflict.change, "accept", true);
                }}
              >
                仍然接受
              </Button>
            )}
          </>
        }
      >
        <div style={{ fontSize: 12, color: "var(--fg-1)", lineHeight: 1.65 }}>
          {conflict?.message}
        </div>
        <div className="warning-box">
          强制操作会覆盖文件当前内容
          {conflict?.action === "revert" ? "，恢复为 Agent 写入前的快照" : ""}。
          建议先在 Diff 面板中审查当前内容，确认没有需要保留的用户修改。
        </div>
      </Modal>
    </div>
  );
}
