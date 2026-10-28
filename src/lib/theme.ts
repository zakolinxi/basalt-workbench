import type { CornerStyle, ThemeMode } from "./types";

const CORNER_KEY = "basalt.corner-style.v1";

export function resolveTheme(mode: ThemeMode): "dark" | "light" {
  if (mode === "system") {
    return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }
  return mode;
}

export function applyTheme(mode: ThemeMode) {
  const resolved = resolveTheme(mode);
  document.documentElement.dataset.theme = resolved;
  document.documentElement.style.colorScheme = resolved;
}

export function loadCornerStyle(): CornerStyle {
  try {
    return localStorage.getItem(CORNER_KEY) === "sharp" ? "sharp" : "round";
  } catch {
    return "round";
  }
}

export function applyCornerStyle(style: CornerStyle) {
  document.documentElement.dataset.corners = style;
  try {
    localStorage.setItem(CORNER_KEY, style);
  } catch {
    /* preference is optional */
  }
}

export function watchSystemTheme(callback: () => void): () => void {
  const media = window.matchMedia?.("(prefers-color-scheme: light)");
  if (!media) return () => {};
  const listener = () => callback();
  media.addEventListener("change", listener);
  return () => media.removeEventListener("change", listener);
}
