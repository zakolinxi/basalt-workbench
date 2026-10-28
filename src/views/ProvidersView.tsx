import { useEffect, useState } from "react";
import {
  Check,
  ChevronDown,
  Copy,
  KeyRound,
  Plus,
  RefreshCw,
  Save,
  ShieldCheck,
  Star,
  Trash2,
  Wifi,
  X,
} from "lucide-react";
import clsx from "clsx";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../lib/store";
import {
  capabilityLabel,
  capabilityStateLabel,
  providerProtocolLabel,
} from "../lib/format";
import { normalizeBaseUrl } from "../lib/baseUrl";
import type {
  CapState,
  ModelCapabilities,
  Provider,
  ProviderInput,
  ProviderModel,
  ProviderTestResult,
} from "../lib/types";
import {
  Badge,
  Button,
  ConfirmDialog,
  Field,
  IconButton,
  Input,
  Select,
  Switch,
  Textarea,
} from "../components/ui";
import { TitlebarDrag } from "../components/WindowControls";

const CHAT_PROTOCOLS = [
  "openai_chat",
  "openai_responses",
  "anthropic",
  "gemini",
  "ollama",
];
const IMAGE_PROTOCOLS = ["openai_images", "gemini_images"];
const CAPABILITIES = [
  "chat",
  "stream",
  "tools",
  "vision",
  "imageGen",
  "imageEdit",
  "json",
  "longContext",
  "multiTool",
  "reasoning",
] as const;
const CAP_STATES: CapState[] = [
  "verified",
  "unverified",
  "unsupported",
  "failed",
  "unavailable",
];

type CapKey = (typeof CAPABILITIES)[number];
type DefaultModelKey =
  | "defaultChatModel"
  | "defaultVisionModel"
  | "defaultImageModel";

const CAP_GROUPS: { title: string; keys: CapKey[] }[] = [
  { title: "对话与输出", keys: ["chat", "stream", "json", "longContext"] },
  { title: "工具与推理", keys: ["tools", "multiTool", "reasoning"] },
  { title: "视觉与图像", keys: ["vision", "imageGen", "imageEdit"] },
];

const CAP_HINTS: Record<CapKey, string> = {
  chat: "基础对话",
  stream: "流式返回",
  tools: "函数与工具调用",
  vision: "图片输入",
  imageGen: "文本生成图片",
  imageEdit: "参考图编辑",
  json: "结构化输出",
  longContext: "长上下文窗口",
  multiTool: "连续多轮工具",
  reasoning: "思考强度档位",
};

const CAPS_BY_CATEGORY: Record<"chat" | "image", CapKey[]> = {
  chat: [
    "chat",
    "stream",
    "json",
    "longContext",
    "tools",
    "multiTool",
    "reasoning",
    "vision",
  ],
  image: ["imageGen", "imageEdit", "vision"],
};

const SUMMARY_CAPS: Record<"chat" | "image", CapKey[]> = {
  chat: ["tools", "vision", "reasoning", "longContext"],
  image: ["imageGen", "imageEdit", "vision"],
};

const CONTEXT_PRESETS = [
  { label: "32K", value: 32_000 },
  { label: "128K", value: 128_000 },
  { label: "200K", value: 200_000 },
  { label: "1M", value: 1_000_000 },
];

const SOURCE_LABELS: Record<string, string> = {
  preset: "预置",
  api: "接口",
  manual: "手动",
};

function trimNumber(value: number): string {
  return Number(value.toFixed(2)).toString();
}

function formatContextLength(value?: number | null): string {
  if (!value || value <= 0) return "未设置";
  if (value >= 1_000_000) return `${trimNumber(value / 1_000_000)}M`;
  if (value >= 1000) return `${trimNumber(value / 1000)}K`;
  return value.toLocaleString("en-US");
}

function chatCaps(stream: boolean): ModelCapabilities {
  return {
    chat: "verified",
    stream: stream ? "unverified" : "unsupported",
    tools: "unverified",
    vision: "unverified",
    imageGen: "unsupported",
    imageEdit: "unsupported",
    json: "unverified",
    longContext: "unverified",
    multiTool: "unverified",
    reasoning: "unverified",
  };
}

function imageCaps(): ModelCapabilities {
  return {
    chat: "unsupported",
    stream: "unsupported",
    tools: "unsupported",
    vision: "unverified",
    imageGen: "unverified",
    imageEdit: "unverified",
    json: "unsupported",
    longContext: "unsupported",
    multiTool: "unsupported",
    reasoning: "unsupported",
  };
}

function blankForm(): ProviderInput {
  return {
    name: "",
    category: "chat",
    protocol: "openai_chat",
    baseUrl: "",
    models: [],
    defaultChatModel: null,
    defaultVisionModel: null,
    defaultImageModel: null,
    headers: {},
    params: {},
    timeoutSecs: 120,
    stream: true,
    enabled: true,
  };
}

function formFromProvider(provider: Provider): ProviderInput {
  return {
    id: provider.id,
    name: provider.name,
    category: provider.category,
    protocol: provider.protocol,
    baseUrl: provider.baseUrl,
    models: provider.models,
    defaultChatModel: provider.defaultChatModel ?? null,
    defaultVisionModel: provider.defaultVisionModel ?? null,
    defaultImageModel: provider.defaultImageModel ?? null,
    headers: provider.headers,
    params: provider.params,
    timeoutSecs: provider.timeoutSecs,
    stream: provider.stream,
    enabled: provider.enabled,
  };
}

export function ProvidersView() {
  const providers = useAppStore((state) => state.providers);
  const refreshProviders = useAppStore((state) => state.refreshProviders);
  const notify = useAppStore((state) => state.notify);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [form, setForm] = useState<ProviderInput | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [clearKey, setClearKey] = useState(false);
  const [headersText, setHeadersText] = useState("{}");
  const [paramsText, setParamsText] = useState("{}");
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<ProviderTestResult | null>(null);
  const [modelDraft, setModelDraft] = useState({
    id: "",
    name: "",
    context: "",
  });
  const [expandedModel, setExpandedModel] = useState<number | null>(null);
  const [addingModel, setAddingModel] = useState(false);
  const [baseUrlNote, setBaseUrlNote] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Provider | null>(null);

  const selected = providers.find((item) => item.id === selectedId) ?? null;
  const templates = providers.filter((provider) => provider.isTemplate);
  const configured = providers.filter((provider) => !provider.isTemplate);

  const loadProvider = (provider: Provider) => {
    setSelectedId(provider.id);
    setTestResult(null);
    setApiKey("");
    setClearKey(false);
    setExpandedModel(null);
    setAddingModel(false);
    setBaseUrlNote(null);
    setModelDraft({ id: "", name: "", context: "" });
    if (provider.isTemplate) {
      setForm(null);
      return;
    }
    const next = formFromProvider(provider);
    setForm(next);
    setHeadersText(JSON.stringify(provider.headers ?? {}, null, 2));
    setParamsText(JSON.stringify(provider.params ?? {}, null, 2));
  };

  useEffect(() => {
    if (selectedId) return;
    const preferred =
      configured.find((provider) => provider.enabled) ?? configured[0] ?? null;
    if (preferred) loadProvider(preferred);
  }, [configured, selectedId]);

  const startNew = () => {
    setSelectedId("new");
    setForm(blankForm());
    setHeadersText("{}");
    setParamsText("{}");
    setApiKey("");
    setClearKey(false);
    setTestResult(null);
    setExpandedModel(null);
    setAddingModel(false);
    setBaseUrlNote(null);
    setModelDraft({ id: "", name: "", context: "" });
  };

  const duplicate = async (provider: Provider) => {
    try {
      const copy = await api.providerDuplicate(provider.id);
      await refreshProviders();
      setSelectedId(copy.id);
      const next = formFromProvider(copy);
      setForm(next);
      setHeadersText(JSON.stringify(copy.headers ?? {}, null, 2));
      setParamsText(JSON.stringify(copy.params ?? {}, null, 2));
      setApiKey("");
      setClearKey(false);
      setTestResult(null);
      setExpandedModel(null);
      setAddingModel(false);
      setBaseUrlNote(null);
      setModelDraft({ id: "", name: "", context: "" });
      notify(
        "success",
        provider.isTemplate ? "已创建供应商" : "已创建供应商副本",
        copy.name,
      );
    } catch (error) {
      notify("error", "复制供应商失败", errorMessage(error));
    }
  };

  const buildInput = (): ProviderInput => {
    if (!form) throw new Error("没有可保存的配置");
    let headers: Record<string, unknown>;
    let params: Record<string, unknown>;
    try {
      headers = headersText.trim() ? JSON.parse(headersText) : {};
      params = paramsText.trim() ? JSON.parse(paramsText) : {};
    } catch {
      throw new Error("自定义请求头或参数不是有效的 JSON");
    }
    if (
      typeof headers !== "object" ||
      Array.isArray(headers) ||
      headers === null
    ) {
      throw new Error("自定义请求头必须是 JSON 对象");
    }
    if (
      typeof params !== "object" ||
      Array.isArray(params) ||
      params === null
    ) {
      throw new Error("自定义参数必须是 JSON 对象");
    }
    const modelIds = new Set(form.models.map((model) => model.id));
    const defaultChatModel =
      form.defaultChatModel && modelIds.has(form.defaultChatModel)
        ? form.defaultChatModel
        : null;
    const defaultVisionModel =
      form.defaultVisionModel && modelIds.has(form.defaultVisionModel)
        ? form.defaultVisionModel
        : null;
    const defaultImageModel =
      form.defaultImageModel && modelIds.has(form.defaultImageModel)
        ? form.defaultImageModel
        : null;
    return {
      ...form,
      name: form.name.trim(),
      baseUrl: normalizeBaseUrl(form.baseUrl, form.protocol),
      defaultChatModel,
      defaultVisionModel,
      defaultImageModel,
      headers,
      params,
      apiKey: apiKey.trim() || undefined,
      clearApiKey: clearKey,
      timeoutSecs: Number(form.timeoutSecs) || 120,
    };
  };

  const save = async (silent = false): Promise<Provider | null> => {
    if (!form) return null;
    setSaving(true);
    try {
      const input = buildInput();
      if (!input.name) throw new Error("供应商名称不能为空");
      const saved = form.id
        ? await api.providerUpdate(input)
        : await api.providerCreate(input);
      await refreshProviders();
      const next = formFromProvider(saved);
      setForm(next);
      setSelectedId(saved.id);
      setApiKey("");
      setClearKey(false);
      setBaseUrlNote(null);
      setHeadersText(JSON.stringify(saved.headers ?? {}, null, 2));
      setParamsText(JSON.stringify(saved.params ?? {}, null, 2));
      if (!silent) notify("success", "供应商配置已保存", saved.name);
      return saved;
    } catch (error) {
      notify("error", "保存供应商失败", errorMessage(error));
      return null;
    } finally {
      setSaving(false);
    }
  };

  const testConnection = async () => {
    const saved = await save(true);
    if (!saved) return;
    setTesting(true);
    setTestResult(null);
    try {
      const result = await api.providerTest(saved.id);
      setTestResult(result);
      if (result.models) {
        setForm((current) =>
          current ? { ...current, models: result.models! } : current,
        );
        setExpandedModel(null);
        setAddingModel(false);
        await refreshProviders();
      }
      notify(
        result.ok ? "success" : "error",
        result.ok ? "连接测试成功" : "连接测试失败",
        result.message,
      );
    } catch (error) {
      setTestResult({
        ok: false,
        message: errorMessage(error),
        latencyMs: 0,
      });
      notify("error", "连接测试失败", errorMessage(error));
    } finally {
      setTesting(false);
    }
  };

  const updateForm = (patch: Partial<ProviderInput>) =>
    setForm((current) => (current ? { ...current, ...patch } : current));

  const updateModel = (index: number, patch: Partial<ProviderModel>) => {
    setForm((current) => {
      if (!current) return current;
      const previous = current.models[index];
      const models = current.models.map((model, i) =>
        i === index ? { ...model, ...patch } : model,
      );
      if (!previous || patch.id === undefined || patch.id === previous.id) {
        return { ...current, models };
      }
      const next = patch.id;
      const rename = (value: string | null | undefined) =>
        value && value === previous.id && next ? next : (value ?? null);
      return {
        ...current,
        models,
        defaultChatModel: rename(current.defaultChatModel),
        defaultVisionModel: rename(current.defaultVisionModel),
        defaultImageModel: rename(current.defaultImageModel),
      };
    });
  };

  const updateCapability = (index: number, key: CapKey, value: CapState) => {
    setForm((current) => {
      if (!current) return current;
      const models = current.models.map((model, i) =>
        i === index
          ? {
              ...model,
              capabilities: { ...model.capabilities, [key]: value },
            }
          : model,
      );
      return { ...current, models };
    });
  };

  const setAllCapabilities = (index: number, value: CapState) => {
    setForm((current) => {
      if (!current) return current;
      const allowed = CAPS_BY_CATEGORY[current.category];
      const models = current.models.map((model, i) => {
        if (i !== index) return model;
        const capabilities = { ...model.capabilities };
        for (const key of allowed) {
          capabilities[key] = value;
        }
        return { ...model, capabilities };
      });
      return { ...current, models };
    });
  };

  const addModel = () => {
    if (!form) return;
    const id = modelDraft.id.trim();
    if (!id) {
      notify("warning", "模型 ID 不能为空");
      return;
    }
    if (form.models.some((model) => model.id === id)) {
      notify("warning", "模型 ID 已存在");
      return;
    }
    const contextLength = modelDraft.context.trim()
      ? Number(modelDraft.context)
      : null;
    if (
      contextLength !== null &&
      (!Number.isFinite(contextLength) || contextLength <= 0)
    ) {
      notify("warning", "上下文窗口必须是正数");
      return;
    }
    const capabilities =
      form.category === "image" ? imageCaps() : chatCaps(form.stream);
    const model: ProviderModel = {
      id,
      name: modelDraft.name.trim() || id,
      contextLength,
      capabilities,
      source: "manual",
    };
    const nextIndex = form.models.length;
    setForm({
      ...form,
      models: [...form.models, model],
      defaultChatModel:
        form.category === "chat" && !form.defaultChatModel
          ? id
          : form.defaultChatModel,
      defaultImageModel:
        form.category === "image" && !form.defaultImageModel
          ? id
          : form.defaultImageModel,
    });
    setExpandedModel(nextIndex);
    setAddingModel(false);
    setModelDraft({ id: "", name: "", context: "" });
  };

  const startAddingModel = () => {
    setAddingModel(true);
    setExpandedModel(null);
    setModelDraft({ id: "", name: "", context: "" });
  };

  const cancelAddingModel = () => {
    setAddingModel(false);
    setModelDraft({ id: "", name: "", context: "" });
  };

  const toggleDefaultModel = (index: number, key: DefaultModelKey) => {
    if (!form) return;
    const id = form.models[index]?.id;
    if (!id) return;
    const patch: Partial<ProviderInput> = {};
    patch[key] = form[key] === id ? null : id;
    updateForm(patch);
  };

  const commitBaseUrl = () => {
    if (!form) return;
    const normalized = normalizeBaseUrl(form.baseUrl, form.protocol);
    if (normalized === form.baseUrl) {
      setBaseUrlNote(null);
      return;
    }
    updateForm({ baseUrl: normalized });
    setBaseUrlNote(normalized ? `已自动补全为 ${normalized}` : null);
  };

  const removeModel = (index: number) => {
    if (!form) return;
    const removed = form.models[index];
    const models = form.models.filter((_, i) => i !== index);
    updateForm({
      models,
      defaultChatModel:
        form.defaultChatModel === removed.id ? null : form.defaultChatModel,
      defaultVisionModel:
        form.defaultVisionModel === removed.id ? null : form.defaultVisionModel,
      defaultImageModel:
        form.defaultImageModel === removed.id ? null : form.defaultImageModel,
    });
    setExpandedModel((current) =>
      current === null
        ? null
        : current === index
          ? null
          : current > index
            ? current - 1
            : current,
    );
  };

  const protocolOptions =
    form?.category === "image" ? IMAGE_PROTOCOLS : CHAT_PROTOCOLS;

  return (
    <div className="view">
      <div className="page">
        <header className="page-head">
          <div>
            <div className="page-kicker">PROVIDER REGISTRY</div>
            <h1 className="page-title">供应商与模型</h1>
            <div className="page-sub">
              多协议适配 · 模型能力矩阵 · 密钥存于 Windows 凭据管理器
            </div>
          </div>
          <TitlebarDrag />
          <div className="page-head-actions">
            <Button onClick={() => void refreshProviders()}>
              <RefreshCw size={12} /> 刷新
            </Button>
            <Button variant="primary" onClick={startNew}>
              <Plus size={12} /> 新增供应商
            </Button>
          </div>
        </header>
        <div className="page-body">
          <div className="providers-layout">
            <div className="provider-list">
              <div className="provider-list-section">
                <div className="provider-list-heading">我的供应商</div>
                {configured.length === 0 ? (
                  <div className="sidebar-empty">还没有配置供应商，请从模板开始。</div>
                ) : null}
                {configured.map((provider) => (
                  <div
                    key={provider.id}
                    className={clsx(
                      "provider-item",
                      provider.id === selectedId && "active",
                    )}
                    onClick={() => loadProvider(provider)}
                  >
                    <span className={clsx("dot", provider.enabled && "on")} />
                    <span className="meta">
                      <span className="name">{provider.name}</span>
                      <span className="detail">
                        {providerProtocolLabel(provider.protocol)}
                        <span>·</span>
                        {provider.models.length} models
                        {provider.lastStatus === "ok" ? (
                          <span style={{ color: "var(--ok)" }}>OK</span>
                        ) : provider.lastStatus === "error" ? (
                          <span style={{ color: "var(--err)" }}>ERR</span>
                        ) : null}
                      </span>
                    </span>
                  </div>
                ))}
              </div>
              <div className="provider-list-section">
                <div className="provider-list-heading">
                  模板
                  <span style={{ marginLeft: "auto" }}>{templates.length}</span>
                </div>
                {templates.map((template) => (
                  <div
                    key={template.id}
                    className={clsx(
                      "provider-item",
                      template.id === selectedId && "active",
                    )}
                    onClick={() => loadProvider(template)}
                  >
                    <span className="dot" />
                    <span className="meta">
                      <span className="name">{template.name}</span>
                      <span className="detail">
                        {providerProtocolLabel(template.protocol)}
                      </span>
                    </span>
                    <button
                      className="btn xs"
                      onClick={(event) => {
                        event.stopPropagation();
                        void duplicate(template);
                      }}
                    >
                      使用
                    </button>
                  </div>
                ))}
              </div>
            </div>

            <div className="provider-detail">
              {!form && selected?.isTemplate ? (
                <TemplatePanel
                  provider={selected}
                  onUse={() => void duplicate(selected)}
                />
              ) : !form ? (
                <div className="provider-empty">
                  <Wifi size={22} />
                  <div>选择左侧供应商，或新增一个供应商配置。</div>
                  <Button variant="primary" onClick={startNew}>
                    <Plus size={12} /> 新增供应商
                  </Button>
                </div>
              ) : (
                <div className="provider-form">
                  <div className="form-actions">
                    <div>
                      <div className="section-label">
                        {form.id ? "EDIT PROVIDER" : "NEW PROVIDER"}
                      </div>
                      <div style={{ fontSize: 15, fontWeight: 600, marginTop: 2 }}>
                        {form.name || "未命名供应商"}
                      </div>
                    </div>
                    <span className="spacer" />
                    <Button
                      onClick={() => void testConnection()}
                      loading={testing}
                      disabled={saving}
                    >
                      <Wifi size={12} /> 测试连接 / 拉取模型
                    </Button>
                    <Button
                      variant="primary"
                      onClick={() => void save()}
                      loading={saving}
                    >
                      <Save size={12} /> 保存
                    </Button>
                    {form.id ? (
                      <>
                        <Button onClick={() => {
                          const provider = providers.find((p) => p.id === form.id);
                          if (provider) void duplicate(provider);
                        }}>
                          <Copy size={12} /> 复制
                        </Button>
                        {!form.id.startsWith("builtin-") ? (
                          <Button
                            variant="danger"
                            onClick={() => {
                              const provider = providers.find((p) => p.id === form.id);
                              if (provider) setDeleteTarget(provider);
                            }}
                          >
                            <Trash2 size={12} /> 删除
                          </Button>
                        ) : null}
                      </>
                    ) : null}
                  </div>

                  {testResult ? (
                    <div className={clsx("test-result", testResult.ok ? "ok" : "err")}>
                      <Wifi size={13} style={{ marginTop: 1 }} />
                      <div>
                        {testResult.ok ? "连接成功" : "连接失败"} ·{" "}
                        {testResult.latencyMs}ms
                        <div style={{ marginTop: 2 }}>{testResult.message}</div>
                      </div>
                    </div>
                  ) : null}

                  <section className="form-section">
                    <div className="form-section-head">基础配置</div>
                    <div className="form-section-body">
                      <div className="form-grid">
                        <Field label="显示名称">
                          <Input
                            value={form.name}
                            onChange={(event) =>
                              updateForm({ name: event.target.value })
                            }
                            placeholder="例如：OpenAI 主账号"
                          />
                        </Field>
                        <Field label="供应商类型">
                          <Select
                            value={form.category}
                            onChange={(event) => {
                              const category = event.target.value as "chat" | "image";
                              const protocol =
                                category === "image" ? "openai_images" : "openai_chat";
                              updateForm({
                                category,
                                protocol,
                                baseUrl: normalizeBaseUrl(form.baseUrl, protocol),
                                defaultChatModel: null,
                                defaultVisionModel: null,
                                defaultImageModel: null,
                              });
                              setBaseUrlNote(null);
                            }}
                          >
                            <option value="chat">对话 / Agent</option>
                            <option value="image">图像生成</option>
                          </Select>
                        </Field>
                        <Field label="协议">
                          <Select
                            value={form.protocol}
                            onChange={(event) => {
                              const protocol = event.target.value;
                              const baseUrl = normalizeBaseUrl(
                                form.baseUrl,
                                protocol,
                              );
                              updateForm({ protocol, baseUrl });
                              setBaseUrlNote(
                                baseUrl && baseUrl !== form.baseUrl
                                  ? `已自动补全为 ${baseUrl}`
                                  : null,
                              );
                            }}
                          >
                            {protocolOptions.map((protocol) => (
                              <option value={protocol} key={protocol}>
                                {providerProtocolLabel(protocol)}
                              </option>
                            ))}
                          </Select>
                        </Field>
                        <Field
                          label="API Base URL"
                          hint={
                            baseUrlNote ? (
                              <span className="base-url-note">{baseUrlNote}</span>
                            ) : (
                              "只填域名即可自动补全 /v1；Gemini 补 /v1beta，Ollama 保留根地址。"
                            )
                          }
                        >
                          <Input
                            className="mono"
                            value={form.baseUrl}
                            onChange={(event) => {
                              setBaseUrlNote(null);
                              updateForm({ baseUrl: event.target.value });
                            }}
                            onBlur={commitBaseUrl}
                            placeholder={
                              form.protocol === "ollama"
                                ? "http://127.0.0.1:11434"
                                : form.protocol === "gemini" ||
                                    form.protocol === "gemini_images"
                                  ? "https://generativelanguage.googleapis.com"
                                  : "https://api.deepseek.com"
                            }
                          />
                        </Field>
                        <Field label="超时（秒）">
                          <Input
                            type="number"
                            min={5}
                            max={600}
                            value={form.timeoutSecs}
                            onChange={(event) =>
                              updateForm({
                                timeoutSecs: Number(event.target.value) || 120,
                              })
                            }
                          />
                        </Field>
                        <Field label="状态">
                          <div className="row" style={{ height: 30 }}>
                            <Switch
                              checked={form.enabled}
                              onChange={(enabled) => updateForm({ enabled })}
                            />
                            <span style={{ fontSize: 11.5, color: "var(--fg-2)" }}>
                              {form.enabled ? "已启用" : "已禁用"}
                            </span>
                          </div>
                        </Field>
                        {form.category === "chat" ? (
                          <Field
                            label="流式输出"
                            hint="关闭后适配器会将完整响应转换为单次输出。"
                          >
                            <div className="row" style={{ height: 30 }}>
                              <Switch
                                checked={form.stream}
                                onChange={(stream) => updateForm({ stream })}
                              />
                              <span style={{ fontSize: 11.5, color: "var(--fg-2)" }}>
                                {form.stream ? "开启" : "关闭"}
                              </span>
                            </div>
                          </Field>
                        ) : null}
                      </div>
                    </div>
                  </section>

                  <section className="form-section">
                    <div className="form-section-head">
                      认证
                      {selected?.hasKey ? (
                        <Badge tone="ok">KEY 已保存</Badge>
                      ) : (
                        <Badge>未设置密钥</Badge>
                      )}
                      <span className="right">
                        <ShieldCheck size={12} />
                      </span>
                    </div>
                    <div className="form-section-body">
                      <Field
                        label={form.protocol === "gemini" || form.protocol === "gemini_images" ? "Gemini API Key" : "API Key"}
                        hint="密钥保存到 Windows 凭据管理器（com.basalt.workbench），不会写入 SQLite 或普通日志。留空表示保持现有密钥。"
                      >
                        <div className="row">
                          <Input
                            type="password"
                            className="mono"
                            value={apiKey}
                            disabled={clearKey}
                            onChange={(event) => setApiKey(event.target.value)}
                            placeholder={
                              selected?.hasKey
                                ? "已保存密钥，输入新值可替换"
                                : "粘贴 API Key"
                            }
                            autoComplete="off"
                          />
                          <KeyRound size={14} style={{ color: "var(--fg-3)" }} />
                        </div>
                      </Field>
                      {selected?.hasKey ? (
                        <label className="checkbox">
                          <input
                            type="checkbox"
                            checked={clearKey}
                            onChange={(event) => setClearKey(event.target.checked)}
                          />
                          保存时清除已存储的 API Key
                        </label>
                      ) : null}
                    </div>
                  </section>

                  <section className="form-section">
                    <div className="form-section-head">
                      模型与能力
                      <span className="right">
                        <span
                          className="mono"
                          style={{ fontSize: 9, color: "var(--fg-3)" }}
                        >
                          {form.models.length} MODELS
                        </span>
                      </span>
                    </div>
                    <div className="form-section-body">
                      <div className="model-section-intro">
                        <div>
                          <div className="model-section-title">模型列表</div>
                          <div className="field-hint">
                            模型 ID 用于接口调用；上下文窗口用于长度估算。能力状态可手动标记，真实调用后会继续更新。
                          </div>
                        </div>
                        <Button size="sm" onClick={startAddingModel}>
                          <Plus size={12} /> 添加模型
                        </Button>
                      </div>

                      {addingModel ? (
                        <div className="model-add-card">
                          <div className="model-add-head">
                            <div>
                              <div className="model-add-title">添加模型</div>
                              <div className="field-hint">
                                先填模型 ID，名称、上下文和能力都可以稍后调整。
                              </div>
                            </div>
                            <IconButton
                              label="关闭添加面板"
                              size="sm"
                              onClick={cancelAddingModel}
                            >
                              <X size={12} />
                            </IconButton>
                          </div>
                          <div className="model-add-fields">
                            <Field label="模型 ID">
                              <Input
                                autoFocus
                                className="mono"
                                placeholder="例如 gpt-4.1"
                                value={modelDraft.id}
                                onChange={(event) =>
                                  setModelDraft((current) => ({
                                    ...current,
                                    id: event.target.value,
                                  }))
                                }
                                onKeyDown={(event) => {
                                  if (event.key === "Enter") addModel();
                                }}
                              />
                            </Field>
                            <Field label="显示名称（可选）">
                              <Input
                                placeholder="默认与模型 ID 相同"
                                value={modelDraft.name}
                                onChange={(event) =>
                                  setModelDraft((current) => ({
                                    ...current,
                                    name: event.target.value,
                                  }))
                                }
                              />
                            </Field>
                            <Field label="上下文窗口（tokens）">
                              <div className="context-input">
                                <Input
                                  type="number"
                                  min={1024}
                                  step={1024}
                                  className="mono"
                                  placeholder="例如 128000"
                                  value={modelDraft.context}
                                  onChange={(event) =>
                                    setModelDraft((current) => ({
                                      ...current,
                                      context: event.target.value,
                                    }))
                                  }
                                />
                                <span className="context-unit">tokens</span>
                              </div>
                            </Field>
                          </div>
                          <div className="model-add-actions">
                            <Button
                              size="sm"
                              variant="primary"
                              onClick={addModel}
                            >
                              <Plus size={12} /> 添加
                            </Button>
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={cancelAddingModel}
                            >
                              取消
                            </Button>
                          </div>
                        </div>
                      ) : null}

                      <div className="model-list">
                        {form.models.length === 0 && !addingModel ? (
                          <div className="model-empty">
                            <div className="model-empty-title">还没有模型</div>
                            <div className="field-hint">
                              手动添加模型，或保存后点击「测试连接 / 拉取模型」自动导入。
                            </div>
                            <Button size="sm" onClick={startAddingModel}>
                              <Plus size={12} /> 添加模型
                            </Button>
                          </div>
                        ) : null}
                        {form.models.map((model, index) => (
                          <ModelCard
                            key={`${model.id}-${index}`}
                            model={model}
                            category={form.category}
                            expanded={expandedModel === index}
                            defaultChat={form.defaultChatModel === model.id}
                            defaultVision={form.defaultVisionModel === model.id}
                            defaultImage={form.defaultImageModel === model.id}
                            onToggle={() =>
                              setExpandedModel((current) =>
                                current === index ? null : index,
                              )
                            }
                            onChange={(patch) => updateModel(index, patch)}
                            onCapability={(key, value) =>
                              updateCapability(index, key, value)
                            }
                            onBulkCaps={(value) =>
                              setAllCapabilities(index, value)
                            }
                            onDefault={(key) => toggleDefaultModel(index, key)}
                            onRemove={() => removeModel(index)}
                          />
                        ))}
                      </div>

                      <div className="field-hint">
                        未验证表示允许尝试但尚未确认；不支持的能力不会出现在模型选择、工具调用和路由中。
                      </div>
                    </div>
                  </section>

                  <section className="form-section">
                    <div className="form-section-head">高级参数</div>
                    <div className="form-section-body">
                      <div className="form-grid">
                        <Field
                          label="自定义请求头（JSON）"
                          hint="Content-Length、Authorization 等核心请求头会被忽略。"
                        >
                          <Textarea
                            className="mono"
                            rows={5}
                            value={headersText}
                            onChange={(event) => setHeadersText(event.target.value)}
                            spellCheck={false}
                          />
                        </Field>
                        <Field
                          label="自定义请求参数（JSON）"
                          hint="会合并到请求体；model、messages、tools、stream 等核心字段不会被覆盖。"
                        >
                          <Textarea
                            className="mono"
                            rows={5}
                            value={paramsText}
                            onChange={(event) => setParamsText(event.target.value)}
                            spellCheck={false}
                          />
                        </Field>
                      </div>
                    </div>
                  </section>

                  <div className="form-actions">
                    <span className="mono" style={{ fontSize: 9, color: "var(--fg-3)" }}>
                      {selected?.lastTestedAt
                        ? `最近测试：${new Date(selected.lastTestedAt).toLocaleString("zh-CN")}`
                        : "尚未测试连接"}
                    </span>
                    <span className="spacer" />
                    <Button onClick={() => void testConnection()} loading={testing}>
                      <Wifi size={12} /> 测试连接
                    </Button>
                    <Button
                      variant="primary"
                      onClick={() => void save()}
                      loading={saving}
                    >
                      <Save size={12} /> 保存配置
                    </Button>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      <ConfirmDialog
        open={deleteTarget !== null}
        title="删除供应商"
        message={
          <>
            将删除 <strong>{deleteTarget?.name}</strong> 的配置与已保存的 API Key。
            历史会话与生成记录会保留，但引用此供应商的路由将被清空。
          </>
        }
        confirmLabel="删除"
        danger
        onCancel={() => setDeleteTarget(null)}
        onConfirm={() => {
          if (!deleteTarget) return;
          const id = deleteTarget.id;
          setDeleteTarget(null);
          void api
            .providerDelete(id)
            .then(async () => {
              await refreshProviders();
              setSelectedId(null);
              setForm(null);
              notify("success", "供应商已删除");
            })
            .catch((error) => notify("error", "删除失败", errorMessage(error)));
        }}
      />
    </div>
  );
}

function ModelCard({
  model,
  category,
  expanded,
  defaultChat,
  defaultVision,
  defaultImage,
  onToggle,
  onChange,
  onCapability,
  onBulkCaps,
  onDefault,
  onRemove,
}: {
  model: ProviderModel;
  category: "chat" | "image";
  expanded: boolean;
  defaultChat: boolean;
  defaultVision: boolean;
  defaultImage: boolean;
  onToggle: () => void;
  onChange: (patch: Partial<ProviderModel>) => void;
  onCapability: (key: CapKey, value: CapState) => void;
  onBulkCaps: (value: CapState) => void;
  onDefault: (key: DefaultModelKey) => void;
  onRemove: () => void;
}) {
  const allowed = CAPS_BY_CATEGORY[category];
  const groups = CAP_GROUPS.map((group) => ({
    title: group.title,
    keys: group.keys.filter((key) => allowed.includes(key)),
  })).filter((group) => group.keys.length > 0);
  const summaryCaps = SUMMARY_CAPS[category].filter(
    (key) => model.capabilities[key] !== "unsupported",
  );
  const initial = (model.name || model.id).trim().charAt(0).toUpperCase() || "M";
  const context = formatContextLength(model.contextLength);

  return (
    <div className={clsx("model-card", expanded && "expanded")}>
      <div className="model-card-head">
        <button
          type="button"
          className="model-card-toggle"
          onClick={onToggle}
          aria-expanded={expanded}
        >
          <span className={clsx("model-card-avatar", expanded && "open")}>
            {initial}
          </span>
          <span className="model-card-main">
            <span className="model-card-title">
              {model.name || model.id}
              {defaultChat ? <Badge tone="info">默认对话</Badge> : null}
              {defaultVision ? <Badge>默认视觉</Badge> : null}
              {defaultImage ? <Badge>默认图像</Badge> : null}
            </span>
            <span className="model-card-sub">
              <span className="model-card-id">{model.id}</span>
              <span className="model-card-dot" />
              <span>
                {model.contextLength ? `${context} 上下文` : "上下文未设置"}
              </span>
              <span className="model-card-dot" />
              <span>{SOURCE_LABELS[model.source] ?? model.source}</span>
            </span>
          </span>
          <span className="model-card-chips">
            {summaryCaps.map((key) => {
              const state = model.capabilities[key];
              return (
                <span
                  key={key}
                  className={clsx("model-chip", state)}
                  title={`${capabilityLabel(key)}：${capabilityStateLabel(state)}`}
                >
                  {capabilityLabel(key)}
                </span>
              );
            })}
          </span>
          <span className={clsx("model-card-chevron", expanded && "open")}>
            <ChevronDown size={14} />
          </span>
        </button>
        <IconButton
          label={`删除模型 ${model.name || model.id}`}
          size="sm"
          onClick={onRemove}
        >
          <Trash2 size={12} />
        </IconButton>
      </div>

      {expanded ? (
        <div className="model-card-body">
          <div className="model-fields">
            <Field label="模型 ID">
              <Input
                className="mono"
                value={model.id}
                onChange={(event) => onChange({ id: event.target.value })}
              />
            </Field>
            <Field label="显示名称">
              <Input
                value={model.name}
                onChange={(event) => onChange({ name: event.target.value })}
              />
            </Field>
          </div>

          <div className="field">
            <span className="field-label">上下文窗口</span>
            <div className="context-control">
              <div className="context-input">
                <Input
                  type="number"
                  min={1024}
                  step={1024}
                  className="mono"
                  placeholder="例如 128000"
                  value={model.contextLength ?? ""}
                  onChange={(event) =>
                    onChange({
                      contextLength: event.target.value
                        ? Number(event.target.value)
                        : null,
                    })
                  }
                />
                <span className="context-unit">tokens</span>
              </div>
              <div className="context-presets">
                {CONTEXT_PRESETS.map((preset) => (
                  <button
                    key={preset.label}
                    type="button"
                    className={clsx(
                      "context-chip",
                      context === preset.label && "active",
                    )}
                    onClick={() => onChange({ contextLength: preset.value })}
                  >
                    {preset.label}
                  </button>
                ))}
              </div>
              <span className="context-readout">
                {model.contextLength ? `≈ ${context}` : "未设置"}
              </span>
            </div>
            <span className="field-hint">
              用于计算上下文占用和长上下文能力；留空表示未知。
            </span>
          </div>

          <div className="cap-panel">
            <div className="cap-panel-head">
              <div className="cap-panel-copy">
                <span className="cap-panel-title">能力状态</span>
                <span className="field-hint">
                  不确定时选「未验证」，真实调用后会更新。
                </span>
              </div>
              <div className="cap-bulk-actions">
                <button
                  type="button"
                  onClick={() => onBulkCaps("verified")}
                >
                  全部可用
                </button>
                <button
                  type="button"
                  onClick={() => onBulkCaps("unverified")}
                >
                  全部未验证
                </button>
              </div>
            </div>
            <div className="cap-groups">
              {groups.map((group) => (
                <div className="cap-group" key={group.title}>
                  <div className="cap-group-title">{group.title}</div>
                  {group.keys.map((key) => {
                    const state = model.capabilities[key];
                    return (
                      <label className="cap-row" key={key}>
                        <span className="cap-meta">
                          <span className="cap-name">
                            {capabilityLabel(key)}
                          </span>
                          <span className="cap-desc">{CAP_HINTS[key]}</span>
                        </span>
                        <select
                          className="cap-select"
                          data-state={state}
                          value={state}
                          onChange={(event) =>
                            onCapability(
                              key,
                              event.target.value as CapState,
                            )
                          }
                        >
                          {CAP_STATES.map((item) => (
                            <option value={item} key={item}>
                              {capabilityStateLabel(item)}
                            </option>
                          ))}
                        </select>
                      </label>
                    );
                  })}
                </div>
              ))}
            </div>
          </div>

          <div className="model-defaults">
            <div>
              <div className="field-label">默认用途</div>
              <div className="field-hint">
                选中后优先用于新会话或生成任务；再次点击可取消。
              </div>
            </div>
            <div className="model-default-actions">
              {category === "chat" ? (
                <>
                  <button
                    type="button"
                    className={clsx(
                      "model-default-btn",
                      defaultChat && "active",
                    )}
                    onClick={() => onDefault("defaultChatModel")}
                  >
                    {defaultChat ? (
                      <Check size={11} />
                    ) : (
                      <Star size={11} />
                    )}
                    默认对话
                  </button>
                  <button
                    type="button"
                    className={clsx(
                      "model-default-btn",
                      defaultVision && "active",
                    )}
                    onClick={() => onDefault("defaultVisionModel")}
                  >
                    {defaultVision ? (
                      <Check size={11} />
                    ) : (
                      <Star size={11} />
                    )}
                    默认视觉
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  className={clsx(
                    "model-default-btn",
                    defaultImage && "active",
                  )}
                  onClick={() => onDefault("defaultImageModel")}
                >
                  {defaultImage ? <Check size={11} /> : <Star size={11} />}
                  默认图像
                </button>
              )}
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function TemplatePanel({
  provider,
  onUse,
}: {
  provider: Provider;
  onUse: () => void;
}) {
  return (
    <div className="provider-form">
      <div className="form-actions">
        <div>
          <div className="section-label">PROVIDER TEMPLATE</div>
          <div style={{ fontSize: 16, fontWeight: 600, marginTop: 2 }}>
            {provider.name}
          </div>
        </div>
        <span className="spacer" />
        <Button variant="primary" onClick={onUse}>
          <Plus size={12} /> 使用模板创建供应商
        </Button>
      </div>
      <section className="form-section">
        <div className="form-section-head">模板信息</div>
        <div className="form-section-body">
          <div className="form-grid">
            <Field label="分类">
              <Input readOnly value={provider.category === "image" ? "图像生成" : "对话 / Agent"} />
            </Field>
            <Field label="协议">
              <Input readOnly value={providerProtocolLabel(provider.protocol)} />
            </Field>
            <Field label="推荐 Base URL" hint="模板 Base URL 可随意修改，部分兼容接口需要自带 /v1。">
              <Input readOnly className="mono" value={provider.baseUrl || "（需要手动填写）"} />
            </Field>
            <Field label="默认模型">
              <Input
                readOnly
                value={provider.defaultChatModel ?? provider.defaultImageModel ?? "拉取模型后设置"}
              />
            </Field>
          </div>
          <div className="warning-box">
            模板只是初始配置，不会自动验证能力。使用模板创建供应商后，请填写 API Key，
            保存并执行连接测试；模型能力需要真实调用或你的明确标记。
          </div>
        </div>
      </section>
      <section className="form-section">
        <div className="form-section-head">说明</div>
        <div className="form-section-body" style={{ fontSize: 11.5, color: "var(--fg-2)", lineHeight: 1.65 }}>
          {templateDescription(provider.protocol)}
        </div>
      </section>
    </div>
  );
}

function templateDescription(protocol: string): string {
  switch (protocol) {
    case "openai_chat":
      return "OpenAI Chat Completions 协议，同时适用于大量兼容接口。支持流式文本、函数工具调用与图片输入（取决于实际服务能力）。";
    case "openai_responses":
      return "OpenAI Responses API，独立解析 response.output_text、function_call 参数与 usage 事件，不使用 Chat Completions 转换。";
    case "anthropic":
      return "Anthropic Messages API，独立处理 system、tool_use / tool_result、content blocks 与 SSE 事件。默认使用 4096 max_tokens，可通过自定义参数覆盖。";
    case "gemini":
      return "Google Gemini generateContent / streamGenerateContent，独立转换 contents、functionCall、inlineData 与系统指令。";
    case "ollama":
      return "Ollama 本地模型服务，使用 /api/chat 与 /api/tags。Base URL 填根地址（例如 http://127.0.0.1:11434），不需要 /api 后缀。";
    case "openai_images":
      return "OpenAI Images 协议，支持文本生图与图片编辑。参数尺寸、质量、背景、蒙版选项由模型能力矩阵严格校验。";
    case "gemini_images":
      return "Gemini 图像生成使用 generateContent 的 IMAGE 模态，支持单张参考图编辑；不支持蒙版与异步任务。";
    default:
      return "请参考供应商官方文档配置协议与 Base URL。";
  }
}
