import { isValidElement, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CopyButton } from "./ui";
import { useAppStore } from "../lib/store";

function textOf(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (isValidElement(node)) {
    return textOf((node.props as { children?: ReactNode }).children);
  }
  return "";
}

const components: Components = {
  a({ href, children }) {
    return (
      <a
        href={href}
        title={href ? `打开链接：${href}` : undefined}
        onClick={(event) => {
          event.preventDefault();
          const target = href?.trim();
          if (!target) return;
          if (!/^https?:\/\//i.test(target)) {
            useAppStore
              .getState()
              .notify("warning", "无法打开链接", "仅支持 http/https 链接");
            return;
          }
          void openUrl(target).catch((error) => {
            useAppStore
              .getState()
              .notify("error", "打开链接失败", String(error));
          });
        }}
      >
        {children}
      </a>
    );
  },
  pre({ children }) {
    const child = Array.isArray(children) ? children[0] : children;
    const className = isValidElement(child)
      ? ((child.props as { className?: string }).className ?? "")
      : "";
    const language = /language-([\w-]+)/.exec(className)?.[1] ?? "text";
    const code = textOf(children);
    return (
      <div className="md-code">
        <div className="code-head">
          <span>{language}</span>
          <CopyButton text={code} label="复制代码" />
        </div>
        <pre>{children}</pre>
      </div>
    );
  },
};

export function Markdown({ content }: { content: string }) {
  return (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeHighlight, { detect: false, ignoreMissing: true }]]}
        components={components}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
