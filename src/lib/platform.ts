/**
 * Platform shims.
 *
 * The desktop build talks to native dialogs and the opener plugin. The phone
 * build has no access to the PC's file system, so those calls degrade to the
 * browser equivalents: a hidden `<input type="file">`, a download link, and
 * `window.open`.
 */
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { isDesktop } from "./bridge";

export type PickOptions = {
  directory?: boolean;
  multiple?: boolean;
  title?: string;
  filters?: { name: string; extensions: string[] }[];
};

/**
 * Native file/folder picker. Returns an empty array off-desktop; callers that
 * need a PC-side path on the phone should use the directory browser instead.
 */
export async function pickPaths(options: PickOptions): Promise<string[]> {
  if (!isDesktop()) return [];
  const selection = await open(options);
  if (!selection) return [];
  return Array.isArray(selection) ? selection : [selection];
}

export async function chooseSavePath(
  defaultPath: string,
  extensions: string[],
): Promise<string | null> {
  if (!isDesktop()) return null;
  return save({ defaultPath, filters: [{ name: "File", extensions }] });
}

/** Opens the phone's own file picker and returns the chosen files. */
export function pickBrowserFiles(
  options: { multiple?: boolean; accept?: string } = {},
): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    if (options.multiple) input.multiple = true;
    if (options.accept) input.accept = options.accept;
    input.style.position = "fixed";
    input.style.left = "-9999px";
    document.body.appendChild(input);

    let settled = false;
    const finish = (files: File[]) => {
      if (settled) return;
      settled = true;
      input.remove();
      resolve(files);
    };

    input.addEventListener("change", () => finish(Array.from(input.files ?? [])));
    input.addEventListener("cancel", () => finish([]));
    input.click();
    // Some WebViews never fire `cancel`; settle once focus comes back.
    window.addEventListener("focus", () => setTimeout(() => finish([]), 800), {
      once: true,
    });
  });
}

/** Saves a data URL to the phone's downloads folder. */
export function downloadDataUrl(name: string, dataUrl: string): void {
  const link = document.createElement("a");
  link.href = dataUrl;
  link.download = name;
  document.body.appendChild(link);
  link.click();
  link.remove();
}

/** Reads a picked file as raw base64 (no data-URL prefix). */
export function readFileBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result ?? "");
      resolve(result.split(",")[1] ?? "");
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

export async function openExternal(url: string): Promise<void> {
  if (isDesktop()) {
    await openUrl(url);
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}
