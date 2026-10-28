import { isValidElement, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
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
        title={href ? `复制链接：${href}` : undefined}
        onClick={(event) => {
          event.preventDefault();
          if (!href) return;
          void navigator.clipboard.writeText(href).then(() => {
            useAppStore.getState().notify("info", "链接已复制", href);
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
