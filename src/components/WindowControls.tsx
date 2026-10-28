import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

const appWindow = getCurrentWindow();

export function toggleMaximize() {
  void appWindow
    .toggleMaximize()
    .then(() => window.dispatchEvent(new Event("basalt:window-state")))
    .catch(() => {});
}

export function TitlebarDrag({ className }: { className?: string }) {
  return (
    <div
      className={className ? `titlebar-drag ${className}` : "titlebar-drag"}
      data-tauri-drag-region
      onDoubleClick={toggleMaximize}
    />
  );
}

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const sync = () => {
      void appWindow
        .isMaximized()
        .then((value) => {
          if (!disposed) setMaximized(value);
        })
        .catch(() => {});
    };
    sync();
    const timer = window.setTimeout(sync, 500);
    window.addEventListener("basalt:window-state", sync);
    void appWindow
      .onResized(sync)
      .then((off) => {
        if (disposed) off();
        else unlisten = off;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      window.removeEventListener("basalt:window-state", sync);
      unlisten?.();
    };
  }, []);

  return (
    <div className="window-controls">
      <button
        type="button"
        className="window-control"
        onClick={() => void appWindow.minimize()}
        title="最小化"
        aria-label="最小化"
      >
        <Minus size={13} />
      </button>
      <button
        type="button"
        className="window-control"
        onClick={toggleMaximize}
        title={maximized ? "还原" : "最大化"}
        aria-label={maximized ? "还原" : "最大化"}
      >
        {maximized ? <Copy size={11} /> : <Square size={9} />}
      </button>
      <button
        type="button"
        className="window-control close"
        onClick={() => void appWindow.close()}
        title="关闭"
        aria-label="关闭"
      >
        <X size={14} />
      </button>
    </div>
  );
}
