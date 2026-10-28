import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Rail } from "./components/Rail";
import { Toasts } from "./components/ui";
import { useAppStore } from "./lib/store";
import { applyCornerStyle, applyTheme, watchSystemTheme } from "./lib/theme";
import type {
  ChatEvent,
  ImageEvent,
  TaskEventEnvelope,
} from "./lib/types";
import { Workbench } from "./views/Workbench";
import { ProvidersView } from "./views/ProvidersView";
import { ImageStudio } from "./views/ImageStudio";
import { SettingsView } from "./views/SettingsView";

export default function App() {
  const booted = useAppStore((state) => state.booted);
  const bootError = useAppStore((state) => state.bootError);
  const boot = useAppStore((state) => state.boot);
  const view = useAppStore((state) => state.view);
  const theme = useAppStore((state) => state.settings.theme);
  const cornerStyle = useAppStore((state) => state.cornerStyle);
  const chatFontSize = useAppStore((state) => state.settings.chatFontSize);
  const layout = useAppStore((state) => state.layout);
  const setLayout = useAppStore((state) => state.setLayout);
  const [retrying, setRetrying] = useState(false);

  useEffect(() => {
    void boot();
  }, [boot]);

  useEffect(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];
    const setup = async () => {
      const listeners = await Promise.all([
        listen<ChatEvent>("basalt:chat-event", (event) =>
          useAppStore.getState().onChatEvent(event.payload),
        ),
        listen<TaskEventEnvelope>("basalt:task-event", (event) =>
          useAppStore.getState().onTaskEvent(event.payload),
        ),
        listen<ImageEvent>("basalt:image-event", (event) =>
          useAppStore.getState().onImageEvent(event.payload),
        ),
        listen<{ terminalId: string; exitCode?: number | null }>(
          "basalt:terminal-exit",
          (event) =>
            useAppStore.getState().onTerminalExit(event.payload.terminalId),
        ),
      ]);
      if (disposed) {
        listeners.forEach((unlisten) => unlisten());
      } else {
        unlisteners.push(...listeners);
      }
    };
    void setup();
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    applyTheme(theme);
    if (theme !== "system") return;
    return watchSystemTheme(() => applyTheme("system"));
  }, [theme]);

  useEffect(() => {
    applyCornerStyle(cornerStyle);
  }, [cornerStyle]);

  useEffect(() => {
    document.documentElement.style.setProperty(
      "--chat-font-size",
      `${chatFontSize}px`,
    );
  }, [chatFontSize]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key === "`") {
        event.preventDefault();
        setLayout({ terminalCollapsed: !layout.terminalCollapsed });
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [layout.terminalCollapsed, setLayout]);

  if (!booted) {
    return (
      <div className="boot-screen">
        <div className="boot-logo">
          <span />
          <span />
          <span />
        </div>
        <div>
          <div className="boot-title">BASALT</div>
          <div className="boot-sub">AI ENGINEERING WORKBENCH</div>
        </div>
        <div className="row" style={{ color: "var(--fg-3)", fontSize: 11 }}>
          <span className="spinner" /> 正在初始化工作台…
        </div>
      </div>
    );
  }

  if (bootError) {
    return (
      <div className="boot-screen">
        <div className="boot-logo">
          <span />
          <span />
          <span />
        </div>
        <div>
          <div className="boot-title">BASALT</div>
          <div className="boot-sub">STARTUP FAILED</div>
        </div>
        <div className="boot-error">{bootError}</div>
        <button
          className="btn primary"
          onClick={() => {
            setRetrying(true);
            location.reload();
          }}
          disabled={retrying}
        >
          重新加载
        </button>
      </div>
    );
  }

  return (
    <div className="app-shell">
      <Rail />
      <div className="app-body">
        <div
          style={{
            display: view === "workbench" ? "flex" : "none",
            flex: 1,
            minWidth: 0,
            minHeight: 0,
            flexDirection: "column",
          }}
        >
          <Workbench />
        </div>
        {view === "providers" ? <ProvidersView /> : null}
        {view === "images" ? <ImageStudio /> : null}
        {view === "settings" ? <SettingsView /> : null}
      </div>
      <Toasts />
    </div>
  );
}
