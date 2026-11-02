import { useCallback, useEffect, useMemo, useState } from "react";
import { isRemote } from "../lib/bridge";
import { pickPaths } from "../lib/platform";
import { requestDirectory } from "../components/DirectoryPicker";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  FolderOpen,
  Loader2,
  Package,
  Play,
  Plug,
  Plus,
  Puzzle,
  RefreshCw,
  Server,
  ShieldCheck,
  Trash2,
  Wrench,
} from "lucide-react";
import clsx from "clsx";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../lib/store";
import type {
  InstalledPlugin,
  McpServer,
  McpServerInput,
  McpTool,
  Skill,
  ToolInfo,
} from "../lib/types";
import {
  Badge,
  Button,
  ConfirmDialog,
  Field,
  IconButton,
  Input,
  Modal,
  Select,
  Switch,
  Textarea,
} from "../components/ui";
import { TitlebarDrag } from "../components/WindowControls";

type Tab = "skills" | "mcp" | "plugins" | "registry";

/** Native folder dialog on the desktop, desktop-side browser on the phone. */
async function chooseFolder(title: string): Promise<string | null> {
  if (isRemote()) return requestDirectory(title);
  const [path] = await pickPaths({ directory: true, multiple: false, title });
  return path ?? null;
}

const TABS: { id: Tab; label: string; icon: typeof Puzzle }[] = [
  { id: "skills", label: "Skills", icon: Puzzle },
  { id: "mcp", label: "MCP", icon: Plug },
  { id: "plugins", label: "插件包", icon: Package },
  { id: "registry", label: "工具注册表", icon: Wrench },
];

const SKILL_SAMPLE = `---
name: release-checklist
description: 发布 Windows 桌面应用前检查版本、构建、产物与说明。
---

# 发布检查

1. 运行完整测试。
2. 构建安装包。
3. 核对版本号与产物哈希。
`;

const PLUGIN_SAMPLE = `{
  "name": "Docs Toolkit",
  "version": "1.0.0",
  "description": "文档检索与发布工作流",
  "skills": ["skills/release-checklist"],
  "mcpServers": {
    "docs": {
      "transport": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-memory"],
      "env": {},
      "enabled": true,
      "autoApprove": false
    }
  }
}`;

interface McpDraft {
  id?: string;
  name: string;
  transport: "stdio" | "http";
  command: string;
  argsText: string;
  envText: string;
  url: string;
  headersText: string;
  enabled: boolean;
  autoApprove: boolean;
}

function emptyDraft(): McpDraft {
  return {
    name: "",
    transport: "stdio",
    command: "",
    argsText: "",
    envText: "{}",
    url: "",
    headersText: "{}",
    enabled: true,
    autoApprove: false,
  };
}

function draftFromServer(server: McpServer): McpDraft {
  return {
    id: server.id,
    name: server.name,
    transport: server.transport === "http" ? "http" : "stdio",
    command: server.command,
    argsText: server.args.join("\n"),
    envText: JSON.stringify(server.env ?? {}, null, 2),
    url: server.url,
    headersText: JSON.stringify(server.headers ?? {}, null, 2),
    enabled: server.enabled,
    autoApprove: server.autoApprove,
  };
}

function parseObject(text: string, label: string): Record<string, string> {
  const raw = text.trim();
  if (!raw) return {};
  const parsed = JSON.parse(raw) as unknown;
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error(`${label}必须是 JSON 对象`);
  }
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
    if (typeof value !== "string") {
      throw new Error(`${label}的值必须全部是字符串`);
    }
    out[key] = value;
  }
  return out;
}

function sourceTone(source: string): "ok" | "warn" | "info" | undefined {
  if (source === "builtin") return "info";
  if (source === "skill") return "warn";
  if (source === "mcp") return "ok";
  return undefined;
}

function mcpStatusTone(status: string): "ok" | "warn" | "err" | undefined {
  if (status === "ready") return "ok";
  if (status === "connecting") return "warn";
  if (status === "error") return "err";
  return undefined;
}

function schemaSkeleton(schema: Record<string, unknown>): Record<string, unknown> {
  const properties = schema?.properties;
  if (!properties || typeof properties !== "object" || Array.isArray(properties)) {
    return {};
  }
  const defaults = (value: unknown): unknown => {
    if (!value || typeof value !== "object") return null;
    const type = (value as Record<string, unknown>).type;
    switch (type) {
      case "string":
        return "";
      case "number":
      case "integer":
        return 0;
      case "boolean":
        return false;
      case "array":
        return [];
      case "object":
        return {};
      default:
        return null;
    }
  };
  return Object.fromEntries(
    Object.entries(properties as Record<string, unknown>).map(([key, value]) => [
      key,
      defaults(value),
    ]),
  );
}

export function ExtensionsView() {
  const notify = useAppStore((state) => state.notify);
  const dataDir = useAppStore((state) => state.dataDir);
  const setView = useAppStore((state) => state.setView);
  const [tab, setTab] = useState<Tab>("skills");
  const [skills, setSkills] = useState<Skill[]>([]);
  const [servers, setServers] = useState<McpServer[]>([]);
  const [plugins, setPlugins] = useState<InstalledPlugin[]>([]);
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [toolsByServer, setToolsByServer] = useState<Record<string, McpTool[]>>({});
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [draft, setDraft] = useState<McpDraft | null>(null);
  const [deleteSkill, setDeleteSkill] = useState<Skill | null>(null);
  const [deleteServer, setDeleteServer] = useState<McpServer | null>(null);
  const [deletePlugin, setDeletePlugin] = useState<InstalledPlugin | null>(null);
  const [toolTest, setToolTest] = useState<{
    server: McpServer;
    tool: McpTool;
  } | null>(null);
  const [testArgs, setTestArgs] = useState("{}");
  const [testResult, setTestResult] = useState<string | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);

  const loadAll = useCallback(async () => {
    setLoading(true);
    try {
      const [nextSkills, nextServers, nextPlugins, nextTools] = await Promise.all([
        api.skillList(),
        api.mcpServerList(),
        api.pluginList(),
        api.toolRegistry(),
      ]);
      setSkills(nextSkills);
      setServers(nextServers);
      setPlugins(nextPlugins);
      setTools(nextTools);
    } catch (error) {
      notify("error", "无法加载扩展", errorMessage(error));
    } finally {
      setLoading(false);
    }
  }, [notify]);

  useEffect(() => {
    void loadAll();
  }, [loadAll]);

  const counts = useMemo(
    () => ({
      skills: skills.filter((skill) => skill.enabled).length,
      servers: servers.filter((server) => server.enabled).length,
      tools: tools.length,
    }),
    [skills, servers, tools],
  );

  const installSkill = async () => {
    try {
      const selection = await chooseFolder("选择包含 SKILL.md 的文件夹");
      if (!selection) return;
      setBusy("install-skill");
      const skill = await api.skillInstall(selection);
      notify("success", "Skill 已安装", skill.name);
      await loadAll();
      setTab("skills");
    } catch (error) {
      notify("error", "安装 Skill 失败", errorMessage(error));
    } finally {
      setBusy(null);
    }
  };

  const installPlugin = async () => {
    try {
      const selection = await chooseFolder("选择包含 plugin.json 的插件文件夹");
      if (!selection) return;
      setBusy("install-plugin");
      const plugin = await api.pluginInstall(selection);
      notify("success", "插件已安装", plugin.name);
      await loadAll();
      setTab("plugins");
    } catch (error) {
      notify("error", "安装插件失败", errorMessage(error));
    } finally {
      setBusy(null);
    }
  };

  const toggleSkill = async (skill: Skill) => {
    setBusy(skill.id);
    try {
      const next = await api.skillSetEnabled(skill.id, !skill.enabled);
      setSkills((current) =>
        current.map((item) => (item.id === next.id ? next : item)),
      );
      setTools(await api.toolRegistry());
    } catch (error) {
      notify("error", "更新 Skill 失败", errorMessage(error));
    } finally {
      setBusy(null);
    }
  };

  const removeSkill = async (skill: Skill) => {
    setBusy(skill.id);
    try {
      await api.skillDelete(skill.id);
      setDeleteSkill(null);
      await loadAll();
    } catch (error) {
      notify("error", "删除 Skill 失败", errorMessage(error));
    } finally {
      setBusy(null);
    }
  };

  const refreshServer = async (server: McpServer) => {
    setBusy(server.id);
    try {
      const list = await api.mcpServerRefresh(server.id);
      setToolsByServer((current) => ({ ...current, [server.id]: list }));
      setExpanded((current) => ({ ...current, [server.id]: true }));
      setServers(await api.mcpServerList());
      setTools(await api.toolRegistry());
      notify("success", "MCP 工具已刷新", `${server.name} · ${list.length} 个工具`);
    } catch (error) {
      notify("error", "连接 MCP 失败", errorMessage(error));
      setServers(await api.mcpServerList().catch(() => servers));
    } finally {
      setBusy(null);
    }
  };

  const toggleServer = async (server: McpServer) => {
    setBusy(server.id);
    try {
      const next = await api.mcpServerSetEnabled(server.id, !server.enabled);
      setServers((current) =>
        current.map((item) => (item.id === next.id ? next : item)),
      );
      setTools(await api.toolRegistry());
    } catch (error) {
      notify("error", "更新 MCP 服务器失败", errorMessage(error));
    } finally {
      setBusy(null);
    }
  };

  const removeServer = async (server: McpServer) => {
    setBusy(server.id);
    try {
      await api.mcpServerDelete(server.id);
      setDeleteServer(null);
      await loadAll();
    } catch (error) {
      notify("error", "删除 MCP 服务器失败", errorMessage(error));
    } finally {
      setBusy(null);
    }
  };

  const removePlugin = async (plugin: InstalledPlugin) => {
    setBusy(plugin.id);
    try {
      await api.pluginUninstall(plugin.id);
      setDeletePlugin(null);
      await loadAll();
    } catch (error) {
      notify("error", "卸载插件失败", errorMessage(error));
    } finally {
      setBusy(null);
    }
  };

  const openToolTest = (server: McpServer, tool: McpTool) => {
    setToolTest({ server, tool });
    setTestArgs(JSON.stringify(schemaSkeleton(tool.inputSchema), null, 2));
    setTestResult(null);
    setTestError(null);
  };

  const runToolTest = async () => {
    if (!toolTest || testing) return;
    let args: unknown;
    try {
      args = JSON.parse(testArgs);
      if (!args || typeof args !== "object" || Array.isArray(args)) {
        throw new Error("工具参数必须是 JSON 对象");
      }
    } catch (error) {
      setTestError(errorMessage(error));
      return;
    }
    setTesting(true);
    setTestError(null);
    try {
      const result = await api.mcpToolCall(
        toolTest.server.id,
        toolTest.tool.toolName,
        args as Record<string, unknown>,
      );
      setTestResult(JSON.stringify(result, null, 2));
    } catch (error) {
      setTestError(errorMessage(error));
    } finally {
      setTesting(false);
    }
  };

  const toggleExpand = async (server: McpServer) => {
    const next = !expanded[server.id];
    setExpanded((current) => ({ ...current, [server.id]: next }));
    if (!next || toolsByServer[server.id]) return;
    try {
      const list = await api.mcpToolList(server.id);
      setToolsByServer((current) => ({ ...current, [server.id]: list }));
    } catch (error) {
      notify("error", "读取 MCP 工具失败", errorMessage(error));
    }
  };

  const saveDraft = async () => {
    if (!draft) return;
    if (!draft.name.trim()) {
      notify("warning", "请填写 MCP 服务器名称");
      return;
    }
    setBusy("save-mcp");
    try {
      const input: McpServerInput = {
        id: draft.id ?? null,
        name: draft.name.trim(),
        transport: draft.transport,
        command: draft.command.trim(),
        args: draft.argsText
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean),
        env: parseObject(draft.envText, "环境变量"),
        url: draft.url.trim(),
        headers: parseObject(draft.headersText, "请求头"),
        enabled: draft.enabled,
        autoApprove: draft.autoApprove,
      };
      await api.mcpServerSave(input);
      setDraft(null);
      await loadAll();
      notify("success", "MCP 服务器已保存", "点击“刷新工具”建立连接并加载工具");
    } catch (error) {
      notify("error", "保存 MCP 服务器失败", errorMessage(error));
    } finally {
      setBusy(null);
    }
  };

  const groupedTools = useMemo(() => {
    const groups: Record<string, ToolInfo[]> = { builtin: [], skill: [], mcp: [] };
    for (const tool of tools) {
      (groups[tool.source] ??= []).push(tool);
    }
    return groups;
  }, [tools]);

  return (
    <div className="view">
      <div className="page">
        <header className="page-head">
          <div>
            <div className="page-kicker">EXTENSIONS · SKILLS · MCP</div>
            <h1 className="page-title">扩展</h1>
            <div className="page-sub">
              统一工具注册表 · Skill 工作流 · MCP 连接与权限
            </div>
          </div>
          <TitlebarDrag />
          <div className="page-head-actions">
            <Button
              onClick={() =>
                void api
                  .openPath(dataDir)
                  .catch((error) =>
                    notify("error", "无法打开数据目录", errorMessage(error)),
                  )
              }
            >
              <FolderOpen size={12} /> 扩展目录
            </Button>
            <Button onClick={() => void loadAll()} disabled={loading}>
              <RefreshCw size={12} className={loading ? "spin" : undefined} /> 刷新
            </Button>
            {tab === "skills" ? (
              <Button
                variant="primary"
                onClick={() => void installSkill()}
                disabled={busy === "install-skill"}
              >
                <Plus size={12} /> 安装 Skill
              </Button>
            ) : null}
            {tab === "mcp" ? (
              <Button variant="primary" onClick={() => setDraft(emptyDraft())}>
                <Plus size={12} /> 添加服务器
              </Button>
            ) : null}
            {tab === "plugins" ? (
              <Button
                variant="primary"
                onClick={() => void installPlugin()}
                disabled={busy === "install-plugin"}
              >
                <Plus size={12} /> 安装插件
              </Button>
            ) : null}
          </div>
        </header>
        <div className="settings-scroll">
          <div className="settings-inner extensions-inner">
            <div className="ext-tabs">
              {TABS.map((item) => {
                const Icon = item.icon;
                const count =
                  item.id === "skills"
                    ? counts.skills
                    : item.id === "mcp"
                      ? counts.servers
                      : item.id === "plugins"
                        ? plugins.length
                        : counts.tools;
                return (
                  <button
                    key={item.id}
                    type="button"
                    className={clsx("ext-tab", tab === item.id && "active")}
                    onClick={() => setTab(item.id)}
                  >
                    <Icon size={13} />
                    {item.label}
                    <span className="ext-tab-count">{count}</span>
                  </button>
                );
              })}
            </div>

            {tab === "skills" ? (
              <section className="settings-section">
                <div className="form-section-head">
                  SKILLS
                  <span className="right">
                    <Badge>{counts.skills} ENABLED</Badge>
                  </span>
                </div>
                <div className="body">
                  <div className="field-hint">
                    Skill 是可复用的工作流说明包，包含 SKILL.md 以及可选脚本和模板。
                    启用后 Agent 会在任务匹配时先读取说明，再按其中的步骤执行。
                  </div>
                  {skills.length === 0 ? (
                    <div className="ext-empty">
                      <Puzzle size={20} />
                      <div className="ext-empty-title">还没有安装 Skill</div>
                      <div className="ext-empty-copy">
                        选择一个包含 SKILL.md 的文件夹导入。你可以从社区下载 Skill，
                        也可以为自己的项目编写发布、测试或文档流程。
                      </div>
                      <pre className="ext-code">{SKILL_SAMPLE}</pre>
                    </div>
                  ) : (
                    <div className="ext-grid">
                      {skills.map((skill) => (
                        <div className="ext-card" key={skill.id}>
                          <div className="ext-card-head">
                            <div className="ext-card-title">
                              <Puzzle size={13} />
                              <span className="name">{skill.name}</span>
                            </div>
                            <div className="ext-card-actions">
                              <Switch
                                checked={skill.enabled}
                                disabled={busy === skill.id}
                                onChange={() => void toggleSkill(skill)}
                              />
                              <IconButton
                                label="打开 Skill 目录"
                                size="sm"
                                onClick={() =>
                                  void api.openPath(
                                    skill.path.replace(/[\\/][^\\/]+$/, ""),
                                  )
                                }
                              >
                                <FolderOpen size={11} />
                              </IconButton>
                              <IconButton
                                label="删除 Skill"
                                size="sm"
                                onClick={() => setDeleteSkill(skill)}
                              >
                                <Trash2 size={11} />
                              </IconButton>
                            </div>
                          </div>
                          <div className="ext-card-meta">
                            <Badge tone={skill.enabled ? "ok" : undefined}>
                              {skill.enabled ? "已启用" : "已停用"}
                            </Badge>
                            {skill.pluginId ? <Badge>PLUGIN</Badge> : null}
                            <span className="path mono">{skill.path}</span>
                          </div>
                          <div className="ext-desc">{skill.description}</div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </section>
            ) : null}

            {tab === "mcp" ? (
              <section className="settings-section">
                <div className="form-section-head">
                  MCP SERVERS
                  <span className="right">
                    <Badge>
                      {counts.servers}/{servers.length} ENABLED
                    </Badge>
                  </span>
                </div>
                <div className="body">
                  <div className="field-hint">
                    MCP 服务器把外部工具接入 Agent。默认每次调用都会请求你的批准；
                    只有明确开启“自动授权”的服务器才会直接执行。stdio 服务器会在首次
                    使用时启动，禁用或删除时结束进程。
                  </div>
                  {servers.length === 0 ? (
                    <div className="ext-empty">
                      <Plug size={20} />
                      <div className="ext-empty-title">还没有配置 MCP 服务器</div>
                      <div className="ext-empty-copy">
                        添加 stdio 命令（例如 npx 启动的 MCP 包）或 Streamable HTTP
                        服务，保存后刷新工具即可让 Agent 使用。
                      </div>
                    </div>
                  ) : (
                    <div className="ext-stack">
                      {servers.map((server) => {
                        const open = Boolean(expanded[server.id]);
                        const list = toolsByServer[server.id] ?? [];
                        return (
                          <div className="ext-card" key={server.id}>
                            <div className="ext-card-head">
                              <button
                                type="button"
                                className="ext-expand"
                                onClick={() => void toggleExpand(server)}
                              >
                                {open ? (
                                  <ChevronDown size={13} />
                                ) : (
                                  <ChevronRight size={13} />
                                )}
                                <Server size={13} />
                                <span className="name">{server.name}</span>
                              </button>
                              <div className="ext-card-actions">
                                <Badge tone={mcpStatusTone(server.status)}>
                                  {server.status.toUpperCase()}
                                </Badge>
                                <Badge>{server.transport.toUpperCase()}</Badge>
                                {server.autoApprove ? (
                                  <Badge tone="warn">自动授权</Badge>
                                ) : null}
                                <Switch
                                  checked={server.enabled}
                                  disabled={busy === server.id}
                                  onChange={() => void toggleServer(server)}
                                />
                                <IconButton
                                  label="刷新工具"
                                  size="sm"
                                  disabled={busy === server.id || !server.enabled}
                                  onClick={() => void refreshServer(server)}
                                >
                                  {busy === server.id ? (
                                    <Loader2
                                      size={11}
                                      style={{ animation: "spin .8s linear infinite" }}
                                    />
                                  ) : (
                                    <RefreshCw size={11} />
                                  )}
                                </IconButton>
                                <IconButton
                                  label="编辑服务器"
                                  size="sm"
                                  onClick={() => setDraft(draftFromServer(server))}
                                >
                                  <Wrench size={11} />
                                </IconButton>
                                <IconButton
                                  label="删除服务器"
                                  size="sm"
                                  onClick={() => setDeleteServer(server)}
                                >
                                  <Trash2 size={11} />
                                </IconButton>
                              </div>
                            </div>
                            <div className="ext-card-meta">
                              <span className="path mono">
                                {server.transport === "http"
                                  ? server.url
                                  : [server.command, ...server.args].join(" ")}
                              </span>
                            </div>
                            {server.lastError ? (
                              <div className="ext-error">
                                <AlertTriangle size={11} /> {server.lastError}
                              </div>
                            ) : null}
                            {open ? (
                              <div className="ext-tool-list">
                                {list.length === 0 ? (
                                  <div className="ext-empty-copy">
                                    尚未加载工具。点击刷新建立连接。
                                  </div>
                                ) : (
                                  list.map((tool) => (
                                    <div className="ext-tool" key={tool.name}>
                                      <span className="ext-tool-name mono" data-i18n-skip>
                                        {tool.toolName}
                                      </span>
                                      {tool.readonly ? (
                                        <Badge tone="info">READ ONLY</Badge>
                                      ) : null}
                                      <span className="ext-tool-desc">
                                        {tool.description || "无描述"}
                                      </span>
                                      <button
                                        type="button"
                                        className="ext-tool-test"
                                        disabled={!server.enabled}
                                        onClick={() => openToolTest(server, tool)}
                                      >
                                        <Play size={9} /> 测试
                                      </button>
                                    </div>
                                  ))
                                )}
                              </div>
                            ) : null}
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              </section>
            ) : null}

            {tab === "plugins" ? (
              <section className="settings-section">
                <div className="form-section-head">
                  PLUGIN PACKS
                  <span className="right">
                    <Badge>{plugins.length} INSTALLED</Badge>
                  </span>
                </div>
                <div className="body">
                  <div className="field-hint">
                    插件包用一个 plugin.json 清单同时分发多个 Skill 与 MCP 服务器。
                    卸载插件会移除它安装的 Skill、MCP 配置和本地副本，不会影响你的项目文件。
                  </div>
                  {plugins.length === 0 ? (
                    <div className="ext-empty">
                      <Package size={20} />
                      <div className="ext-empty-title">还没有安装插件包</div>
                      <div className="ext-empty-copy">
                        文件夹根目录放置 plugin.json，声明 skills 与 mcpServers 后即可导入。
                      </div>
                      <pre className="ext-code">{PLUGIN_SAMPLE}</pre>
                    </div>
                  ) : (
                    <div className="ext-grid">
                      {plugins.map((plugin) => (
                        <div className="ext-card" key={plugin.id}>
                          <div className="ext-card-head">
                            <div className="ext-card-title">
                              <Package size={13} />
                              <span className="name">{plugin.name}</span>
                              {plugin.version ? (
                                <Badge>v{plugin.version}</Badge>
                              ) : null}
                            </div>
                            <div className="ext-card-actions">
                              <IconButton
                                label="打开插件目录"
                                size="sm"
                                onClick={() => void api.openPath(plugin.path)}
                              >
                                <FolderOpen size={11} />
                              </IconButton>
                              <IconButton
                                label="卸载插件"
                                size="sm"
                                onClick={() => setDeletePlugin(plugin)}
                              >
                                <Trash2 size={11} />
                              </IconButton>
                            </div>
                          </div>
                          <div className="ext-card-meta">
                            <Badge tone="ok">{plugin.skills} SKILLS</Badge>
                            <Badge tone="warn">{plugin.mcpServers} MCP</Badge>
                          </div>
                          <div className="ext-desc">
                            {plugin.description || "没有提供说明"}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </section>
            ) : null}

            {tab === "registry" ? (
              <section className="settings-section">
                <div className="form-section-head">
                  TOOL REGISTRY
                  <span className="right">
                    <Badge>{tools.length} TOOLS</Badge>
                  </span>
                </div>
                <div className="body">
                  <div className="field-hint">
                    这是当前 Agent 实际可用的统一工具注册表：内置工具、Skill 加载器与
                    已启用的 MCP 工具。审批策略在服务端强制执行，不能由模型绕过。
                  </div>
                  {(["builtin", "skill", "mcp"] as const).map((source) => (
                    <div className="ext-registry-group" key={source}>
                      <div className="ext-registry-head">
                        {source === "builtin"
                          ? "内置工具"
                          : source === "skill"
                            ? "SKILL 工具"
                            : "MCP 工具"}
                        <span className="ext-tab-count">
                          {groupedTools[source]?.length ?? 0}
                        </span>
                      </div>
                      <div className="ext-stack">
                        {(groupedTools[source] ?? []).map((tool) => (
                          <div className="ext-tool-row" key={tool.name}>
                            <span
                              className="ext-tool-name mono"
                              data-i18n-skip
                            >
                              {tool.name}
                            </span>
                            <Badge tone={sourceTone(tool.source)}>
                              {tool.source.toUpperCase()}
                            </Badge>
                            {tool.server ? (
                              <Badge>{tool.server}</Badge>
                            ) : null}
                            {tool.readonly ? <Badge tone="info">READ ONLY</Badge> : null}
                            {tool.requiresApproval ? (
                              <Badge tone="warn">
                                <ShieldCheck size={9} /> 需批准
                              </Badge>
                            ) : (
                              <Badge tone="ok">自动</Badge>
                            )}
                            <span className="ext-tool-desc">{tool.description}</span>
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </section>
            ) : null}

            <div className="ext-footnote">
              <ShieldCheck size={11} />
              Skill 与 MCP 配置保存在本机应用数据目录；MCP 认证信息会随配置保存在
              SQLite 中，请勿在不可信设备上导入未知插件。需要更高安全性时请使用系统凭据
              管理器保存的密钥，不要在环境变量中写入长期令牌。
            </div>
          </div>
        </div>
      </div>

      <Modal
        open={draft !== null}
        onClose={() => setDraft(null)}
        kicker="MCP SERVER"
        title={draft?.id ? "编辑 MCP 服务器" : "添加 MCP 服务器"}
        size="lg"
        footer={
          <>
            <Button onClick={() => setDraft(null)}>取消</Button>
            <Button
              variant="primary"
              onClick={() => void saveDraft()}
              disabled={busy === "save-mcp"}
            >
              {busy === "save-mcp" ? <Loader2 size={12} /> : null} 保存
            </Button>
          </>
        }
      >
        {draft ? (
          <div className="ext-form">
            <div className="ext-presets">
              <span className="field-label">快速模板</span>
              <Button
                size="sm"
                onClick={() =>
                  setDraft({
                    ...draft,
                    name: "memory",
                    transport: "stdio",
                    command: "npx",
                    argsText: "-y\n@modelcontextprotocol/server-memory",
                    envText: "{}",
                  })
                }
              >
                Memory
              </Button>
              <Button
                size="sm"
                onClick={() =>
                  setDraft({
                    ...draft,
                    name: "filesystem",
                    transport: "stdio",
                    command: "npx",
                    argsText:
                      "-y\n@modelcontextprotocol/server-filesystem\nC:\\path\\to\\folder",
                    envText: "{}",
                  })
                }
              >
                Filesystem
              </Button>
              <Button
                size="sm"
                onClick={() =>
                  setDraft({
                    ...draft,
                    name: "fetch",
                    transport: "stdio",
                    command: "uvx",
                    argsText: "mcp-server-fetch",
                    envText: "{}",
                  })
                }
              >
                Fetch
              </Button>
            </div>
            <div className="ext-form-grid">
              <Field label="名称">
                <Input
                  value={draft.name}
                  onChange={(event) =>
                    setDraft({ ...draft, name: event.target.value })
                  }
                  placeholder="例如 docs 或 filesystem"
                />
              </Field>
              <Field label="传输方式">
                <Select
                  value={draft.transport}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      transport:
                        event.target.value === "http" ? "http" : "stdio",
                    })
                  }
                >
                  <option value="stdio">stdio（本地进程）</option>
                  <option value="http">Streamable HTTP</option>
                </Select>
              </Field>
            </div>
            {draft.transport === "stdio" ? (
              <>
                <Field
                  label="启动命令"
                  hint="在 Windows 上通过 cmd /C 启动，因此 npx、uvx 等 .cmd 包可以直接使用。"
                >
                  <Input
                    className="mono"
                    value={draft.command}
                    onChange={(event) =>
                      setDraft({ ...draft, command: event.target.value })
                    }
                    placeholder="npx"
                    data-i18n-skip
                  />
                </Field>
                <Field label="参数（每行一个）">
                  <Textarea
                    rows={4}
                    className="mono"
                    value={draft.argsText}
                    onChange={(event) =>
                      setDraft({ ...draft, argsText: event.target.value })
                    }
                    placeholder={"-y\n@modelcontextprotocol/server-memory"}
                    data-i18n-skip
                  />
                </Field>
                <Field
                  label="环境变量（JSON）"
                  hint="令牌会以明文保存在本机 SQLite 的服务器配置中。"
                >
                  <Textarea
                    rows={3}
                    className="mono"
                    value={draft.envText}
                    onChange={(event) =>
                      setDraft({ ...draft, envText: event.target.value })
                    }
                    data-i18n-skip
                  />
                </Field>
              </>
            ) : (
              <>
                <Field label="服务器 URL">
                  <Input
                    className="mono"
                    value={draft.url}
                    onChange={(event) =>
                      setDraft({ ...draft, url: event.target.value })
                    }
                    placeholder="https://example.com/mcp"
                    data-i18n-skip
                  />
                </Field>
                <Field label="请求头（JSON）">
                  <Textarea
                    rows={3}
                    className="mono"
                    value={draft.headersText}
                    onChange={(event) =>
                      setDraft({ ...draft, headersText: event.target.value })
                    }
                    data-i18n-skip
                  />
                </Field>
              </>
            )}
            <div className="ext-form-toggles">
              <label className="ext-toggle-row">
                <span>
                  <span className="ext-toggle-title">启用服务器</span>
                  <span className="ext-toggle-desc">
                    禁用时隐藏其工具并结束已启动的本地进程。
                  </span>
                </span>
                <Switch
                  checked={draft.enabled}
                  onChange={(enabled) => setDraft({ ...draft, enabled })}
                />
              </label>
              <label className="ext-toggle-row">
                <span>
                  <span className="ext-toggle-title">自动授权工具调用</span>
                  <span className="ext-toggle-desc">
                    开启后该服务器的工具不再逐次请求批准。只对完全信任的服务器启用。
                  </span>
                </span>
                <Switch
                  checked={draft.autoApprove}
                  onChange={(autoApprove) => setDraft({ ...draft, autoApprove })}
                />
              </label>
            </div>
            <div className="warning-box">
              保存后工具缓存会清空。点击服务器卡片上的刷新按钮重新连接并加载工具。
            </div>
          </div>
        ) : null}
      </Modal>

      <Modal
        open={toolTest !== null}
        onClose={() => setToolTest(null)}
        kicker="MCP TOOL TEST"
        title={
          toolTest
            ? `测试工具 · ${toolTest.server.name} / ${toolTest.tool.toolName}`
            : "测试工具"
        }
        size="lg"
        footer={
          <>
            <Button onClick={() => setToolTest(null)}>关闭</Button>
            <Button
              variant="primary"
              disabled={testing}
              onClick={() => void runToolTest()}
            >
              {testing ? (
                <Loader2
                  size={12}
                  style={{ animation: "spin .8s linear infinite" }}
                />
              ) : (
                <Play size={12} />
              )}
              运行调用
            </Button>
          </>
        }
      >
        {toolTest ? (
          <div className="ext-form">
            <div className="field-hint">
              测试调用会直接连接 MCP 服务器并执行 tools/call，不经过 Agent
              的逐次审批。请先确认参数不会产生你不希望发生的副作用。
            </div>
            <Field label="参数（JSON）">
              <Textarea
                rows={6}
                className="mono"
                value={testArgs}
                onChange={(event) => setTestArgs(event.target.value)}
                data-i18n-skip
              />
            </Field>
            {testError ? (
              <div className="ext-error">
                <AlertTriangle size={11} /> {testError}
              </div>
            ) : null}
            {testResult ? (
              <div className="ext-test-result">
                <div className="ext-registry-head">调用结果</div>
                <pre className="ext-code">{testResult}</pre>
              </div>
            ) : null}
          </div>
        ) : null}
      </Modal>

      <ConfirmDialog
        open={deleteSkill !== null}
        title="删除 Skill"
        message={
          <>
            将删除 <strong>{deleteSkill?.name}</strong> 及其本地副本。项目文件不会被修改。
          </>
        }
        confirmLabel="删除"
        danger
        onCancel={() => setDeleteSkill(null)}
        onConfirm={() => {
          if (deleteSkill) void removeSkill(deleteSkill);
        }}
      />
      <ConfirmDialog
        open={deleteServer !== null}
        title="删除 MCP 服务器"
        message={
          <>
            将删除 <strong>{deleteServer?.name}</strong> 的配置、工具缓存并结束进程。
          </>
        }
        confirmLabel="删除"
        danger
        onCancel={() => setDeleteServer(null)}
        onConfirm={() => {
          if (deleteServer) void removeServer(deleteServer);
        }}
      />
      <ConfirmDialog
        open={deletePlugin !== null}
        title="卸载插件包"
        message={
          <>
            将卸载 <strong>{deletePlugin?.name}</strong> 以及它安装的 Skill 和 MCP
            服务器。你的项目文件不会被修改。
          </>
        }
        confirmLabel="卸载"
        danger
        onCancel={() => setDeletePlugin(null)}
        onConfirm={() => {
          if (deletePlugin) void removePlugin(deletePlugin);
        }}
      />
    </div>
  );
}
