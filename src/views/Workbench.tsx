import { useCallback, useEffect, useRef, useState } from "react";
import {
  FolderOpen,
  PanelLeft,
  PanelRight,
  Plus,
  TerminalSquare,
  X,
} from "lucide-react";
import { useAppStore, currentProject, currentSession } from "../lib/store";
import { taskStatusClass, TASK_STATUS_LABEL } from "../lib/format";
import { pickAndAddProject } from "../lib/project";
import { Sidebar } from "../components/Sidebar";
import { TitlebarDrag } from "../components/WindowControls";
import { ChatView } from "../components/ChatView";
import { RightPanel } from "../components/RightPanel";
import { TerminalPanel } from "../components/TerminalPanel";
import { Button, IconButton, TaskStatusBadge } from "../components/ui";
import type { TaskStatus } from "../lib/types";

export function Workbench() {
  const layout = useAppStore((state) => state.layout);
  const setLayout = useAppStore((state) => state.setLayout);
  const project = useAppStore(currentProject);
  const session = useAppStore(currentSession);
  const task = useAppStore(
    (state) =>
      state.tasks.find((item) => item.id === state.activeTaskId) ?? null,
  );
  const newSession = useAppStore((state) => state.newSession);
  const [dragging, setDragging] = useState<"sidebar" | "right" | "terminal" | null>(
    null,
  );

  const beginResize = useCallback(
    (
      mode: "sidebar" | "right" | "terminal",
      event: React.PointerEvent<HTMLDivElement>,
    ) => {
      event.preventDefault();
      const startX = event.clientX;
      const startY = event.clientY;
      const state = useAppStore.getState().layout;
      const initial =
        mode === "sidebar"
          ? state.sidebarWidth
          : mode === "right"
            ? state.rightWidth
            : state.terminalHeight;
      setDragging(mode);
      document.body.style.userSelect = "none";
      document.body.style.cursor = mode === "terminal" ? "row-resize" : "col-resize";
      const move = (moveEvent: PointerEvent) => {
        if (mode === "sidebar") {
          const next = Math.max(204, Math.min(440, initial + moveEvent.clientX - startX));
          setLayout({ sidebarWidth: next });
        } else if (mode === "right") {
          const next = Math.max(340, Math.min(960, initial - (moveEvent.clientX - startX)));
          setLayout({ rightWidth: next });
        } else {
          const next = Math.max(
            140,
            Math.min(window.innerHeight - 260, initial - (moveEvent.clientY - startY)),
          );
          setLayout({ terminalHeight: next });
        }
      };
      const up = () => {
        setDragging(null);
        document.body.style.userSelect = "";
        document.body.style.cursor = "";
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    },
    [setLayout],
  );

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (useAppStore.getState().view !== "workbench") return;
      if (!event.ctrlKey) return;
      if (event.key.toLowerCase() === "n" && !event.shiftKey) {
        event.preventDefault();
        void newSession();
      } else if (event.key.toLowerCase() === "n" && event.shiftKey) {
        event.preventDefault();
        window.dispatchEvent(
          new CustomEvent("basalt:focus-composer", { detail: { mode: "agent" } }),
        );
      } else if (event.key.toLowerCase() === "b") {
        event.preventDefault();
        setLayout({ sidebarCollapsed: !layout.sidebarCollapsed });
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [layout.sidebarCollapsed, newSession, setLayout]);

  return (
    <div className="view">
      <div className="workbench">
        {!layout.sidebarCollapsed ? (
          <>
            <Sidebar style={{ width: layout.sidebarWidth }} />
            <div
              className={`splitter-col ${dragging === "sidebar" ? "dragging" : ""}`}
              onPointerDown={(event) => beginResize("sidebar", event)}
            />
          </>
        ) : null}
        <div className="main-area">
          <TopBar />
          {!project ? (
            <ProjectOnboarding />
          ) : (
            <>
              <div className="workspace-row">
                <ChatView />
                {!layout.rightCollapsed ? (
                  <>
                    <div
                      className={`splitter-col ${dragging === "right" ? "dragging" : ""}`}
                      onPointerDown={(event) => beginResize("right", event)}
                    />
                    <RightPanel style={{ width: layout.rightWidth }} />
                  </>
                ) : null}
              </div>
              {!layout.terminalCollapsed ? (
                <div
                  className={`splitter-row ${dragging === "terminal" ? "dragging" : ""}`}
                  onPointerDown={(event) => beginResize("terminal", event)}
                />
              ) : null}
              <div
                style={{
                  display: layout.terminalCollapsed ? "none" : "flex",
                  flex: "none",
                }}
              >
                <TerminalPanel style={{ height: layout.terminalHeight }} />
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function TopBar() {
  const project = useAppStore(currentProject);
  const session = useAppStore(currentSession);
  const task = useAppStore(
    (state) =>
      state.tasks.find((item) => item.id === state.activeTaskId) ?? null,
  );
  const layout = useAppStore((state) => state.layout);
  const setLayout = useAppStore((state) => state.setLayout);
  const newSession = useAppStore((state) => state.newSession);
  const terminals = useAppStore((state) => state.terminals);
  const rightTab = useAppStore((state) => state.rightTab);

  return (
    <header className="topbar">
      <div className="breadcrumb">
        <IconButton
          label={layout.sidebarCollapsed ? "展开侧栏" : "收起侧栏"}
          onClick={() => setLayout({ sidebarCollapsed: !layout.sidebarCollapsed })}
        >
          <PanelLeft size={15} />
        </IconButton>
        {project ? (
          <>
            <span className="crumb">
              <FolderOpen size={13} />
              {project.name}
            </span>
            <span className="sep">/</span>
            <span className="crumb muted">{session?.title ?? "未选择会话"}</span>
          </>
        ) : (
          <span className="crumb muted">NO PROJECT</span>
        )}
        {task ? (
          <>
            <span className="sep">/</span>
            <button
              className="crumb"
              onClick={() => {
                const detail = useAppStore.getState().taskDetails[task.id];
                if (detail) useAppStore.getState().selectTask(task.id);
              }}
            >
              <span
                className={`status-dot ${task.status}`}
                style={{ display: "inline-block" }}
              />
              {task.title}
            </button>
            <TaskStatusBadge status={task.status} />
          </>
        ) : null}
      </div>
      <TitlebarDrag />
      <div className="topbar-actions">
        <Button size="sm" variant="ghost" onClick={() => void newSession()}>
          <Plus size={12} /> 新会话
        </Button>
        <IconButton
          label={terminals.length ? "切换终端" : "打开终端"}
          onClick={async () => {
            if (!terminals.length) {
              await useAppStore.getState().newTerminal();
            } else {
              setLayout({ terminalCollapsed: !layout.terminalCollapsed });
            }
          }}
          active={!layout.terminalCollapsed}
        >
          <TerminalSquare size={15} />
        </IconButton>
        <IconButton
          label={layout.rightCollapsed ? "展开右侧面板" : "收起右侧面板"}
          onClick={() => setLayout({ rightCollapsed: !layout.rightCollapsed })}
          active={!layout.rightCollapsed}
        >
          <PanelRight size={15} />
        </IconButton>
      </div>
    </header>
  );
}

function ProjectOnboarding() {
  const projects = useAppStore((state) => state.projects);
  const setCurrentProject = useAppStore((state) => state.setCurrentProject);
  return (
    <div className="chat-empty">
      <div className="chat-empty-inner">
        <div className="brand-line">
          <span className="brand-mark">
            <span />
            <span />
            <span />
          </span>
          <div>
            <h1>打开一个项目，开始工作</h1>
            <div className="sub">SELECT A WORKSPACE</div>
          </div>
        </div>
        <p>
          Basalt 只会访问你明确授权的本地项目目录。添加项目后，你可以进行多轮对话、
          Agent 编程任务、实时 Diff 审查和真实终端操作。
        </p>
        <div className="empty-actions">
          <Button variant="primary" onClick={() => void pickAndAddProject()}>
            <FolderOpen size={13} /> 添加项目目录
          </Button>
        </div>
        {projects.length > 0 ? (
          <>
            <div className="section-label" style={{ marginBottom: 8 }}>
              RECENT PROJECTS
            </div>
            {projects.slice(0, 5).map((project) => (
              <button
                key={project.id}
                className="side-row"
                style={{ paddingLeft: 4 }}
                onClick={() => void setCurrentProject(project.id)}
              >
                <FolderOpen size={13} />
                <span className="label">{project.name}</span>
                <span className="sub">
                  {project.path.length > 50
                    ? `…${project.path.slice(-48)}`
                    : project.path}
                </span>
              </button>
            ))}
          </>
        ) : null}
        <div className="empty-grid" style={{ marginTop: 26 }}>
          <div className="empty-cell">
            <TerminalSquare size={14} />
            <div>
              <div className="t">真实终端</div>
              <div className="d">PowerShell / CMD / WSL，用户命令与 Agent 命令严格区分。</div>
            </div>
          </div>
          <div className="empty-cell">
            <X size={14} />
            <div>
              <div className="t">权限与审批</div>
              <div className="d">文件写入和终端命令默认需要你的明确批准。</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
