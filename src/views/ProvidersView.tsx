import { useEffect, useState } from "react";
import {
  Copy,
  KeyRound,
  Plus,
  RefreshCw,
  Save,
  ShieldCheck,
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
  defaultModelFor,
  providerProtocolLabel,
} from "../lib/format";
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
  const [modelDraft, setModelDraft] = useState({ id: "", name: "" });
  const [deleteTarget, setDeleteTarget] = useState<Provider | null>(null);

  const selected = providers.find((item) => item.id === selectedId) ?? null;
  const templates = providers.filter((provider) => provider.isTemplate);
  const configured = providers.filter((provider) => !provider.isTemplate);

  const loadProvider = (provider: Provider) => {
    setSelectedId(provider.id);
    setTestResult(null);
    setApiKey("");
    setClearKey(false);
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
    return {
      ...form,
      name: form.name.trim(),
      baseUrl: form.baseUrl.trim(),
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
      const models = current.models.map((model, i) =>
        i === index ? { ...model, ...patch } : model,
      );
      return { ...current, models };
    });
  };

  const updateCapability = (
    index: number,
    key: (typeof CAPABILITIES)[number],
    value: CapState,
  ) => {
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
    const capabilities =
      form.category === "image" ? imageCaps() : chatCaps(form.stream);
    const model: ProviderModel = {
      id,
      name: modelDraft.name.trim() || id,
      capabilities,
      source: "manual",
    };
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
    setModelDraft({ id: "", name: "" });
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
                              updateForm({
                                category,
                                protocol:
                                  category === "image"
                                    ? "openai_images"
                                    : "openai_chat",
                                defaultChatModel: null,
                                defaultVisionModel: null,
                                defaultImageModel: null,
                              });
                            }}
                          >
                            <option value="chat">对话 / Agent</option>
                            <option value="image">图像生成</option>
                          </Select>
                        </Field>
                        <Field label="协议">
                          <Select
                            value={form.protocol}
                            onChange={(event) =>
                              updateForm({ protocol: event.target.value })
                            }
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
                          hint="需要包含版本路径，例如 https://api.openai.com/v1。Ollama 填根地址。"
                        >
                          <Input
                            className="mono"
                            value={form.baseUrl}
                            onChange={(event) =>
                              updateForm({ baseUrl: event.target.value })
                            }
                            placeholder={
                              form.protocol === "ollama"
                                ? "http://127.0.0.1:11434"
                                : "https://…/v1"
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
                      模型与能力矩阵
                      <span className="right">
                        <span className="mono" style={{ fontSize: 9, color: "var(--fg-3)" }}>
                          {form.models.length} MODELS
                        </span>
                      </span>
                    </div>
                    <div className="form-section-body">
                      <div className="model-add">
                        <Input
                          className="mono"
                          placeholder="模型 ID，例如 gpt-4.1"
                          value={modelDraft.id}
                          onChange={(event) =>
                            setModelDraft((current) => ({
                              ...current,
                              id: event.target.value,
                            }))
                          }
                        />
                        <Input
                          placeholder="显示名称（可选）"
                          value={modelDraft.name}
                          onChange={(event) =>
                            setModelDraft((current) => ({
                              ...current,
                              name: event.target.value,
                            }))
                          }
                        />
                        <Button onClick={addModel}>
                          <Plus size={12} /> 添加模型
                        </Button>
                      </div>
                      <div className="model-list">
                        {form.models.length === 0 ? (
                          <div className="field-hint">
                            还没有模型。可手动添加，或保存后点击「测试连接 / 拉取模型」从接口获取。
                          </div>
                        ) : null}
                        {form.models.map((model, index) => (
                          <div className="model-row" key={`${model.id}-${index}`}>
                            <div className="model-row-head">
                              <Input
                                className="mono"
                                value={model.id}
                                onChange={(event) =>
                                  updateModel(index, { id: event.target.value })
                                }
                                style={{ minHeight: 24, height: 24, padding: "2px 6px" }}
                              />
                              <Input
                                value={model.name}
                                onChange={(event) =>
                                  updateModel(index, { name: event.target.value })
                                }
                                style={{ minHeight: 24, height: 24, padding: "2px 6px" }}
                              />
                              <Badge>{model.source}</Badge>
                              <Input
                                type="number"
                                min={1}
                                className="mono"
                                title="上下文窗口（tokens）"
                                placeholder="ctx"
                                value={model.contextLength ?? ""}
                                onChange={(event) =>
                                  updateModel(index, {
                                    contextLength: event.target.value
                                      ? Number(event.target.value)
                                      : null,
                                  })
                                }
                                style={{
                                  width: 78,
                                  height: 24,
                                  minHeight: 24,
                                  padding: "2px 6px",
                                  fontSize: 10,
                                }}
                              />
                              <IconButton
                                label="删除模型"
                                size="sm"
                                onClick={() => removeModel(index)}
                              >
                                <Trash2 size={11} />
                              </IconButton>
                            </div>
                            <div className="cap-grid">
                              {CAPABILITIES.map((capability) => (
                                <label className="cap-item" key={capability}>
                                  <span className="cap-label">
                                    {capabilityLabel(capability)}
                                  </span>
                                  <select
                                    data-state={
                                      model.capabilities[capability]
                                    }
                                    value={model.capabilities[capability]}
                                    onChange={(event) =>
                                      updateCapability(
                                        index,
                                        capability,
                                        event.target.value as CapState,
                                      )
                                    }
                                  >
                                    {CAP_STATES.map((state) => (
                                      <option value={state} key={state}>
                                        {capabilityStateLabel(state)}
                                      </option>
                                    ))}
                                  </select>
                                </label>
                              ))}
                            </div>
                          </div>
                        ))}
                      </div>
                      {form.models.length > 0 ? (
                        <div className="form-grid three">
                          {form.category === "chat" ? (
                            <>
                              <Field label="默认聊天模型">
                                <Select
                                  value={form.defaultChatModel ?? ""}
                                  onChange={(event) =>
                                    updateForm({
                                      defaultChatModel: event.target.value || null,
                                    })
                                  }
                                >
                                  <option value="">未设置</option>
                                  {form.models.map((model) => (
                                    <option value={model.id} key={model.id}>
                                      {model.id}
                                    </option>
                                  ))}
                                </Select>
                              </Field>
                              <Field label="默认视觉模型">
                                <Select
                                  value={form.defaultVisionModel ?? ""}
                                  onChange={(event) =>
                                    updateForm({
                                      defaultVisionModel: event.target.value || null,
                                    })
                                  }
                                >
                                  <option value="">未设置</option>
                                  {form.models.map((model) => (
                                    <option value={model.id} key={model.id}>
                                      {model.id}
                                    </option>
                                  ))}
                                </Select>
                              </Field>
                            </>
                          ) : (
                            <Field label="默认图像模型">
                              <Select
                                value={form.defaultImageModel ?? ""}
                                onChange={(event) =>
                                  updateForm({
                                    defaultImageModel: event.target.value || null,
                                  })
                                }
                              >
                                <option value="">未设置</option>
                                {form.models.map((model) => (
                                  <option value={model.id} key={model.id}>
                                    {model.id}
                                  </option>
                                ))}
                              </Select>
                            </Field>
                          )}
                        </div>
                      ) : null}
                      <div className="field-hint">
                        能力状态由接口元数据或真实调用验证，不会因为名称包含 GPT / Claude /
                        Gemini 就默认可用。未验证能力可在 Agent 或图像工作区试用后自动标记。
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
