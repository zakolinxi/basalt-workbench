import { useState, type ReactNode } from "react";
import { FileText, RotateCcw } from "lucide-react";
import clsx from "clsx";
import { chooseSavePath, downloadDataUrl } from "../lib/platform";
import { isRemote } from "../lib/bridge";
import { api, errorMessage } from "../lib/api";
import { useAppStore } from "../lib/store";
import type { Attachment, Message } from "../lib/types";
import { formatTime } from "../lib/format";
import { Button, CopyButton, IconButton, MediaThumb } from "./ui";
import { Markdown } from "./Markdown";

function ImageActions({ images }: { images: Attachment[] }) {
  const setView = useAppStore((state) => state.setView);
  const notify = useAppStore((state) => state.notify);
  const [busy, setBusy] = useState(false);
  const download = async () => {
    const image = images[0];
    if (!image) return;
    setBusy(true);
    try {
      const extension = image.name.split(".").pop() || "png";
      if (isRemote()) {
        downloadDataUrl(image.name, await api.readMedia(image.path));
        notify("success", "图片已保存到手机", image.name);
        return;
      }
      const target = await chooseSavePath(image.name, [extension]);
      if (!target) return;
      await api.imageSaveAs(image.path, target);
      notify("success", "图片已保存", target);
    } catch (error) {
      notify("error", "保存图片失败", errorMessage(error));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="msg-attachments">
      <Button size="xs" onClick={() => void download()} loading={busy}>
        保存图片
      </Button>
      <Button size="xs" onClick={() => setView("images")}>
        图像工作区
      </Button>
      <span className="mono" style={{ fontSize: 9, color: "var(--fg-3)" }}>
        可保存、查看历史并继续编辑
      </span>
    </div>
  );
}

export function MessageItem({
  message,
  onRetry,
  canRetry,
  footer,
}: {
  message: Message;
  onRetry?: () => void;
  canRetry?: boolean;
  footer?: ReactNode;
}) {
  const isUser = message.role === "user";
  const images = message.attachments.filter((attachment) => attachment.kind === "image");
  const files = message.attachments.filter((attachment) => attachment.kind !== "image");
  return (
    <article className={clsx("msg", isUser ? "user" : "assistant")}>
      <header className={clsx("msg-head", isUser && "user-meta")}>
        {isUser ? null : (
          <span className="msg-role">
            <span className="role-mark">B</span>
            Basalt
            {message.model ? (
              <span className="msg-model">{message.model}</span>
            ) : null}
          </span>
        )}
        <span className="msg-meta">
          {formatTime(message.createdAt)}
          <CopyButton text={message.content} label="复制" />
        </span>
      </header>
      <div className="msg-body">
        {isUser ? (
          message.content
        ) : (
          <Markdown content={message.content || "_（无文本内容）_"} />
        )}
      </div>
      {images.length > 0 ? (
        <div className="image-grid">
          {images.map((attachment, index) => (
            <MediaThumb
              key={`${attachment.path}-${index}`}
              path={attachment.path}
              alt={attachment.name}
            />
          ))}
        </div>
      ) : null}
      {!isUser && message.kind === "image" && images.length > 0 ? (
        <ImageActions images={images} />
      ) : null}
      {files.length > 0 ? (
        <div className="msg-attachments">
          {files.map((attachment) => (
            <span className="attachment-chip" key={attachment.path} title={attachment.path}>
              <FileText size={11} />
              <span className="name">{attachment.name}</span>
            </span>
          ))}
        </div>
      ) : null}
      {footer ? <div className="msg-inline-activity">{footer}</div> : null}
      {!isUser && canRetry && onRetry ? (
        <div className="msg-actions">
          <IconButton label="重试上一条请求" onClick={onRetry} size="sm">
            <RotateCcw size={12} />
          </IconButton>
        </div>
      ) : null}
    </article>
  );
}

export function StreamingItem({
  text,
  model,
}: {
  text: string;
  model?: string | null;
}) {
  return (
    <article className="msg assistant">
      <header className="msg-head">
        <span className="msg-role">
          <span className="role-mark">B</span>
          Basalt
          {model ? <span className="msg-model">{model}</span> : null}
        </span>
        <span className="msg-meta">
          <span className="streaming-hint">
            <span className="spinner" style={{ width: 9, height: 9 }} />
            STREAMING
          </span>
        </span>
      </header>
      <div className="msg-body">
        <Markdown content={text} />
        <span className="streaming-caret" />
      </div>
    </article>
  );
}
