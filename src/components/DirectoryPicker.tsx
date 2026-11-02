import { useCallback, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { ChevronLeft, Folder, HardDrive, Home, Loader2 } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import type { DirListing } from "../lib/types";
import { Button, Modal } from "./ui";

/**
 * Browses directories on the machine running Basalt and resolves with the
 * chosen absolute path. Used by the phone client, where a native folder dialog
 * would only see the phone's own storage.
 */
export function requestDirectory(title = "选择文件夹"): Promise<string | null> {
  return new Promise((resolve) => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    const close = (value: string | null) => {
      root.unmount();
      host.remove();
      resolve(value);
    };
    root.render(<DirectoryPicker title={title} onClose={close} />);
  });
}

function DirectoryPicker({
  title,
  onClose,
}: {
  title: string;
  onClose: (value: string | null) => void;
}) {
  const [listing, setListing] = useState<DirListing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async (path: string | null) => {
    setLoading(true);
    setError(null);
    try {
      const next = await api.browseDirectories(path);
      setListing(next);
    } catch (issue) {
      setError(errorMessage(issue));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(null);
  }, [load]);

  const parent = listing?.parent ?? null;

  return (
    <Modal
      open
      onClose={() => onClose(null)}
      kicker="电脑端目录"
      title={title}
      footer={
        <>
          <Button onClick={() => onClose(null)}>取消</Button>
          <Button
            variant="primary"
            disabled={!listing}
            onClick={() => onClose(listing?.path ?? null)}
          >
            选择此目录
          </Button>
        </>
      }
    >
      <div className="dir-picker">
        <div className="dir-picker-bar">
          <Button
            onClick={() => void load(parent)}
            disabled={!parent || loading}
            title="上一级"
          >
            <ChevronLeft size={14} />
          </Button>
          <Button onClick={() => void load(listing?.home ?? null)} disabled={loading}>
            <Home size={14} />
          </Button>
          <code className="dir-picker-path" title={listing?.path}>
            {listing?.path ?? "…"}
          </code>
        </div>

        {error ? <div className="dir-picker-error">{error}</div> : null}

        <div className="dir-picker-list">
          {loading ? (
            <div className="dir-picker-empty">
              <Loader2 size={15} className="spin" /> 正在读取…
            </div>
          ) : listing && listing.entries.length === 0 ? (
            <div className="dir-picker-empty">该目录下没有子文件夹</div>
          ) : (
            listing?.entries.map((entry) => (
              <button
                key={entry.path}
                type="button"
                className="dir-picker-item"
                onClick={() => void load(entry.path)}
              >
                <Folder size={15} />
                <span>{entry.name}</span>
              </button>
            ))
          )}
        </div>

        <div className="dir-picker-hint">
          <HardDrive size={13} /> 浏览的是电脑上的目录，不是手机存储
        </div>
      </div>
    </Modal>
  );
}
