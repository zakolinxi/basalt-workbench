import { useMemo, useState, type CSSProperties } from "react";
import {
  ChevronDown,
  ChevronRight,
  FolderOpen,
  FolderPlus,
  GripVertical,
  MessageSquare,
  Plus,
  Search,
  Settings,
  Trash2,
} from "lucide-react";
import clsx from "clsx";
import { useAppStore, currentProject } from "../lib/store";
import { pickAndAddProject } from "../lib/project";
import { relativeTime } from "../lib/format";
import { ConfirmDialog, IconButton } from "./ui";

export function Sidebar({ style }: { style?: CSSProperties }) {
  const projects = useAppStore((state) => state.projects);
  const sessions = useAppStore((state) => state.sessions);
  const tasks = useAppStore((state) => state.tasks);
  const project = useAppStore(currentProject);
  const currentSessionId = useAppStore((state) => state.currentSessionId);
  const setCurrentProject = useAppStore((state) => state.setCurrentProject);
  const newSession = useAppStore((state) => state.newSession);
  const selectSession = useAppStore((state) => state.selectSession);
  const deleteSession = useAppStore((state) => state.deleteSession);
  const removeProject = useAppStore((state) => state.removeProject);
  const setView = useAppStore((state) => state.setView);
  const appVersion = useAppStore((state) => state.appVersion);

  const [query, setQuery] = useState("");
  const [sessionsCollapsed, setSessionsCollapsed] = useState(false);
  const [confirm, setConfirm] = useState<
    | { kind: "session" | "project"; id: string; title: string }
    | null
  >(null);

  const latestTaskByProject = useMemo(() => {
    const latest = new Map<string, (typeof tasks)[number]>();
    for (const task of tasks) {
      const previous = latest.get(task.projectId);
      if (!previous || task.createdAt > previous.createdAt) {
        latest.set(task.projectId, task);
      }
    }
    return latest;
  }, [tasks]);

  const projectSessions = useMemo(
    () =>
      sessions.filter(
        (session) =>
          session.projectId === project?.id ||
          (!session.projectId && !project),
      ),
    [project?.id, sessions],
  );

  const needle = query.trim().toLowerCase();
  const filteredSessions = projectSessions.filter(
    (session) => !needle || session.title.toLowerCase().includes(needle),
  );

  return (
    <aside className="sidebar" style={style}>
      <div className="workspace-panel">
        <div className="workspace-label">工作区</div>
        <div className="workspace-list">
          {projects.length === 0 ? (
            <div className="workspace-empty">还没有项目</div>
          ) : null}
          {projects.map((item) => {
            const latest = latestTaskByProject.get(item.id);
            const active = item.id === project?.id;
            return (
              <button
                type="button"
                key={item.id}
                className={clsx("workspace-item", active && "active")}
                onClick={() => void setCurrentProject(item.id)}
              >
                <FolderOpen size={18} className="workspace-folder" />
                <span className="workspace-meta">
                  <span className="workspace-name">{item.name}</span>
                  <span className={clsx("workspace-sub", !latest && "empty")}>
                    {latest?.title ?? "暂无任务"}
                  </span>
                </span>
                {active ? (
                  <span
                    className="workspace-grip"
                    title="当前工作区"
                    aria-hidden="true"
                  >
                    <GripVertical size={14} />
                  </span>
                ) : null}
              </button>
            );
          })}
        </div>
        <div className="workspace-actions">
          <button type="button" onClick={() => void pickAndAddProject()}>
            <FolderPlus size={13} /> 添加项目目录
          </button>
          {project ? (
            <button
              type="button"
              className="danger"
              onClick={() =>
                setConfirm({
                  kind: "project",
                  id: project.id,
                  title: project.name,
                })
              }
            >
              <Trash2 size={13} /> 移除当前项目
            </button>
          ) : null}
        </div>
      </div>

      <div className="side-search">
        <Search size={12} />
        <input
          className="input search"
          placeholder="搜索会话"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>

      <div className="side-actions">
        <button className="btn sm" onClick={() => void newSession()} disabled={!project}>
          <Plus size={12} /> 会话
        </button>
      </div>

      <div className="side-scroll">
        <div className="side-section">
          <div className="side-heading">
            <button
              onClick={() => setSessionsCollapsed((value) => !value)}
              aria-label="折叠会话"
            >
              {sessionsCollapsed ? (
                <ChevronRight size={11} />
              ) : (
                <ChevronDown size={11} />
              )}
            </button>
            <MessageSquare size={11} />
            会话
            <span className="count">{filteredSessions.length}</span>
          </div>
          {!sessionsCollapsed ? (
            filteredSessions.length === 0 ? (
              <div className="sidebar-empty">还没有会话</div>
            ) : (
              filteredSessions.map((session) => (
                <div
                  key={session.id}
                  className={clsx(
                    "side-row",
                    session.id === currentSessionId && "active",
                  )}
                  onClick={() => void selectSession(session.id)}
                  title={session.title}
                >
                  <MessageSquare size={12} style={{ color: "var(--fg-3)", flex: "none" }} />
                  <span className="label">{session.title}</span>
                  <span className="row-actions">
                    <IconButton
                      size="sm"
                      label="删除会话"
                      onClick={(event) => {
                        event.stopPropagation();
                        setConfirm({
                          kind: "session",
                          id: session.id,
                          title: session.title,
                        });
                      }}
                    >
                      <Trash2 size={10} />
                    </IconButton>
                  </span>
                  <span className="sub">{relativeTime(session.updatedAt)}</span>
                </div>
              ))
            )
          ) : null}
        </div>
      </div>

      <div className="sidebar-foot">
        <button className="icon-btn" title="设置" onClick={() => setView("settings")}>
          <Settings size={14} />
        </button>
        <span className="version">v{appVersion}</span>
        <span className="badge plain" title="所有数据保存在本地">
          LOCAL
        </span>
      </div>

      <ConfirmDialog
        open={confirm !== null}
        title={confirm?.kind === "project" ? "从工作区移除项目" : "删除会话"}
        message={
          confirm?.kind === "project" ? (
            <>
              将从 Basalt 移除 <strong>{confirm.title}</strong>。
              磁盘上的项目文件不会被删除，之后可以重新添加。
            </>
          ) : (
            <>
              将删除会话 <strong>{confirm?.title}</strong> 及其消息、任务记录与变更记录。
              已生成的文件和磁盘项目不会被删除。
            </>
          )
        }
        confirmLabel={confirm?.kind === "project" ? "移除" : "删除"}
        danger
        onCancel={() => setConfirm(null)}
        onConfirm={() => {
          if (!confirm) return;
          if (confirm.kind === "project") void removeProject(confirm.id);
          if (confirm.kind === "session") void deleteSession(confirm.id);
          setConfirm(null);
        }}
      />
    </aside>
  );
}
