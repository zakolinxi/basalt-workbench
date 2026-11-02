import { useCallback, useEffect, useRef, useState } from "react";
import QRCode from "qrcode";
import { Copy, Loader2, QrCode, RefreshCw, Smartphone, Wifi } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { isDesktop } from "../lib/bridge";
import type { RemoteStatus } from "../lib/types";
import { Badge, Button, Input, Switch } from "./ui";

/**
 * Desktop-only panel that turns the LAN gateway on and shows the address,
 * pairing code and QR code a phone needs.
 */
export function RemoteAccessPanel() {
  const [status, setStatus] = useState<RemoteStatus | null>(null);
  const [port, setPort] = useState("7717");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [qr, setQr] = useState<string | null>(null);
  const [showQr, setShowQr] = useState(false);
  const [copied, setCopied] = useState(false);
  const visible = isDesktop();
  const alive = useRef(true);

  const apply = useCallback((next: RemoteStatus) => {
    setStatus(next);
    setPort(String(next.port));
    setError(next.error);
  }, []);

  const refresh = useCallback(async () => {
    if (!visible) return;
    try {
      const next = await api.remoteStatus();
      if (alive.current) apply(next);
    } catch (issue) {
      if (alive.current) setError(errorMessage(issue));
    }
  }, [apply, visible]);

  useEffect(() => {
    alive.current = true;
    void refresh();
    // Poll so the code expiry and connected client count stay honest.
    const timer = window.setInterval(() => void refresh(), 5000);
    return () => {
      alive.current = false;
      window.clearInterval(timer);
    };
  }, [refresh]);

  const pairUrl =
    status && status.code
      ? `http://${status.address}/?code=${status.code}`
      : null;

  useEffect(() => {
    if (!showQr || !pairUrl) {
      setQr(null);
      return;
    }
    let cancelled = false;
    void QRCode.toDataURL(pairUrl, { margin: 1, width: 240 })
      .then((url) => {
        if (!cancelled) setQr(url);
      })
      .catch(() => {
        if (!cancelled) setQr(null);
      });
    return () => {
      cancelled = true;
    };
  }, [showQr, pairUrl]);

  const toggle = async (enabled: boolean) => {
    setBusy(true);
    setError(null);
    try {
      const nextPort = Number.parseInt(port, 10);
      const result = await api.remoteSetEnabled(
        enabled,
        Number.isFinite(nextPort) ? nextPort : undefined,
      );
      apply(result);
    } catch (issue) {
      setError(errorMessage(issue));
    } finally {
      setBusy(false);
    }
  };

  const rotate = async () => {
    try {
      apply(await api.remoteRotateCode());
      setCopied(false);
    } catch (issue) {
      setError(errorMessage(issue));
    }
  };

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch {
      setError("无法写入剪贴板，请手动抄写");
    }
  };

  if (!visible) {
    return (
      <section className="settings-section">
        <div className="form-section-head">手机连接</div>
        <div className="body">
          <div className="setting-row">
            <div className="copy">
              <div className="title">当前是手机客户端</div>
              <div className="desc">
                远程网关运行在电脑上。请在电脑端的设置里开启开关并完成配对。
              </div>
            </div>
            <div className="control">
              <Badge tone="info">REMOTE</Badge>
            </div>
          </div>
        </div>
      </section>
    );
  }

  const running = status?.running ?? false;
  const code = status?.code ?? "";

  return (
    <section className="settings-section">
      <div className="form-section-head">手机连接</div>
      <div className="body">
        <div className="setting-row">
          <div className="copy">
            <div className="title inline-flex">
              <Smartphone size={12} /> 允许手机连接本机
            </div>
            <div className="desc">
              在同一 WiFi 下，手机可以查看会话、审批 Agent 操作、看变更 Diff 并使用终端。
              所有项目、密钥与 Agent 仍在电脑上运行，网关只监听局域网。
            </div>
          </div>
          <div className="control">
            <Switch
              checked={status?.enabled ?? false}
              disabled={busy || !status}
              onChange={(value) => void toggle(value)}
            />
          </div>
        </div>

        {error ? <div className="remote-error">{error}</div> : null}

        <div className="setting-row">
          <div className="copy">
            <div className="title">监听端口</div>
            <div className="desc">默认 7717。修改后需要关闭再打开上面的开关。</div>
          </div>
          <div className="control" style={{ display: "flex", gap: 8 }}>
            <Input
              className="mono"
              style={{ width: 110 }}
              value={port}
              inputMode="numeric"
              onChange={(event) => setPort(event.target.value.replace(/\D/g, "").slice(0, 5))}
            />
            <Badge tone={running ? "ok" : "info"}>{running ? "LISTENING" : "OFF"}</Badge>
          </div>
        </div>

        {running ? (
          <>
            <div className="setting-row">
              <div className="copy">
                <div className="title inline-flex">
                  <Wifi size={12} /> 地址
                </div>
                <div className="desc">在手机浏览器直接打开这个地址，或在 APK 里手动输入。</div>
              </div>
              <div className="control remote-address">
                <code className="mono">{status?.address}</code>
                <Button
                  size="sm"
                  onClick={() => void copy(`http://${status?.address}/`)}
                >
                  <Copy size={12} /> {copied ? "已复制" : "复制"}
                </Button>
              </div>
            </div>

            <div className="setting-row">
              <div className="copy">
                <div className="title inline-flex">
                  <QrCode size={12} /> 配对码
                </div>
                <div className="desc">
                  {status?.codeExpiresIn ? (
                    <>
                      剩余 {Math.ceil(status.codeExpiresIn / 60)} 分钟失效 · 已连接{" "}
                      {status.clients} 台设备
                    </>
                  ) : (
                    "开启后生成"
                  )}
                </div>
              </div>
              <div className="control remote-pairing">
                <div className="remote-code mono">{code || "······"}</div>
                <div className="remote-pairing-actions">
                  <Button size="sm" onClick={() => void rotate()}>
                    <RefreshCw size={12} /> 换一个
                  </Button>
                  <Button size="sm" onClick={() => setShowQr((value) => !value)}>
                    <QrCode size={12} /> {showQr ? "隐藏二维码" : "显示二维码"}
                  </Button>
                </div>
              </div>
            </div>

            {showQr ? (
              <div className="remote-qr">
                {qr ? (
                  <img src={qr} alt="配对二维码" width={200} height={200} />
                ) : (
                  <div className="remote-qr-loading">
                    <Loader2 size={16} className="spin" /> 生成中…
                  </div>
                )}
                <div className="remote-qr-tip">
                  用手机相机扫码即可打开配对页；扫码后会自动带上配对码。
                </div>
              </div>
            ) : null}
          </>
        ) : (
          <div className="remote-offline">
            打开开关后，这里会显示配对地址、配对码与二维码。
          </div>
        )}
      </div>
    </section>
  );
}
