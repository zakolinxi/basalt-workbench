import type { CornerStyle, ThemeMode, ThemePalette } from "./types";

const CORNER_KEY = "basalt.corner-style.v1";
const COLOR_TRANSITION_MS = 460;

let transitionTimer: number | null = null;

function beginColorTransition() {
  const root = document.documentElement;
  root.classList.add("color-transition");
  if (transitionTimer !== null) window.clearTimeout(transitionTimer);
  transitionTimer = window.setTimeout(() => {
    root.classList.remove("color-transition");
    transitionTimer = null;
  }, COLOR_TRANSITION_MS);
}

export function resolveTheme(mode: ThemeMode): "dark" | "light" {
  if (mode === "system") {
    return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }
  return mode;
}

export function applyTheme(mode: ThemeMode) {
  const resolved = resolveTheme(mode);
  const root = document.documentElement;
  if (root.dataset.theme && root.dataset.theme !== resolved) {
    beginColorTransition();
  }
  root.dataset.theme = resolved;
  root.style.colorScheme = resolved;
}

export function applyPalette(palette: ThemePalette) {
  const root = document.documentElement;
  if (root.dataset.palette && root.dataset.palette !== palette) {
    beginColorTransition();
  }
  root.dataset.palette = palette;
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
