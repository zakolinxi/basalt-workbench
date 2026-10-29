import {
  FolderOpen,
  KeyRound,
  Save,
  ShieldCheck,
  TerminalSquare,
} from "lucide-react";
import clsx from "clsx";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../lib/store";
import { APPROVAL_OPTIONS, approvalMode, approvalPatch } from "../lib/permissions";
import type { Route, Routing, ThemePalette } from "../lib/types";
import {
  Badge,
  Button,
  Field,
  Input,
  Select,
} from "../components/ui";
import { TitlebarDrag } from "../components/WindowControls";

const CHAT_ROUTES: {
  key: keyof Routing;
  label: string;
  description: string;
}[] = [
  {
    key: "chat",
    label: "默认聊天",
    description: "普通对话与快捷问题的默认路由",
  },
  {
    key: "agentPlan",
    label: "Agent 规划",
    description: "用于任务规划（当前执行器使用 Agent 执行模型）",
  },
  {
    key: "agentExecute",
    label: "Agent 执行",
    description: "Agent Runtime 调用工具与执行任务使用的模型",
  },
  {
    key: "review",
    label: "代码审查",
    description: "审查 Diff 与结果时使用的模型",
  },
  {
    key: "vision",
    label: "视觉理解",
    description: "处理图片理解任务的默认模型",
  },
];

const IMAGE_ROUTES: { key: keyof Routing; label: string; description: string }[] = [
  {
    key: "image",
    label: "图像生成",
    description: "对话内生图与图像工作区的默认生成路由",
  },
  {
    key: "imageEdit",
    label: "图像编辑",
    description: "参考图编辑的默认路由；未设置时回退到图像生成路由",
  },
];

const PALETTES: { id: ThemePalette; label: string }[] = [
  { id: "basalt", label: "玄武" },
  { id: "forest", label: "森林" },
  { id: "mint", label: "薄荷" },
  { id: "honey", label: "蜜蜂" },
  { id: "parchment", label: "羊皮纸" },
];

export function SettingsView() {
  const settings = useAppStore((state) => state.settings);
  const providers = useAppStore((state) => state.providers);
  const updateSettings = useAppStore((state) => state.updateSettings);
  const saveSettings = useAppStore((state) => state.saveSettings);
  const setTheme = useAppStore((state) => state.setTheme);
  const setPalette = useAppStore((state) => state.setPalette);
  const cornerStyle = useAppStore((state) => state.cornerStyle);
  const setCornerStyle = useAppStore((state) => state.setCornerStyle);
  const dataDir = useAppStore((state) => state.dataDir);
  const appVersion = useAppStore((state) => state.appVersion);
  const setView = useAppStore((state) => state.setView);
  const notify = useAppStore((state) => state.notify);

  const approval = approvalMode(settings);

  const chatProviders = providers.filter(
    (provider) => provider.category === "chat" && !provider.isTemplate,
  );
  const imageProviders = providers.filter(
    (provider) => provider.category === "image" && !provider.isTemplate,
  );

  const setRoute = (key: keyof Routing, next: Partial<Route>) => {
    updateSettings({
      routing: {
        ...settings.routing,
        [key]: { ...settings.routing[key], ...next },
      },
    });
  };

  const providerFor = (route: Route, category: "chat" | "image") =>
    (category === "chat" ? chatProviders : imageProviders).find(
      (provider) => provider.id === route.providerId,
    );

  const routeRows = (category: "chat" | "image") => {
    const rows = category === "chat" ? CHAT_ROUTES : IMAGE_ROUTES;
    const list = category === "chat" ? chatProviders : imageProviders;
    return rows.map((row) => {
      const route = settings.routing[row.key];
      const provider = providerFor(route, category);
      return (
        <div key={row.key} style={{ display: "flex", flexDirection: "column", gap: 3 }}>
          <div className="route-row">
            <span className="route-label">
              {row.label}
              <div
                style={{
                  fontFamily: "var(--font-ui)",
                  fontSize: 9.5,
                  color: "var(--fg-3)",
                  marginTop: 1,
                }}
              >
                {row.description}
              </div>
            </span>
            <Select
              value={route.providerId}
              onChange={(event) => {
                const nextProvider = list.find(
                  (item) => item.id === event.target.value,
                );
                setRoute(row.key, {
                  providerId: event.target.value,
                  model:
                    (category === "chat"
                      ? nextProvider?.defaultChatModel
                      : nextProvider?.defaultImageModel) ||
                    nextProvider?.models[0]?.id ||
                    "",
                });
              }}
            >
              <option value="">未设置</option>
              {list.map((item) => (
                <option value={item.id} key={item.id}>
                  {item.name}
                  {item.enabled ? "" : "（已禁用）"}
                </option>
              ))}
            </Select>
            <div>
              <Input
                className="mono"
                list={`route-models-${row.key}`}
                value={route.model}
                placeholder="model id"
                onChange={(event) =>
                  setRoute(row.key, { model: event.target.value })
                }
              />
              <datalist id={`route-models-${row.key}`}>
                {provider?.models.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.name}
                  </option>
                ))}
              </datalist>
            </div>
          </div>
        </div>
      );
    });
  };

  return (
    <div className="view">
      <div className="page">
        <header className="page-head">
          <div>
            <div className="page-kicker">APPLICATION SETTINGS</div>
            <h1 className="page-title">设置</h1>
            <div className="page-sub">模型路由 · 审批策略 · 终端与界面</div>
          </div>
          <TitlebarDrag />
          <div className="page-head-actions">
            <Button
              onClick={() =>
                void api.openPath(dataDir).catch((error) =>
                  notify("error", "无法打开数据目录", errorMessage(error)),
                )
              }
            >
              <FolderOpen size={12} /> 数据目录
            </Button>
            <Button variant="primary" onClick={() => void saveSettings()}>
              <Save size={12} /> 保存设置
            </Button>
          </div>
        </header>
        <div className="settings-scroll">
          <div className="settings-inner">
            <section className="settings-section">
              <div className="form-section-head">外观与编辑器</div>
              <div className="body">
                <div className="setting-row">
                  <div className="copy">
                    <div className="title">主题</div>
                    <div className="desc">
                      深色与浅色使用同一套设计 Token；跟随系统会实时响应 Windows 主题。
                    </div>
                  </div>
                  <div className="control">
                    <Select
                      value={settings.theme}
                      onChange={(event) =>
                        void setTheme(
                          event.target.value as "dark" | "light" | "system",
                        )
                      }
                    >
                      <option value="dark">深色</option>
                      <option value="light">浅色</option>
                      <option value="system">跟随系统</option>
                    </Select>
                  </div>
                </div>
                <div className="appearance-palette">
                  <div className="setting-row">
                    <div className="copy">
                      <div className="title">配色</div>
                      <div className="desc">
                        整套界面的背景、表面与强调色都会随配色变化；切换时颜色平滑过渡。
                      </div>
                    </div>
                  </div>
                  <div className="palette-grid">
                    {PALETTES.map((palette) => (
                      <button
                        key={palette.id}
                        type="button"
                        data-palette={palette.id}
                        className={clsx(
                          "palette-card",
                          settings.palette === palette.id && "active",
                        )}
                        aria-pressed={settings.palette === palette.id}
                        onClick={() => void setPalette(palette.id)}
                      >
                        <span className="palette-preview">
                          <span className="palette-window">
                            <span className="palette-accent-bar" />
                            <span className="palette-accent-dot" />
                            <span className="palette-panel">
                              <span className="palette-line" />
                              <span className="palette-line short" />
                            </span>
                          </span>
                        </span>
                        <span className="palette-name">{palette.label}</span>
                      </button>
                    ))}
                  </div>
                </div>
                <div className="setting-row">
                  <div className="copy">
                    <div className="title">界面圆角</div>
                    <div className="desc">
                      立即切换全局直角或圆角风格，偏好会记录在本机，下次启动继续生效。
                    </div>
                  </div>
                  <div className="control">
                    <div className="segmented choice-group">
                      <button
                        type="button"
                        className={cornerStyle === "sharp" ? "active" : ""}
                        onClick={() => setCornerStyle("sharp")}
                      >
                        直角
                      </button>
                      <button
                        type="button"
                        className={cornerStyle === "round" ? "active" : ""}
                        onClick={() => setCornerStyle("round")}
                      >
                        圆角
                      </button>
                    </div>
                  </div>
                </div>
                <div className="setting-row">
                  <div className="copy">
                    <div className="title">终端 Shell</div>
                    <div className="desc">新终端默认使用；已有的终端不会被重启。</div>
                  </div>
                  <div className="control">
                    <Select
                      value={settings.terminalShell}
                      onChange={(event) =>
                        updateSettings({ terminalShell: event.target.value })
                      }
                    >
                      <option value="powershell">PowerShell</option>
                      <option value="cmd">CMD</option>
                      <option value="wsl">WSL（若已安装）</option>
                    </Select>
                  </div>
                </div>
                <div className="setting-row">
                  <div className="copy">
                    <div className="title">编辑器字号</div>
                    <div className="desc">Monaco 编辑器字体大小（10–24 px）。</div>
                  </div>
                  <div className="control">
                    <Input
                      type="number"
                      min={10}
                      max={24}
                      value={settings.editorFontSize}
                      onChange={(event) =>
                        updateSettings({
                          editorFontSize: Number(event.target.value) || 13,
                        })
                      }
                    />
                  </div>
                </div>
                <div className="setting-row">
                  <div className="copy">
                    <div className="title">对话字号</div>
                    <div className="desc">消息正文与输入框字号（11–22 px）。</div>
                  </div>
                  <div className="control">
                    <Input
                      type="number"
                      min={11}
                      max={22}
                      value={settings.chatFontSize}
                      onChange={(event) =>
                        updateSettings({
                          chatFontSize: Number(event.target.value) || 14,
                        })
                      }
                    />
                  </div>
                </div>
              </div>
            </section>

            <section className="settings-section">
              <div className="form-section-head">
                模型路由
                <span className="right">
                  <Badge>{providers.length} PROVIDERS</Badge>
                </span>
              </div>
              <div className="body">
                <div className="field-hint">
                  不同用途可以指向不同模型。路由只在已配置供应商的模型列表中推荐，
                  也允许手动填写自定义模型 ID。未授权的自动升级或收费更高的回退不会发生。
                </div>
                {routeRows("chat")}
                <div className="divider" />
                {routeRows("image")}
                <Button size="sm" onClick={() => setView("providers")}>
                  打开供应商管理
                </Button>
              </div>
            </section>

            <section className="settings-section">
              <div className="form-section-head">Agent 与审批</div>
              <div className="body">
                <div className="setting-row">
                  <div className="copy">
                    <div className="title">Agent 安全上限（轮）</div>
                    <div className="desc">
                      默认 200 轮，仅在超长任务或循环时触发。达到上限后任务会暂停并保留进度，提高上限后点击「继续」接着执行。
                    </div>
                  </div>
                  <div className="control">
                    <Input
                      type="number"
                      min={1}
                      max={1000}
                      value={settings.agentMaxIterations}
                      onChange={(event) =>
                        updateSettings({
                          agentMaxIterations: Number(event.target.value) || 200,
                        })
                      }
                    />
                  </div>
                </div>
                <div className="setting-row">
                  <div className="copy">
                    <div className="title">审批策略</div>
                    <div className="desc">
                      每次询问：写入与命令都暂停；自动审批：文件写入自动执行，命令仍需批准；
                      完全访问：写入与命令都自动执行。危险命令始终被权限策略硬拒绝。
                    </div>
                  </div>
                  <div className="control">
                    <div className="segmented choice-group approval-choice">
                      {APPROVAL_OPTIONS.map((option) => (
                        <button
                          type="button"
                          key={option.value}
                          className={approval === option.value ? "active" : ""}
                          onClick={() => updateSettings(approvalPatch(option.value))}
                          title={option.description}
                        >
                          {option.label}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
                {approval !== "ask" ? (
                  <div className="warning-box">
                    {approval === "auto"
                      ? "自动审批已开启：Agent 的文件写入将直接执行，终端命令仍会暂停等待你批准。"
                      : "完全访问已开启：Agent 的文件写入与终端命令都会自动执行。危险命令仍会被安全策略拒绝，请在受控项目中运行 Agent。"}
                  </div>
                ) : null}
                <div className="field-hint">
                  无论开关状态，路径越界、UNC 网络路径、注册表、提权、格式化与下载执行等
                  高风险命令都在权限策略中被硬拒绝。
                </div>
              </div>
            </section>

            <section className="settings-section">
              <div className="form-section-head">安全与数据</div>
              <div className="body">
                <div className="setting-row">
                  <div className="copy">
                    <div className="title">请求 User-Agent</div>
                    <div className="desc">
                      所有供应商请求使用的 UA；个别网关会拦截非浏览器 UA。留空则回退到内置标识。
                    </div>
                  </div>
                  <div className="control" style={{ width: 360 }}>
                    <Input
                      className="mono"
                      value={settings.userAgent}
                      onChange={(event) =>
                        updateSettings({ userAgent: event.target.value })
                      }
                      placeholder="Mozilla/5.0 (Windows NT 10.0; Win64; x64) …"
                    />
                  </div>
                </div>
                <div className="setting-row">
                  <div className="copy">
                    <div className="title inline-flex">
                      <KeyRound size={12} /> API Key 存储
                    </div>
                    <div className="desc">
                      API Key 写入 Windows 凭据管理器，不进入 SQLite、配置文件或普通日志。
                      普通日志也会过滤疑似令牌字符串。
                    </div>
                  </div>
                  <div className="control">
                    <Badge tone="ok">WINDOWS CREDENTIAL</Badge>
                  </div>
                </div>
                <div className="setting-row">
                  <div className="copy">
                    <div className="title inline-flex">
                      <ShieldCheck size={12} /> 项目边界
                    </div>
                    <div className="desc">
                      Agent 只能通过相对路径访问当前授权的项目目录；路径穿越、符号链接逃逸、
                      .git 写入与项目外绝对路径写入会被阻止。命令中的项目外绝对路径会被拒绝。
                    </div>
                  </div>
                  <div className="control">
                    <Badge tone="ok">ENFORCED</Badge>
                  </div>
                </div>
                <div className="setting-row">
                  <div className="copy">
                    <div className="title inline-flex">
                      <TerminalSquare size={12} /> 进程与审批
                    </div>
                    <div className="desc">
                      Agent 命令带超时、取消与进程树终止；用户终端与 Agent 命令分开记录。
                      命令不通过字符串拼接注入；模型不能直接获得系统 Shell。
                    </div>
                  </div>
                  <div className="control">
                    <Badge tone="ok">POLICY</Badge>
                  </div>
                </div>
                <div
                  className="mono"
                  style={{ fontSize: 10, color: "var(--fg-3)", lineHeight: 1.7 }}
                >
                  <div>DATA DIR · {dataDir}</div>
                  <div>VERSION · {appVersion}</div>
                  <div>UI · React + Tauri 2 · LOCAL-FIRST</div>
                </div>
              </div>
            </section>

            <section className="settings-section">
              <div className="form-section-head">快捷键</div>
              <div className="body about-grid">
                <div>
                  <div className="k">Ctrl + N</div>
                  <div className="v">新建会话</div>
                </div>
                <div>
                  <div className="k">Ctrl + Shift + N</div>
                  <div className="v">新建 Agent 任务</div>
                </div>
                <div>
                  <div className="k">Ctrl + B</div>
                  <div className="v">切换侧栏</div>
                </div>
                <div>
                  <div className="k">Ctrl + `</div>
                  <div className="v">切换终端</div>
                </div>
                <div>
                  <div className="k">Ctrl + S</div>
                  <div className="v">保存当前文件（编辑模式）</div>
                </div>
                <div>
                  <div className="k">Enter / Shift + Enter</div>
                  <div className="v">发送 / 换行</div>
                </div>
              </div>
            </section>

            <div className="row" style={{ justifyContent: "flex-end" }}>
              <Button variant="primary" onClick={() => void saveSettings()}>
                <Save size={12} /> 保存全部设置
              </Button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
