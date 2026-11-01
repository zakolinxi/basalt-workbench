import { lazy, Suspense } from "react";
import { FileCode } from "lucide-react";
import { useAppStore } from "../lib/store";
import { formatBytes } from "../lib/format";

const CodeEditorView = lazy(() =>
  import("./CodeEditorView").then((module) => ({
    default: module.CodeEditorView,
  })),
);

export function CodeView() {
  const openFile = useAppStore((state) => state.openFile);

  if (!openFile) {
    return (
      <div className="code-placeholder">
        <FileCode size={26} strokeWidth={1.2} />
        <div>从变更面板打开文件</div>
        <div className="mono" style={{ fontSize: 9 }}>
          CTRL+S 保存 · READ-ONLY 预览
        </div>
      </div>
    );
  }

  if (openFile.isBinary) {
    return (
      <div className="code-placeholder">
        <FileCode size={26} strokeWidth={1.2} />
        <div>二进制文件无法以文本方式预览</div>
        <div
          className="mono"
          style={{ fontSize: 9, maxWidth: 360, textAlign: "center" }}
        >
          {openFile.path} · {formatBytes(openFile.size)}
        </div>
      </div>
    );
  }

  return (
    <Suspense
      fallback={
        <div className="code-placeholder">
          <span className="spinner" />
        </div>
      }
    >
      <CodeEditorView />
    </Suspense>
  );
}
