import {
  useEffect,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from "react";
import { createPortal } from "react-dom";
import clsx from "clsx";
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Copy,
  Info,
  X,
  XCircle,
} from "lucide-react";
import { api } from "../lib/api";
import { useAppStore } from "../lib/store";
import type { TaskStatus, Toast } from "../lib/types";

export function Button({
  variant = "default",
  size,
  loading,
  className,
  children,
  disabled,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "default" | "primary" | "ghost" | "danger";
  size?: "sm" | "xs";
  loading?: boolean;
}) {
  return (
    <button
      className={clsx("btn", variant !== "default" && variant, size, className)}
      disabled={disabled || loading}
      {...rest}
    >
      {loading ? <span className="spinner" /> : null}
      {children}
    </button>
  );
}

export function IconButton({
  label,
  className,
  active,
  size,
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  active?: boolean;
  size?: "sm";
}) {
  return (
    <button
      className={clsx("icon-btn", size, active && "active", className)}
      title={label}
      aria-label={label}
      {...rest}
    >
      {children}
    </button>
  );
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={clsx("input", props.className)} />;
}

export function Textarea(props: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea {...props} className={clsx("textarea", props.className)} />;
}

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={clsx("select", props.className)} />;
}

export function Field({
  label,
  hint,
  children,
  className,
}: {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <label className={clsx("field", className)}>
      <span className="field-label">{label}</span>
      {children}
      {hint ? <span className="field-hint">{hint}</span> : null}
    </label>
  );
}

export function Badge({
  children,
  tone,
  className,
  title,
}: {
  children: ReactNode;
  tone?: "ok" | "warn" | "err" | "info";
  className?: string;
  title?: string;
}) {
  return (
    <span className={clsx("badge", tone, className)} title={title}>
      {children}
    </span>
  );
}

export function TaskStatusBadge({ status }: { status: TaskStatus | string }) {
  const tone =
    status === "completed"
      ? "ok"
      : status === "failed"
        ? "err"
        : status === "running"
          ? "warn"
          : status === "waitingApproval"
            ? "info"
            : status === "interrupted"
              ? "warn"
              : undefined;
  const label =
    status === "completed"
      ? "已完成"
      : status === "failed"
        ? "失败"
        : status === "running"
          ? "执行中"
          : status === "waitingApproval"
            ? "等待审批"
            : status === "queued"
              ? "排队中"
              : status === "cancelled"
                ? "已取消"
                : status === "interrupted"
                  ? "已中断"
                  : status;
  return <Badge tone={tone as "ok" | "warn" | "err" | "info" | undefined}>{label}</Badge>;
}

export function Switch({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      className={clsx("switch", checked && "on")}
      onClick={() => !disabled && onChange(!checked)}
      disabled={disabled}
      style={disabled ? { opacity: 0.45, cursor: "not-allowed" } : undefined}
    />
  );
}

export function SectionLabel({ children }: { children: ReactNode }) {
  return <span className="section-label">{children}</span>;
}

export function Kbd({ children }: { children: ReactNode }) {
  return <span className="kbd">{children}</span>;
}

export function useClickOutside<T extends HTMLElement>(
  onOutside: () => void,
  active = true,
) {
  const ref = useRef<T>(null);
  useEffect(() => {
    if (!active) return;
    const handler = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) {
        onOutside();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [active, onOutside]);
  return ref;
}

export function Modal({
  open,
  onClose,
  title,
  kicker,
  children,
  footer,
  size,
}: {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  kicker?: string;
  children: ReactNode;
  footer?: ReactNode;
  size?: "lg" | "xl";
}) {
  useEffect(() => {
    if (!open) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);
  if (!open) return null;
  return createPortal(
    <div
      className="modal-overlay"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className={clsx("modal", size)} role="dialog" aria-modal="true">
        <div className="modal-head">
          <div>
            {kicker ? <div className="kicker">{kicker}</div> : null}
            <div className="title">{title}</div>
          </div>
          <IconButton label="关闭" className="modal-close" onClick={onClose}>
            <X size={15} />
          </IconButton>
        </div>
        <div className="modal-body">{children}</div>
        {footer ? <div className="modal-foot">{footer}</div> : null}
      </div>
    </div>,
    document.body,
  );
}

export function ConfirmDialog({
  open,
  title,
  message,
  detail,
  confirmLabel = "确认",
  danger,
  loading,
  onCancel,
  onConfirm,
}: {
  open: boolean;
  title: string;
  message: ReactNode;
  detail?: ReactNode;
  confirmLabel?: string;
  danger?: boolean;
  loading?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <Modal
      open={open}
      onClose={onCancel}
      kicker="CONFIRM"
      title={title}
      footer={
        <>
          <Button onClick={onCancel} disabled={loading}>
            取消
          </Button>
          <Button
            variant={danger ? "danger" : "primary"}
            onClick={onConfirm}
            loading={loading}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div style={{ fontSize: 12.5, lineHeight: 1.65, color: "var(--fg-1)" }}>
        {message}
      </div>
      {detail ? (
        <div
          className="mono"
          style={{
            fontSize: 10.5,
            color: "var(--fg-3)",
            border: "1px solid var(--line)",
            padding: "8px 10px",
            maxHeight: 180,
            overflow: "auto",
            whiteSpace: "pre-wrap",
            overflowWrap: "anywhere",
          }}
        >
          {detail}
        </div>
      ) : null}
    </Modal>
  );
}

export function CopyButton({
  text,
  label = "复制",
  size = "xs",
  className,
}: {
  text: string;
  label?: string;
  size?: "sm" | "xs";
  className?: string;
}) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
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
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };
  return (
    <Button size={size} variant="ghost" onClick={copy} className={className}>
      {copied ? <Check size={11} /> : <Copy size={11} />}
      {copied ? "已复制" : label}
    </Button>
  );
}

export function MediaThumb({
  path,
  alt,
  className,
  thumb,
}: {
  path: string;
  alt: string;
  className?: string;
  thumb?: boolean;
}) {
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    let alive = true;
    api
      .readMedia(path)
      .then((url) => {
        if (alive) setSrc(url);
      })
      .catch(() => {
        if (alive) setFailed(true);
      });
    return () => {
      alive = false;
    };
  }, [path]);

  if (failed) {
    return (
      <div
        className="attachment-chip"
        title={path}
        style={{ color: "var(--err)" }}
      >
        <AlertTriangle size={11} /> 图片不可用
      </div>
    );
  }
  if (!src) {
    return (
      <div
        className={clsx("attachment-image", className)}
        style={{
          width: thumb ? 64 : 180,
          height: thumb ? 52 : 110,
          display: "grid",
          placeItems: "center",
        }}
      >
        <span className="spinner" />
      </div>
    );
  }
  return (
    <>
      <div className={clsx("attachment-image", className)} onClick={() => setOpen(true)}>
        <img src={src} alt={alt} style={thumb ? { maxWidth: 64, maxHeight: 52 } : undefined} />
      </div>
      {open ? (
        <Modal
          open={open}
          onClose={() => setOpen(false)}
          kicker="IMAGE PREVIEW"
          title={alt}
          size="lg"
        >
          <img
            src={src}
            alt={alt}
            style={{
              maxWidth: "100%",
              maxHeight: "65vh",
              objectFit: "contain",
              display: "block",
              margin: "0 auto",
              border: "1px solid var(--line)",
            }}
          />
        </Modal>
      ) : null}
    </>
  );
}

const toastIcon: Record<Toast["kind"], ReactNode> = {
  success: <CheckCircle2 size={15} />,
  error: <XCircle size={15} />,
  warning: <AlertTriangle size={15} />,
  info: <Info size={15} />,
};

export function Toasts() {
  const toasts = useAppStore((state) => state.toasts);
  const dismiss = useAppStore((state) => state.dismissToast);
  return createPortal(
    <div className="toasts">
      {toasts.map((toast) => (
        <div key={toast.id} className={clsx("toast", toast.kind)}>
          <span className="icon">{toastIcon[toast.kind]}</span>
          <div className="content">
            <div className="t">{toast.title}</div>
            {toast.message ? <div className="m">{toast.message}</div> : null}
          </div>
          <button className="dismiss" onClick={() => dismiss(toast.id)} aria-label="关闭">
            <X size={13} />
          </button>
        </div>
      ))}
    </div>,
    document.body,
  );
}
