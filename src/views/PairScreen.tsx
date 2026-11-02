import { useEffect, useRef, useState } from "react";
import { Loader2, MonitorSmartphone, Wifi } from "lucide-react";
import { normalizeBase, pairWith } from "../lib/bridge";
import { Button, Input } from "../components/ui";

const LAST_HOST_KEY = "basalt.remote.lastHost";

function defaultAddress(): string {
  // When the page is served by the desktop gateway itself, the origin already
  // points at the right machine and only the code is missing.
  const { protocol, hostname, host } = window.location;
  if (protocol.startsWith("http") && hostname !== "localhost" && hostname !== "127.0.0.1") {
    return `${protocol}//${host}`;
  }
  const remembered = localStorage.getItem(LAST_HOST_KEY);
  if (remembered) return remembered;
  return "";
}

function codeFromUrl(): string {
  const params = new URLSearchParams(window.location.search);
  return (params.get("code") ?? "").replace(/\D/g, "").slice(0, 6);
}

export function PairScreen({ onPaired }: { onPaired: () => void }) {
  const [address, setAddress] = useState(defaultAddress);
  const [code, setCode] = useState(codeFromUrl);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const autoTried = useRef(false);

  const connect = async (host: string, pairingCode: string) => {
    if (pairingCode.length !== 6) {
      setError("配对码是 6 位数字，请查看电脑端显示的号码");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const base = normalizeBase(host);
      await pairWith(base, pairingCode);
      localStorage.setItem(LAST_HOST_KEY, base);
      onPaired();
    } catch (issue) {
      setError(issue instanceof Error ? issue.message : String(issue));
    } finally {
      setBusy(false);
    }
  };

  // A QR code on the desktop links here with the code prefilled.
  useEffect(() => {
    if (autoTried.current) return;
    const prefill = codeFromUrl();
    if (prefill.length === 6 && address) {
      autoTried.current = true;
      void connect(address, prefill);
    }
  }, [address]);

  return (
    <div className="pair-screen">
      <div className="pair-card">
        <div className="pair-brand">
          <div className="boot-logo">
            <span />
            <span />
            <span />
          </div>
          <div>
            <div className="boot-title">BASALT</div>
            <div className="boot-sub">REMOTE CONSOLE</div>
          </div>
        </div>

        <div className="pair-intro">
          <MonitorSmartphone size={16} />
          <div>
            <strong>连接你的电脑</strong>
            <p>
              所有项目、密钥与 Agent 都留在电脑上运行，手机只是遥控器。
              先在电脑端「设置 → 手机连接」打开开关，然后输入上方显示的地址与 6 位配对码。
            </p>
          </div>
        </div>

        <label className="pair-field">
          <span>电脑地址</span>
          <Input
            value={address}
            placeholder="192.168.1.8:7717"
            inputMode="url"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            onChange={(event) => setAddress(event.target.value)}
          />
        </label>

        <label className="pair-field">
          <span>配对码</span>
          <Input
            value={code}
            placeholder="000000"
            inputMode="numeric"
            maxLength={6}
            className="pair-code"
            onChange={(event) => setCode(event.target.value.replace(/\D/g, "").slice(0, 6))}
            onKeyDown={(event) => {
              if (event.key === "Enter") void connect(address, code);
            }}
          />
        </label>

        {error ? <div className="pair-error">{error}</div> : null}

        <Button
          variant="primary"
          className="pair-submit"
          disabled={busy || !address.trim()}
          onClick={() => void connect(address, code)}
        >
          {busy ? <Loader2 size={15} className="spin" /> : <Wifi size={15} />}
          {busy ? "正在连接…" : "连接电脑"}
        </Button>

        <div className="pair-foot">
          手机与电脑需在同一个 WiFi。配对码 10 分钟内有效，连接成功后手机会记住这台电脑。
        </div>
      </div>
    </div>
  );
}
