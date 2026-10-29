import { LayoutGrid, Server, Image as ImageIcon, Settings, Sun, Moon, Laptop } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useAppStore } from "../lib/store";
import { resolveTheme } from "../lib/theme";
import type { ThemeMode } from "../lib/types";

const GITHUB_URL = "https://github.com/zakolinxi/basalt-workbench";

const navItems = [
  { view: "workbench" as const, label: "工作台", icon: LayoutGrid },
  { view: "providers" as const, label: "供应商", icon: Server },
  { view: "images" as const, label: "图像工作区", icon: ImageIcon },
];

export function Rail() {
  const view = useAppStore((state) => state.view);
  const setView = useAppStore((state) => state.setView);
  const notify = useAppStore((state) => state.notify);
  const theme = useAppStore((state) => state.settings.theme);
  const setTheme = useAppStore((state) => state.setTheme);
  const pending = useAppStore((state) => state.pendingApprovals.length);
  const running = useAppStore((state) =>
    state.tasks.some(
      (task) => task.status === "running" || task.status === "waitingApproval",
    ),
  );
  const [resolved, setResolved] = useState(resolveTheme(theme));

  useEffect(() => {
    setResolved(resolveTheme(theme));
    if (theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const listener = () => setResolved(resolveTheme("system"));
    media.addEventListener("change", listener);
    return () => media.removeEventListener("change", listener);
  }, [theme]);

  const nextTheme: Record<ThemeMode, ThemeMode> = {
    dark: "light",
    light: "system",
    system: "dark",
  };

  return (
    <nav className="rail">
      <button
        className="rail-brand"
        onClick={() => setView("workbench")}
        title="BASALT · AI Engineering Workbench"
      >
        <span className="brand-mark">
          <span />
          <span />
          <span />
        </span>
      </button>
      <div className="rail-nav">
        {navItems.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.view}
              className={`rail-btn ${view === item.view ? "active" : ""}`}
              title={item.label}
              aria-label={item.label}
              onClick={() => setView(item.view)}
            >
              <Icon size={17} strokeWidth={1.6} />
              {item.view === "workbench" && (pending > 0 || running) ? (
                <span
                  className="badge-dot"
                  style={pending > 0 ? { background: "var(--warn)" } : undefined}
                />
              ) : null}
            </button>
          );
        })}
      </div>
      <div className="rail-spacer" />
      <button
        type="button"
        className="rail-btn rail-github"
        aria-label="GitHub 仓库"
        onClick={() => {
          void openUrl(GITHUB_URL).catch((error) =>
            notify("error", "打开 GitHub 失败", String(error)),
          );
        }}
      >
        <svg
          viewBox="0 0 24 24"
          width="17"
          height="17"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22" />
        </svg>
        <span className="rail-tooltip">GitHub</span>
      </button>
      <button
        className="rail-btn"
        title={`主题：${theme === "dark" ? "深色" : theme === "light" ? "浅色" : "跟随系统"}`}
        aria-label="切换主题"
        onClick={() => void setTheme(nextTheme[theme])}
      >
        {resolved === "dark" ? (
          <Moon size={16} strokeWidth={1.6} />
        ) : (
          <Sun size={16} strokeWidth={1.6} />
        )}
        {theme === "system" ? (
          <span
            style={{
              position: "absolute",
              bottom: 3,
              right: 5,
              fontSize: 7,
              color: "var(--fg-3)",
            }}
          >
            <Laptop size={8} />
          </span>
        ) : null}
      </button>
      <button
        className={`rail-btn ${view === "settings" ? "active" : ""}`}
        title="设置"
        aria-label="设置"
        onClick={() => setView("settings")}
      >
        <Settings size={17} strokeWidth={1.6} />
      </button>
    </nav>
  );
}
