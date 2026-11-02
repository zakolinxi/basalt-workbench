/**
 * Transport bridge.
 *
 * Basalt ships one frontend bundle that runs in two places:
 *
 * - inside the Tauri desktop shell, where `invoke`/`listen` talk to Rust
 *   directly over the IPC bridge;
 * - in a phone browser or the Capacitor app, where the same calls are sent to
 *   the desktop's remote gateway over HTTP + Server-Sent Events.
 *
 * Everything above this file (api.ts, the store, every view) stays unaware of
 * which one is in play, which is what keeps phone and desktop at feature parity.
 */
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import {
  listen as tauriListen,
  type UnlistenFn,
  type EventCallback,
} from "@tauri-apps/api/event";

export type { UnlistenFn } from "@tauri-apps/api/event";

export type RemoteConfig = {
  /** Origin of the desktop gateway, e.g. `http://192.168.1.8:7717`. */
  base: string;
  /** Bearer token handed out by the pairing exchange. */
  token: string;
  /** Human label shown in the connection bar. */
  host?: string;
  version?: string;
};

export type RemoteInfo = {
  name: string;
  version: string;
  port: number;
  host: string;
  requirePairing: boolean;
};

const CONFIG_KEY = "basalt.remote.v1";

export function isDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function isRemote(): boolean {
  return !isDesktop();
}

// ------------------------------------------------------------- config store

export function loadRemoteConfig(): RemoteConfig | null {
  if (!isRemote()) return null;
  try {
    const raw = localStorage.getItem(CONFIG_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as RemoteConfig;
    if (!parsed?.base || !parsed?.token) return null;
    return parsed;
  } catch {
    return null;
  }
}

export function saveRemoteConfig(config: RemoteConfig): void {
  localStorage.setItem(CONFIG_KEY, JSON.stringify(config));
}

export function clearRemoteConfig(): void {
  localStorage.removeItem(CONFIG_KEY);
  disposeStream();
}

function activeConfig(): RemoteConfig {
  const config = loadRemoteConfig();
  if (!config) throw new Error("尚未连接到电脑，请先完成配对");
  return config;
}

// ------------------------------------------------------------------- errors

export class RemoteAuthError extends Error {}

const OFFLINE_MESSAGE = "连接已断开，请确认电脑上的 Basalt 仍在运行且处于同一网络";

async function toNetworkError(error: unknown): Promise<never> {
  if (error instanceof RemoteAuthError) throw error;
  if (error instanceof DOMException && error.name === "AbortError") {
    throw new Error(OFFLINE_MESSAGE);
  }
  throw new Error(OFFLINE_MESSAGE);
}

// ------------------------------------------------------------------- invoke

export async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isDesktop()) {
    return tauriInvoke<T>(cmd, args);
  }

  const config = activeConfig();
  let response: Response;
  try {
    response = await fetch(`${config.base}/api/rpc`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${config.token}`,
      },
      body: JSON.stringify({ cmd, args: args ?? {} }),
    });
  } catch (error) {
    return toNetworkError(error);
  }

  if (response.status === 401) {
    await handleAuthLost();
    throw new RemoteAuthError("授权已失效，请重新配对");
  }

  let payload: { ok?: boolean; data?: T; error?: string };
  try {
    payload = await response.json();
  } catch {
    throw new Error(`电脑端返回了无法解析的响应（HTTP ${response.status}）`);
  }

  if (payload?.ok === true) return payload.data as T;
  throw new Error(payload?.error || `调用 ${cmd} 失败`);
}

// ----------------------------------------------------------- event channel

type Handler = (payload: unknown) => void;

const handlers = new Map<string, Set<Handler>>();
let streamController: AbortController | null = null;
let streamRunning = false;
let authLostHandled = false;

const AUTH_LOST_EVENT = "basalt:remote-auth-lost";

export function onAuthLost(handler: () => void): () => void {
  return subscribe(AUTH_LOST_EVENT, handler as Handler);
}

async function handleAuthLost(): Promise<void> {
  if (authLostHandled) return;
  authLostHandled = true;
  clearRemoteConfig();
  notify(AUTH_LOST_EVENT, null);
}

function notify(event: string, payload: unknown): void {
  const bucket = handlers.get(event);
  if (!bucket) return;
  for (const handler of bucket) {
    try {
      handler(payload);
    } catch {
      // A broken subscriber must not kill the stream for everyone else.
    }
  }
}

function subscribe(event: string, handler: Handler): () => void {
  let bucket = handlers.get(event);
  if (!bucket) {
    bucket = new Set();
    handlers.set(event, bucket);
  }
  bucket.add(handler);
  return () => {
    bucket?.delete(handler);
  };
}

function dispatchFrame(raw: string): void {
  let parsed: { event?: string; payload?: unknown };
  try {
    parsed = JSON.parse(raw);
  } catch {
    return;
  }
  if (!parsed?.event) return;
  notify(parsed.event, parsed.payload);
}

async function consumeStream(config: RemoteConfig): Promise<void> {
  const controller = new AbortController();
  streamController = controller;

  const response = await fetch(`${config.base}/api/events`, {
    headers: { authorization: `Bearer ${config.token}` },
    signal: controller.signal,
  });

  if (response.status === 401) {
    await handleAuthLost();
    return;
  }
  if (!response.ok || !response.body) {
    throw new Error(OFFLINE_MESSAGE);
  }

  const reader = response.body
    .pipeThrough(new TextDecoderStream())
    .getReader();
  let buffer = "";

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += value;
    let boundary = buffer.indexOf("\n\n");
    while (boundary >= 0) {
      const chunk = buffer.slice(0, boundary);
      buffer = buffer.slice(boundary + 2);
      for (const line of chunk.split("\n")) {
        if (line.startsWith("data: ")) {
          dispatchFrame(line.slice(6));
        }
      }
      boundary = buffer.indexOf("\n\n");
    }
  }

  throw new Error(OFFLINE_MESSAGE);
}

function disposeStream(): void {
  streamController?.abort();
  streamController = null;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function runStream(): Promise<void> {
  if (streamRunning) return;
  streamRunning = true;
  let attempt = 0;
  while (streamRunning) {
    const config = loadRemoteConfig();
    if (!config) break;
    try {
      await consumeStream(config);
      attempt = 0;
    } catch {
      attempt += 1;
    }
    if (!streamRunning) break;
    if (!loadRemoteConfig()) break;
    // Back off up to 8s so a sleeping phone does not hammer the desktop.
    await sleep(Math.min(8000, 600 * attempt));
  }
  streamRunning = false;
}

export async function listen<T>(
  event: string,
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  if (isDesktop()) {
    return tauriListen<T>(event, handler);
  }
  const unsubscribe = subscribe(event, handler as Handler);
  void runStream();
  return () => {
    unsubscribe();
  };
}

// ---------------------------------------------------------------- pairing

export function normalizeBase(raw: string): string {
  let value = raw.trim();
  if (!value) throw new Error("请填写电脑地址");
  if (!/^https?:\/\//i.test(value)) value = `http://${value}`;
  return value.replace(/\/+$/, "");
}

export async function probeHost(raw: string): Promise<RemoteInfo> {
  const base = normalizeBase(raw);
  let response: Response;
  try {
    response = await fetch(`${base}/api/info`, { signal: AbortSignal.timeout(6000) });
  } catch {
    throw new Error("无法连接该地址，请确认电脑端已开启「手机连接」且在同一 WiFi");
  }
  if (!response.ok) throw new Error(`电脑未响应（HTTP ${response.status}）`);
  return (await response.json()) as RemoteInfo;
}

export async function pairWith(
  raw: string,
  code: string,
): Promise<RemoteConfig> {
  const base = normalizeBase(raw);
  const info = await probeHost(base);
  let response: Response;
  try {
    response = await fetch(`${base}/api/pair`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ code: code.trim() }),
      signal: AbortSignal.timeout(10000),
    });
  } catch {
    throw new Error(OFFLINE_MESSAGE);
  }

  const payload = (await response.json().catch(() => null)) as
    | { ok?: boolean; token?: string; error?: string }
    | null;
  if (!response.ok || !payload?.ok || !payload.token) {
    throw new Error(payload?.error || "配对失败，请检查配对码");
  }

  const config: RemoteConfig = {
    base,
    token: payload.token,
    host: info.host,
    version: info.version,
  };
  saveRemoteConfig(config);
  authLostHandled = false;
  return config;
}

export function disconnect(): void {
  clearRemoteConfig();
  disposeStream();
}

/** Restarts the event stream, e.g. after the app comes back to the foreground. */
export function resumeStream(): void {
  if (isDesktop()) return;
  disposeStream();
  streamRunning = false;
  if (loadRemoteConfig()) void runStream();
}
