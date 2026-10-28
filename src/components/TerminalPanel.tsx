import { useEffect, useRef, useState, type CSSProperties } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import {
  ChevronDown,
  ChevronsDownUp,
  Copy,
  Plus,
  Search,
  Square,
  TerminalSquare,
  Trash2,
  X,
} from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../lib/store";
import type { TerminalEvent, TerminalTab } from "../lib/types";
import { IconButton, useClickOutside } from "./ui";

interface TerminalHandle {
  search: (query: string, previous?: boolean) => void;
  clear: () => void;
  copy: () => Promise<void>;
  fit: () => void;
  focus: () => void;
}

const DARK_THEME = {
  background: "#09090a",
  foreground: "#c6c6cc",
  cursor: "#ececef",
  cursorAccent: "#09090a",
  selectionBackground: "#ffffff26",
  black: "#1b1b1f",
  red: "#f07171",
  green: "#54d68a",
  yellow: "#e8b94a",
  blue: "#85a8f5",
  magenta: "#c49bf0",
  cyan: "#6fd3d3",
  white: "#d8d8dc",
  brightBlack: "#5e5e66",
  brightRed: "#ff8a8a",
  brightGreen: "#7ee2a8",
  brightYellow: "#f5cc6b",
  brightBlue: "#9dbcf8",
  brightMagenta: "#d4b3f5",
  brightCyan: "#8fe0e0",
  brightWhite: "#f0f0f2",
};

function TerminalInstance({
  tab,
  active,
  onReady,
}: {
  tab: TerminalTab;
  active: boolean;
  onReady: (id: string, handle: TerminalHandle | null) => void;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);

  useEffect(() => {
    if (!hostRef.current) return;
    let disposed = false;
    const term = new Terminal({
      fontFamily: '"Cascadia Code", "Cascadia Mono", Consolas, monospace',
      fontSize: 12.5,
      lineHeight: 1.3,
      cursorBlink: true,
      cursorStyle: "bar",
      scrollback: 12000,
      allowProposedApi: true,
      theme: DARK_THEME,
      convertEol: false,
    });
    const fit = new FitAddon();
    const search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(search);
    term.open(hostRef.current);
    termRef.current = term;
    fitRef.current = fit;
    searchRef.current = search;

    const applyFit = () => {
      try {
        fit.fit();
        const dims = fit.proposeDimensions();
        if (dims && Number.isFinite(dims.cols) && Number.isFinite(dims.rows)) {
          void api
            .terminalResize(tab.id, Math.max(2, dims.cols), Math.max(1, dims.rows))
            .catch(() => {});
        }
      } catch {
        /* hidden or not mounted yet */
      }
    };
    requestAnimationFrame(applyFit);

    const observer = new ResizeObserver(() => {
      if (!disposed) applyFit();
    });
    observer.observe(hostRef.current);
    window.addEventListener("resize", applyFit);

    const dataDisposable = term.onData((data) => {
      void api.terminalWrite(tab.id, data).catch(() => {});
    });

    const outputListener = listen<TerminalEvent>("basalt:terminal-output", (event) => {
      if (event.payload.terminalId === tab.id) {
        term.write(event.payload.data);
      }
    });
    const exitListener = listen<{ terminalId: string; exitCode?: number | null }>(
      "basalt:terminal-exit",
      (event) => {
        if (event.payload.terminalId === tab.id) {
          const code =
            event.payload.exitCode === null || event.payload.exitCode === undefined
              ? ""
              : ` · exit ${event.payload.exitCode}`;
          term.write(`\r\n\x1b[2m[进程已退出${code}]\x1b[0m\r\n`);
          useAppStore.getState().markTerminalExit(tab.id);
        }
      },
    );
    let outputUnlisten: UnlistenFn | null = null;
    let exitUnlisten: UnlistenFn | null = null;
    void outputListener.then((fn) => {
      if (disposed) fn();
      else outputUnlisten = fn;
    });
    void exitListener.then((fn) => {
      if (disposed) fn();
      else exitUnlisten = fn;
    });

    const handle: TerminalHandle = {
      search: (query, previous) => {
        if (!query) return;
        const options = { incremental: true, caseSensitive: false };
        if (previous) search.findPrevious(query, options);
        else search.findNext(query, options);
      },
      clear: () => {
        term.clear();
        term.write("\x1b[2J\x1b[H");
      },
      copy: async () => {
        let text = term.getSelection();
        if (!text) {
          term.selectAll();
          text = term.getSelection();
          term.clearSelection();
        }
        if (!text) return;
        try {
          await navigator.clipboard.writeText(text);
        } catch {
          const area = document.createElement("textarea");
          area.value = text;
          document.body.appendChild(area);
          area.select();
          document.execCommand("copy");
          area.remove();
        }
      },
      fit: applyFit,
      focus: () => term.focus(),
    };
    onReady(tab.id, handle);

    const redraw = window.setTimeout(() => {
      if (!disposed) {
        void api.terminalWrite(tab.id, "\r").catch(() => {});
        if (active) term.focus();
      }
    }, 350);

    if (active) term.focus();

    return () => {
      disposed = true;
      window.clearTimeout(redraw);
      observer.disconnect();
      window.removeEventListener("resize", applyFit);
      dataDisposable.dispose();
      outputUnlisten?.();
      exitUnlisten?.();
      onReady(tab.id, null);
      term.dispose();
      termRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab.id]);

  useEffect(() => {
    if (active) {
      requestAnimationFrame(() => {
        fitRef.current?.fit();
        termRef.current?.focus();
      });
    }
  }, [active]);

  return <div ref={hostRef} className={`terminal-instance ${active ? "" : "hidden"}`} />;
}

export function TerminalPanel({ style }: { style?: CSSProperties }) {
  const terminals = useAppStore((state) => state.terminals);
  const activeTerminalId = useAppStore((state) => state.activeTerminalId);
  const newTerminal = useAppStore((state) => state.newTerminal);
  const closeTerminal = useAppStore((state) => state.closeTerminal);
  const setActiveTerminal = useAppStore((state) => state.setActiveTerminal);
  const markTerminalExit = useAppStore((state) => state.markTerminalExit);
  const setLayout = useAppStore((state) => state.setLayout);
  const notify = useAppStore((state) => state.notify);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchText, setSearchText] = useState("");
  const [shellMenu, setShellMenu] = useState(false);
  const menuRef = useClickOutside<HTMLDivElement>(() => setShellMenu(false), shellMenu);
  const handles = useRef(new Map<string, TerminalHandle>());

  const activeHandle = activeTerminalId
    ? handles.current.get(activeTerminalId)
    : undefined;

  const create = (shell?: string) => {
    setShellMenu(false);
    void newTerminal(shell);
  };

  return (
    <section className="terminal-panel" style={style}>
      <div className="terminal-head">
        {terminals.map((tab) => (
          <button
            key={tab.id}
            className={`term-tab ${tab.id === activeTerminalId ? "active" : ""}`}
            onClick={() => setActiveTerminal(tab.id)}
          >
            <span className={tab.alive ? "live" : "dead"} />
            {tab.title}
            <span
              className="x"
              role="button"
              tabIndex={-1}
              title="关闭终端"
              onClick={(event) => {
                event.stopPropagation();
                void closeTerminal(tab.id);
              }}
            >
              <X size={10} />
            </span>
          </button>
        ))}
        <div ref={menuRef} style={{ position: "relative", display: "flex", alignItems: "center" }}>
          <button
            className="icon-btn sm"
            title="新建终端"
            onClick={() => create()}
          >
            <Plus size={13} />
          </button>
          <button
            className="icon-btn sm"
            title="选择终端类型"
            onClick={() => setShellMenu((open) => !open)}
          >
            <ChevronDown size={11} />
          </button>
          {shellMenu ? (
            <div className="menu" style={{ bottom: "calc(100% + 4px)", left: 0 }}>
              <div className="menu-label">新终端</div>
              <button className="menu-item" onClick={() => create("powershell")}>
                <TerminalSquare size={12} /> PowerShell
              </button>
              <button className="menu-item" onClick={() => create("cmd")}>
                <TerminalSquare size={12} /> CMD
              </button>
              <button className="menu-item" onClick={() => create("wsl")}>
                <TerminalSquare size={12} /> WSL
              </button>
            </div>
          ) : null}
        </div>
        <div className="terminal-tools">
          {searchOpen ? (
            <input
              className="input terminal-search"
              placeholder="搜索终端输出"
              autoFocus
              value={searchText}
              onChange={(event) => {
                setSearchText(event.target.value);
                activeHandle?.search(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  activeHandle?.search(searchText, event.shiftKey);
                } else if (event.key === "Escape") {
                  setSearchOpen(false);
                }
              }}
            />
          ) : null}
          <IconButton
            size="sm"
            label="搜索输出"
            active={searchOpen}
            onClick={() => setSearchOpen((open) => !open)}
          >
            <Search size={12} />
          </IconButton>
          <IconButton
            size="sm"
            label="复制选中内容"
            onClick={() => {
              activeHandle?.copy().then(
                () => notify("info", "已复制终端内容"),
                () => notify("warning", "没有可复制的选中内容"),
              );
            }}
          >
            <Copy size={12} />
          </IconButton>
          <IconButton
            size="sm"
            label="清屏"
            onClick={() => activeHandle?.clear()}
          >
            <Trash2 size={12} />
          </IconButton>
          <IconButton
            size="sm"
            label="终止当前进程"
            onClick={async () => {
              if (!activeTerminalId) return;
              try {
                await api.terminalKill(activeTerminalId);
                markTerminalExit(activeTerminalId);
                notify("info", "终端进程已终止");
              } catch (error) {
                notify("error", "终止进程失败", errorMessage(error));
              }
            }}
            disabled={!activeTerminalId}
          >
            <Square size={11} />
          </IconButton>
          <IconButton
            size="sm"
            label="收起终端"
            onClick={() => setLayout({ terminalCollapsed: true })}
          >
            <ChevronsDownUp size={12} />
          </IconButton>
        </div>
      </div>
      {terminals.length === 0 ? (
        <div className="terminal-no-tabs">
          <TerminalSquare size={15} />
          没有打开的终端
          <button className="btn xs" onClick={() => create()}>
            新建 PowerShell
          </button>
          <span className="muted" style={{ textTransform: "none", letterSpacing: 0 }}>
            Agent 执行的命令会记录在任务时间线中
          </span>
        </div>
      ) : (
        <div className="terminal-body">
          {terminals.map((tab) => (
            <TerminalInstance
              key={tab.id}
              tab={tab}
              active={tab.id === activeTerminalId}
              onReady={(id, handle) => {
                if (handle) handles.current.set(id, handle);
                else handles.current.delete(id);
              }}
            />
          ))}
        </div>
      )}
    </section>
  );
}
