import * as monaco from "monaco-editor";
import { loader } from "@monaco-editor/react";
import editorWorker from "monaco-editor/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/language/json/json.worker?worker";
import tsWorker from "monaco-editor/language/typescript/ts.worker?worker";

let initialized = false;

export function setupMonaco() {
  if (initialized) return;
  initialized = true;
  (self as unknown as { MonacoEnvironment: unknown }).MonacoEnvironment = {
    getWorker(_workerId: string, label: string) {
      if (label === "json") return new jsonWorker();
      if (label === "typescript" || label === "javascript") return new tsWorker();
      return new editorWorker();
    },
  };
  loader.config({ monaco });
  monaco.editor.defineTheme("basalt-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "comment", foreground: "6b6b73", fontStyle: "italic" },
      { token: "keyword", foreground: "c792ea" },
      { token: "string", foreground: "9ece9a" },
      { token: "number", foreground: "f0b429" },
      { token: "type", foreground: "82aaff" },
      { token: "function", foreground: "82aaff" },
    ],
    colors: {
      "editor.background": "#0f0f11",
      "editor.foreground": "#c3c3c9",
      "editorLineNumber.foreground": "#4d4d54",
      "editorLineNumber.activeForeground": "#a9a9b2",
      "editor.selectionBackground": "#31313a",
      "editor.inactiveSelectionBackground": "#26262c",
      "editor.lineHighlightBackground": "#141417",
      "editorCursor.foreground": "#ececef",
      "editorIndentGuide.background1": "#202024",
      "editorIndentGuide.activeBackground1": "#36363d",
      "editorWidget.background": "#141416",
      "editorWidget.border": "#35353b",
      "editorSuggestWidget.background": "#141416",
      "editorSuggestWidget.border": "#35353b",
      "editorHoverWidget.background": "#141416",
      "editorHoverWidget.border": "#35353b",
      "scrollbarSlider.background": "#ffffff14",
      "scrollbarSlider.hoverBackground": "#ffffff24",
      "minimap.background": "#0f0f11",
    },
  });
  monaco.editor.defineTheme("basalt-light", {
    base: "vs",
    inherit: true,
    rules: [
      { token: "comment", foreground: "8d8d85", fontStyle: "italic" },
      { token: "keyword", foreground: "8b3dbd" },
      { token: "string", foreground: "2f7d32" },
      { token: "number", foreground: "9a6b09" },
      { token: "type", foreground: "2f5fb0" },
      { token: "function", foreground: "2f5fb0" },
    ],
    colors: {
      "editor.background": "#fbfbfa",
      "editor.foreground": "#37373b",
      "editorLineNumber.foreground": "#a3a39c",
      "editorLineNumber.activeForeground": "#55554e",
      "editor.selectionBackground": "#d8d8d1",
      "editor.inactiveSelectionBackground": "#e7e7e1",
      "editor.lineHighlightBackground": "#f4f4f0",
      "editorCursor.foreground": "#17171a",
      "editorIndentGuide.background1": "#e4e4de",
      "editorIndentGuide.activeBackground1": "#c6c6bf",
      "editorWidget.background": "#fbfbfa",
      "editorWidget.border": "#c4c4be",
      "scrollbarSlider.background": "#00000014",
      "scrollbarSlider.hoverBackground": "#00000024",
      "minimap.background": "#fbfbfa",
    },
  });
}
