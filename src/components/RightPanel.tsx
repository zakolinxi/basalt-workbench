import type { CSSProperties } from "react";
import { PanelRightClose } from "lucide-react";
import { useAppStore } from "../lib/store";
import { IconButton } from "./ui";
import { CodeView } from "./CodeView";
import { ChangesPanel } from "./ChangesPanel";
import { DiffView } from "./DiffView";

export function RightPanel({ style }: { style?: CSSProperties }) {
  const rightTab = useAppStore((state) => state.rightTab);
  const setRightTab = useAppStore((state) => state.setRightTab);
  const setLayout = useAppStore((state) => state.setLayout);
  const changes = useAppStore((state) => state.changes);
  const gitEntries = useAppStore((state) => state.projectGit?.entries.length ?? 0);
  const pendingChanges = changes.filter(
    (change) => change.status === "pending" || change.status === "conflict",
  ).length;

  return (
    <aside className="right-panel" style={style}>
      <div className="right-tabs">
        <button
          className={`right-tab ${rightTab === "code" ? "active" : ""}`}
          onClick={() => setRightTab("code")}
        >
          代码 CODE
        </button>
        <button
          className={`right-tab ${rightTab === "changes" ? "active" : ""}`}
          onClick={() => setRightTab("changes")}
        >
          变更 CHANGES
          <span className="count">{pendingChanges + gitEntries}</span>
        </button>
        <button
          className={`right-tab ${rightTab === "diff" ? "active" : ""}`}
          onClick={() => setRightTab("diff")}
        >
          DIFF
        </button>
        <div className="right-actions">
          <IconButton
            label="收起右侧面板"
            onClick={() => setLayout({ rightCollapsed: true })}
          >
            <PanelRightClose size={14} />
          </IconButton>
        </div>
      </div>
      <div className="right-content">
        {rightTab === "code" ? <CodeView /> : null}
        {rightTab === "changes" ? <ChangesPanel /> : null}
        {rightTab === "diff" ? <DiffView /> : null}
      </div>
    </aside>
  );
}
