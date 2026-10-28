import { useEffect, useMemo, useState } from "react";
import {
  Download,
  FolderOpen,
  Image as ImageIcon,
  Pencil,
  Plus,
  RefreshCw,
  RotateCcw,
  Server,
  Trash2,
  Upload,
} from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import clsx from "clsx";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../lib/store";
import { formatDateTime } from "../lib/format";
import type {
  Attachment,
  ImageCapabilities,
  ImageFile,
  ImageGeneration,
} from "../lib/types";
import {
  Badge,
  Button,
  ConfirmDialog,
  CopyButton,
  Field,
  IconButton,
  Input,
  Modal,
  Select,
  Textarea,
} from "../components/ui";
import { TitlebarDrag } from "../components/WindowControls";

function ImageFromPath({
  path,
  alt,
  className,
}: {
  path: string;
  alt: string;
  className?: string;
}) {
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let alive = true;
    setSrc(null);
    setFailed(false);
    api
      .readMedia(path)
      .then((url) => alive && setSrc(url))
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [path]);
  if (failed) {
    return (
      <div className={className} style={{ display: "grid", placeItems: "center", color: "var(--fg-3)", fontSize: 11 }}>
        图片不可用
      </div>
    );
  }
  if (!src) {
    return (
      <div className={className} style={{ display: "grid", placeItems: "center" }}>
        <span className="spinner" />
      </div>
    );
  }
  return <img className={className} src={src} alt={alt} />;
}

interface ReferenceFile {
  path: string;
  name: string;
}

export function ImageStudio() {
  const providers = useAppStore((state) => state.providers);
  const settings = useAppStore((state) => state.settings);
  const history = useAppStore((state) => state.imageHistory);
  const progress = useAppStore((state) => state.imageProgress);
  const refreshHistory = useAppStore((state) => state.refreshImageHistory);
  const setView = useAppStore((state) => state.setView);
  const notify = useAppStore((state) => state.notify);
  const dataDir = useAppStore((state) => state.dataDir);

  const imageProviders = useMemo(
    () =>
      providers.filter(
        (provider) => provider.category === "image" && provider.enabled && !provider.isTemplate,
      ),
    [providers],
  );
  const [providerId, setProviderId] = useState("");
  const [model, setModel] = useState("");
  const [mode, setMode] = useState<"generate" | "edit">("generate");
  const [prompt, setPrompt] = useState("");
  const [size, setSize] = useState("");
  const [quality, setQuality] = useState("");
  const [count, setCount] = useState(1);
  const [background, setBackground] = useState("");
  const [reference, setReference] = useState<ReferenceFile | null>(null);
  const [mask, setMask] = useState<ReferenceFile | null>(null);
  const [caps, setCaps] = useState<ImageCapabilities | null>(null);
  const [capsLoading, setCapsLoading] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [preview, setPreview] = useState<{ path: string; title: string } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ImageGeneration | null>(null);
  const [error, setError] = useState<string | null>(null);

  const provider = imageProviders.find((item) => item.id === providerId) ?? null;
  const modelInfo = provider?.models.find((item) => item.id === model) ?? null;

  useEffect(() => {
    void refreshHistory();
  }, [refreshHistory]);

  useEffect(() => {
    if (imageProviders.length === 0) {
      setProviderId("");
      setModel("");
      return;
    }
    const current = imageProviders.find((item) => item.id === providerId);
    const preferred =
      current ??
      imageProviders.find((item) => item.id === settings.routing.image.providerId) ??
      imageProviders[0];
    if (!current) {
      setProviderId(preferred.id);
      setModel(
        preferred.defaultImageModel ||
          preferred.models[0]?.id ||
          "",
      );
    }
  }, [imageProviders, providerId, settings.routing.image.providerId]);

  useEffect(() => {
    if (!providerId || !model) {
      setCaps(null);
      return;
    }
    let alive = true;
    setCapsLoading(true);
    setCaps(null);
    api
      .imageCapabilities(providerId, model)
      .then((capabilities) => {
        if (!alive) return;
        setCaps(capabilities);
        setSize(capabilities.sizes[0] ?? "");
        setQuality(capabilities.qualities[0] ?? "");
        setCount(1);
        setBackground("");
        if (mode === "edit" && !capabilities.edit) setMode("generate");
        if (!capabilities.referenceImages) setReference(null);
        if (!capabilities.mask) setMask(null);
      })
      .catch((err) => {
        if (alive) {
          setCaps(null);
          setError(errorMessage(err));
        }
      })
      .finally(() => alive && setCapsLoading(false));
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providerId, model]);

  const pickReference = async (kind: "reference" | "mask") => {
    try {
      const selection = await open({
        multiple: false,
        title: kind === "reference" ? "选择参考图片" : "选择蒙版图片",
        filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp"] }],
      });
      const path = Array.isArray(selection) ? selection[0] : selection;
      if (!path) return;
      const attachment: Attachment = await api.attachmentImport(path);
      if (attachment.kind !== "image") {
        notify("warning", "请选择图片文件");
        return;
      }
      if (kind === "reference") {
        setReference({ path: attachment.path, name: attachment.name });
      } else {
        setMask({ path: attachment.path, name: attachment.name });
      }
    } catch (err) {
      notify("error", "导入图片失败", errorMessage(err));
    }
  };

  const validate = (): string | null => {
    if (!providerId || !model.trim()) return "请选择图像供应商与模型";
    if (!prompt.trim()) return "提示词不能为空";
    if (!caps) return "正在加载模型能力，请稍候";
    if (mode === "edit" && !reference) return "图像编辑需要一张参考图片";
    if (mode === "edit" && !caps.edit) return "当前模型不支持图像编辑";
    if (mode === "generate" && !caps.generate) return "当前模型不支持文本生图";
    return null;
  };

  const runGeneration = async (overrides?: {
    providerId?: string;
    model?: string;
    prompt?: string;
    mode?: "generate" | "edit";
    size?: string;
    quality?: string;
    count?: number;
    background?: string;
    reference?: ReferenceFile | null;
    mask?: ReferenceFile | null;
  }) => {
    const targetProviderId = overrides?.providerId ?? providerId;
    const targetModel = overrides?.model ?? model;
    const targetPrompt = overrides?.prompt ?? prompt;
    const targetMode = overrides?.mode ?? mode;
    const targetSize = overrides?.size ?? size;
    const targetQuality = overrides?.quality ?? quality;
    const targetCount = overrides?.count ?? count;
    const targetBackground = overrides?.background ?? background;
    const targetReference = overrides?.reference === undefined ? reference : overrides.reference;
    const targetMask = overrides?.mask === undefined ? mask : overrides.mask;
    const problem = validate();
    if (problem && !overrides) {
      notify("warning", problem);
      return;
    }
    if (targetMode === "edit" && !targetReference) {
      notify("warning", "图像编辑需要参考图片");
      return;
    }
    setGenerating(true);
    setError(null);
    try {
      const generation = await api.imageGenerate({
        providerId: targetProviderId,
        model: targetModel,
        prompt: targetPrompt,
        mode: targetMode,
        size: targetSize || null,
        quality: targetQuality || null,
        count: targetCount,
        background: targetBackground || null,
        referencePaths: targetReference ? [targetReference.path] : [],
        maskPath: targetMask?.path ?? null,
      });
      await refreshHistory();
      notify(
        "success",
        "图像生成完成",
        `${generation.providerName} / ${generation.model}`,
      );
    } catch (err) {
      const message = errorMessage(err);
      setError(message);
      notify("error", "图像生成失败", message);
      await refreshHistory();
    } finally {
      setGenerating(false);
    }
  };

  const reuse = (generation: ImageGeneration) => {
    setProviderId(generation.providerId);
    setModel(generation.model);
    setPrompt(generation.prompt);
    setMode(generation.mode === "edit" ? "edit" : "generate");
    const params = generation.params as Record<string, unknown>;
    setSize(typeof params.size === "string" ? params.size : "");
    setQuality(typeof params.quality === "string" ? params.quality : "");
    setCount(typeof params.count === "number" ? params.count : 1);
    setBackground(typeof params.background === "string" ? params.background : "");
    window.scrollTo({ top: 0, behavior: "smooth" });
  };

  const continueEdit = (generation: ImageGeneration) => {
    if (!generation.files[0]) return;
    setMode("edit");
    setPrompt(generation.prompt);
    setReference({ path: generation.files[0].path, name: generation.files[0].name });
    notify("info", "已载入生成图作为参考图片，输入新提示词后继续编辑");
  };

  const saveFile = async (file: ImageFile) => {
    const extension = file.name.split(".").pop() || "png";
    try {
      const target = await save({
        defaultPath: file.name,
        filters: [{ name: "Image", extensions: [extension] }],
      });
      if (!target) return;
      await api.imageSaveAs(file.path, target);
      notify("success", "图片已保存", target);
    } catch (err) {
      notify("error", "保存图片失败", errorMessage(err));
    }
  };

  const items = useMemo(
    () =>
      history.flatMap((generation) =>
        generation.files.length > 0
          ? generation.files.map((file) => ({ generation, file }))
          : [{ generation, file: null as ImageFile | null }],
      ),
    [history],
  );

  const busyGeneration = Object.values(progress).find(
    (generation) => generation.status === "running",
  );

  return (
    <div className="view">
      <div className="page">
        <header className="page-head">
          <div>
            <div className="page-kicker">IMAGE PROVIDER STUDIO</div>
            <h1 className="page-title">图像工作区</h1>
            <div className="page-sub">
              独立的图像生成与编辑服务，不依赖聊天模型与 Agent Runtime
            </div>
          </div>
          <TitlebarDrag />
          <div className="page-head-actions">
            <Button onClick={() => void refreshHistory()}>
              <RefreshCw size={12} /> 刷新历史
            </Button>
            <Button onClick={() => void api.openPath(`${dataDir}\\images`).catch((err) => notify("error", "无法打开目录", errorMessage(err)))}>
              <FolderOpen size={12} /> 图像目录
            </Button>
            <Button variant="primary" onClick={() => setView("providers")}>
              <Server size={12} /> 管理图像供应商
            </Button>
          </div>
        </header>
        <div className="page-body">
          <div className="studio-layout">
            <div className="studio-controls">
              {imageProviders.length === 0 ? (
                <div className="studio-empty">
                  <ImageIcon size={24} style={{ opacity: 0.5, marginBottom: 8 }} />
                  <div>还没有可用的图像供应商。</div>
                  <div style={{ marginTop: 6 }}>
                    在供应商页面配置 OpenAI Images 或 Gemini Images。
                  </div>
                  <Button
                    variant="primary"
                    style={{ marginTop: 12 }}
                    onClick={() => setView("providers")}
                  >
                    打开供应商管理
                  </Button>
                </div>
              ) : (
                <>
                  <div className="form-grid">
                    <Field label="图像供应商">
                      <Select
                        value={providerId}
                        onChange={(event) => {
                          const id = event.target.value;
                          setProviderId(id);
                          const next = imageProviders.find((item) => item.id === id);
                          setModel(next?.defaultImageModel || next?.models[0]?.id || "");
                        }}
                      >
                        {imageProviders.map((item) => (
                          <option key={item.id} value={item.id}>
                            {item.name}
                          </option>
                        ))}
                      </Select>
                    </Field>
                    <Field label="图像模型" hint={modelInfo?.name}>
                      <Input
                        className="mono"
                        list="basalt-image-models"
                        value={model}
                        onChange={(event) => setModel(event.target.value)}
                        placeholder="图像模型 ID"
                      />
                      <datalist id="basalt-image-models">
                        {provider?.models.map((item) => (
                          <option key={item.id} value={item.id}>
                            {item.name}
                          </option>
                        ))}
                      </datalist>
                    </Field>
                  </div>

                  <div className="segmented" style={{ alignSelf: "flex-start" }}>
                    <button
                      className={mode === "generate" ? "active" : ""}
                      onClick={() => setMode("generate")}
                      disabled={Boolean(caps && !caps.generate)}
                    >
                      文本生图
                    </button>
                    <button
                      className={mode === "edit" ? "active" : ""}
                      onClick={() => setMode("edit")}
                      disabled={Boolean(caps && !caps.edit)}
                    >
                      图像编辑
                    </button>
                  </div>

                  <Field label="提示词">
                    <Textarea
                      rows={6}
                      value={prompt}
                      onChange={(event) => setPrompt(event.target.value)}
                      placeholder="描述画面、风格、构图、光线与限制条件…"
                    />
                  </Field>

                  {mode === "edit" ? (
                    <Field
                      label="参考图片"
                      hint={
                        caps?.referenceImages
                          ? "仅支持一张参考图片；会先复制到工作台数据目录，并在生成时发送给当前图像供应商。"
                          : "当前模型不支持参考图片"
                      }
                    >
                      {reference ? (
                        <div className="studio-ref" style={{ marginTop: 4 }}>
                          <ImageFromPath
                            path={reference.path}
                            alt={reference.name}
                            className="studio-ref-img"
                          />
                          <div className="info">
                            <div className="truncate">{reference.name}</div>
                            <div style={{ marginTop: 6 }}>
                              <Button size="xs" onClick={() => setReference(null)}>
                                移除
                              </Button>
                            </div>
                          </div>
                        </div>
                      ) : (
                        <Button
                          onClick={() => void pickReference("reference")}
                          disabled={!caps?.referenceImages}
                        >
                          <Upload size={12} /> 选择参考图片
                        </Button>
                      )}
                    </Field>
                  ) : null}

                  {mode === "edit" && caps?.mask ? (
                    <Field label="蒙版（可选）">
                      {mask ? (
                        <div className="inline-flex">
                          <span className="mono" style={{ fontSize: 10 }}>
                            {mask.name}
                          </span>
                          <Button size="xs" onClick={() => setMask(null)}>
                            移除
                          </Button>
                        </div>
                      ) : (
                        <Button size="sm" onClick={() => void pickReference("mask")}>
                          <Upload size={12} /> 选择蒙版
                        </Button>
                      )}
                    </Field>
                  ) : null}

                  <div className="form-grid three">
                    {caps?.sizes.length ? (
                      <Field label="尺寸">
                        <Select value={size} onChange={(event) => setSize(event.target.value)}>
                          {caps.sizes.map((option) => (
                            <option key={option} value={option}>
                              {option}
                            </option>
                          ))}
                        </Select>
                      </Field>
                    ) : null}
                    {caps?.qualities.length ? (
                      <Field label="质量">
                        <Select
                          value={quality}
                          onChange={(event) => setQuality(event.target.value)}
                        >
                          {caps.qualities.map((option) => (
                            <option key={option} value={option}>
                              {option}
                            </option>
                          ))}
                        </Select>
                      </Field>
                    ) : null}
                    {caps?.multiple ? (
                      <Field label="数量">
                        <Input
                          type="number"
                          min={1}
                          max={caps.maxCount}
                          value={count}
                          onChange={(event) =>
                            setCount(
                              Math.max(
                                1,
                                Math.min(caps.maxCount, Number(event.target.value) || 1),
                              ),
                            )
                          }
                        />
                      </Field>
                    ) : null}
                    {caps?.transparentBackground && mode === "generate" ? (
                      <Field label="背景">
                        <Select
                          value={background}
                          onChange={(event) => setBackground(event.target.value)}
                        >
                          <option value="">默认</option>
                          <option value="opaque">不透明</option>
                          <option value="transparent">透明</option>
                        </Select>
                      </Field>
                    ) : null}
                  </div>

                  {capsLoading ? (
                    <div className="studio-status">
                      <span className="inline-flex">
                        <span className="spinner" /> 正在读取模型能力…
                      </span>
                    </div>
                  ) : caps ? (
                    <div className="studio-status">
                      <div className="inline-flex" style={{ marginBottom: 4 }}>
                        <Badge tone="ok">CAPABILITIES</Badge>
                        <span className="mono" style={{ fontSize: 9, color: "var(--fg-3)" }}>
                          {caps.maxCount} MAX · {caps.asyncTasks ? "异步" : "同步"}
                        </span>
                      </div>
                      {caps.notes.map((note) => (
                        <div key={note}>· {note}</div>
                      ))}
                    </div>
                  ) : null}

                  {error ? <div className="timeline-error">{error}</div> : null}

                  {busyGeneration ? (
                    <div className="studio-status">
                      <span className="inline-flex">
                        <span className="spinner" />
                        正在生成：{busyGeneration.prompt.slice(0, 60)}…
                      </span>
                    </div>
                  ) : null}

                  <div className="studio-actions">
                    <Button
                      id="image-generate-btn"
                      variant="primary"
                      className="block"
                      loading={generating}
                      onClick={() => void runGeneration()}
                      disabled={!caps || capsLoading}
                    >
                      <ImageIcon size={13} />
                      {mode === "edit" ? "应用编辑" : "生成图像"}
                    </Button>
                    <IconButton
                      label="清空提示词"
                      onClick={() => setPrompt("")}
                    >
                      <RotateCcw size={14} />
                    </IconButton>
                  </div>
                  <div className="field-hint">
                    参数严格来自当前模型能力；不支持的能力会被禁用或隐藏，不会静默忽略。
                    图片保存在本地工作台数据目录，可随时删除。
                  </div>
                </>
              )}
            </div>

            <div className="studio-gallery">
              <div className="row" style={{ marginBottom: 12 }}>
                <span className="section-label">GENERATION HISTORY</span>
                <span className="mono" style={{ fontSize: 9, color: "var(--fg-3)" }}>
                  {history.length} RECORDS
                </span>
                <span style={{ marginLeft: "auto" }} />
              </div>
              {items.length === 0 ? (
                <div className="studio-empty">
                  <ImageIcon size={22} style={{ opacity: 0.5, marginBottom: 8 }} />
                  还没有生成记录。选择供应商与模型，输入提示词开始创作。
                </div>
              ) : (
                <div className="image-grid-gallery">
                  {items.map(({ generation, file }) => (
                    <article className="gen-card" key={`${generation.id}-${file?.path ?? "failed"}`}>
                      {file ? (
                        <div
                          className="gen-thumb"
                          onClick={() => setPreview({ path: file.path, title: file.name })}
                        >
                          <ImageFromPath path={file.path} alt={file.name} />
                        </div>
                      ) : (
                        <div className="gen-thumb" style={{ cursor: "default" }}>
                          <div style={{ padding: 12, textAlign: "center", color: "var(--err)", fontSize: 11 }}>
                            生成失败
                            <div style={{ marginTop: 4, color: "var(--fg-3)", fontSize: 10 }}>
                              {generation.error?.slice(0, 160)}
                            </div>
                          </div>
                        </div>
                      )}
                      <div className="gen-meta">
                        <div className="row" style={{ gap: 5, flexWrap: "wrap" }}>
                          <Badge>{generation.mode === "edit" ? "EDIT" : "GEN"}</Badge>
                          <Badge>{generation.status}</Badge>
                          {file ? <Badge>{file.mime.replace("image/", "").toUpperCase()}</Badge> : null}
                        </div>
                        <div className="p" title={generation.prompt}>
                          {generation.prompt}
                        </div>
                        <div className="row" style={{ gap: 6 }}>
                          <span className="truncate" style={{ maxWidth: 130 }}>
                            {generation.providerName}
                          </span>
                          <span className="truncate" style={{ maxWidth: 100 }}>
                            {generation.model}
                          </span>
                          <span style={{ marginLeft: "auto" }}>
                            {formatDateTime(generation.createdAt)}
                          </span>
                        </div>
                        <div className="actions">
                          {file ? (
                            <>
                              <Button size="xs" onClick={() => void saveFile(file)}>
                                <Download size={10} /> 保存
                              </Button>
                              <Button
                                size="xs"
                                onClick={() =>
                                  void runGeneration({
                                    providerId: generation.providerId,
                                    model: generation.model,
                                    prompt: generation.prompt,
                                    mode: generation.mode === "edit" ? "edit" : "generate",
                                    size: typeof generation.params.size === "string" ? generation.params.size : "",
                                    quality:
                                      typeof generation.params.quality === "string"
                                        ? generation.params.quality
                                        : "",
                                    count:
                                      typeof generation.params.count === "number"
                                        ? generation.params.count
                                        : 1,
                                    reference:
                                      generation.mode === "edit" && file
                                        ? { path: file.path, name: file.name }
                                        : null,
                                  })
                                }
                                disabled={generating}
                              >
                                <RefreshCw size={10} /> 重生成
                              </Button>
                              <Button
                                size="xs"
                                onClick={() => continueEdit(generation)}
                                disabled={!file}
                                title="将结果作为参考图继续编辑"
                              >
                                <Pencil size={10} /> 编辑
                              </Button>
                            </>
                          ) : null}
                          <CopyButton text={generation.prompt} label="提示词" />
                          <IconButton
                            label="删除生成记录"
                            size="sm"
                            onClick={() => setDeleteTarget(generation)}
                          >
                            <Trash2 size={10} />
                          </IconButton>
                        </div>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      <Modal
        open={preview !== null}
        onClose={() => setPreview(null)}
        kicker="GENERATION PREVIEW"
        title={preview?.title ?? ""}
        size="xl"
      >
        {preview ? (
          <ImageFromPath
            path={preview.path}
            alt={preview.title}
            className="preview-image"
          />
        ) : null}
      </Modal>

      <ConfirmDialog
        open={deleteTarget !== null}
        title="删除生成记录"
        message={
          <>
            将删除 <strong>{deleteTarget?.prompt.slice(0, 60)}</strong> 的本地图片文件与记录。
            已保存到其他位置的图片不受影响。
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
            .imageDelete(id)
            .then(async () => {
              await refreshHistory();
              notify("success", "生成记录已删除");
            })
            .catch((err) => notify("error", "删除失败", errorMessage(err)));
        }}
      />
    </div>
  );
}
