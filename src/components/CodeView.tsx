import { useEffect, useMemo, useRef, useState } from "react";
import Editor from "@monaco-editor/react";
import type { editor as MonacoEditor } from "monaco-editor";
import { KeyCode, KeyMod } from "monaco-editor";
import { Eye, FileCode, RefreshCw, Save } from "lucide-react";
import { useAppStore } from "../lib/store";
import { formatBytes } from "../lib/format";
import { resolveTheme } from "../lib/theme";
import { setupMonaco } from "../lib/monaco";
import { ConfirmDialog, IconButton } from "./ui";

setupMonaco();

function useResolvedTheme() {
  const mode = useAppStore((state) => state.settings.theme);
  const [resolved, setResolved] = useState(resolveTheme(mode));
  useEffect(() => {
    setResolved(resolveTheme(mode));
    if (mode !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const listener = () => setResolved(resolveTheme("system"));
    media.addEventListener("change", listener);
    return () => media.removeEventListener("change", listener);
  }, [mode]);
  return resolved;
}

export function CodeView() {
  const openFile = useAppStore((state) => state.openFile);
  const saveOpenFile = useAppStore((state) => state.saveOpenFile);
  const openFileAt = useAppStore((state) => state.openFileAt);
  const fontSize = useAppStore((state) => state.settings.editorFontSize);
  const resolvedTheme = useResolvedTheme();
  const [content, setContent] = useState("");
  const [editing, setEditing] = useState(false);
  const [confirmReload, setConfirmReload] = useState(false);
  const editorRef = useRef<MonacoEditor.IStandaloneCodeEditor | null>(null);
  const contentRef = useRef("");
  const editingRef = useRef(false);
  const saveRef = useRef<() => void>(() => {});

  contentRef.current = content;
  editingRef.current = editing;

  useEffect(() => {
    setEditing(false);
  }, [openFile?.path]);

  useEffect(() => {
    if (!openFile) {
      setContent("");
      return;
    }
    setContent(openFile.savedContent);
  }, [openFile?.path, openFile?.hash]);

  const dirty = Boolean(openFile && content !== openFile.savedContent);

  const save = async () => {
    if (!openFile || !editingRef.current) return;
    try {
      await saveOpenFile(contentRef.current);
    } catch {
      /* toast handled by store */
    }
  };
  saveRef.current = () => void save();

  const options = useMemo(
    () => ({
      fontFamily:
        '"Cascadia Code", "Cascadia Mono", Consolas, "JetBrains Mono", monospace',
      fontSize,
      lineHeight: Math.round(fontSize * 1.6),
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      smoothScrolling: false,
      cursorBlinking: "smooth" as const,
      renderLineHighlight: "line" as const,
      wordWrap: "off" as const,
      automaticLayout: true,
      tabSize: 2,
      padding: { top: 10, bottom: 10 },
      stickyScroll: { enabled: false },
      overviewRulerLanes: 0,
      hideCursorInOverviewRuler: true,
      lineNumbersMinChars: 4,
      glyphMargin: false,
      folding: true,
      renderWhitespace: "selection" as const,
      scrollbar: { verticalScrollbarSize: 9, horizontalScrollbarSize: 9 },
    }),
    [fontSize],
  );

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
        <div className="mono" style={{ fontSize: 9, maxWidth: 360, textAlign: "center" }}>
          {openFile.path} · {formatBytes(openFile.size)}
        </div>
      </div>
    );
  }

  return (
    <>
      <div className="file-toolbar">
        <span className="path" title={openFile.path}>
          {openFile.path}
        </span>
        {dirty ? <span className="dirty" title="有未保存的修改" /> : null}
        <span className="mono" style={{ fontSize: 9, color: "var(--fg-3)" }}>
          {formatBytes(openFile.size)}
        </span>
        <IconButton
          label={editing ? "切换到只读预览" : "编辑文件"}
          active={editing}
          onClick={() => setEditing((value) => !value)}
        >
          {editing ? <Eye size={13} /> : <FileCode size={13} />}
        </IconButton>
        <IconButton
          label="重新加载文件"
          onClick={() => {
            if (dirty) setConfirmReload(true);
            else void openFileAt(openFile.path);
          }}
        >
          <RefreshCw size={13} />
        </IconButton>
        <IconButton label="保存文件 (Ctrl+S)" disabled={!editing || !dirty} onClick={() => void save()}>
          <Save size={13} />
        </IconButton>
      </div>
      {openFile.truncated ? (
        <div className="code-file-note">文件较大，仅显示前 2MB 内容。</div>
      ) : null}
      <div className="code-host">
        <Editor
          value={content}
          language={openFile.language}
          theme={resolvedTheme === "dark" ? "basalt-dark" : "basalt-light"}
          options={{ ...options, readOnly: !editing }}
          onChange={(value) => setContent(value ?? "")}
          onMount={(editor) => {
            editorRef.current = editor;
            editor.addCommand(KeyMod.CtrlCmd | KeyCode.KeyS, () => {
              saveRef.current();
            });
          }}
        />
      </div>
      <ConfirmDialog
        open={confirmReload}
        title="重新加载文件"
        message="当前存在未保存的修改。重新加载会丢弃这些修改，是否继续？"
        confirmLabel="重新加载"
        danger
        onCancel={() => setConfirmReload(false)}
        onConfirm={() => {
          setConfirmReload(false);
          void openFileAt(openFile.path);
        }}
      />
    </>
  );
}
