import type { Language } from "./types";
import { dictionaries as baseDictionaries } from "./locales";
import { extraDictionaries } from "./locales-extra";
import { componentDictionaries } from "./locales-components";
import { libDictionaries } from "./locales-lib";
import { providerDictionaries } from "./locales-providers";
import { imageDictionaries } from "./locales-images";

type TranslatedLanguage = Exclude<Language, "zh-CN">;

const dictionaries: Record<TranslatedLanguage, Record<string, string>> = {
  "en-US": {
    ...baseDictionaries["en-US"],
    ...extraDictionaries["en-US"],
    ...componentDictionaries["en-US"],
    ...libDictionaries["en-US"],
    ...providerDictionaries["en-US"],
    ...imageDictionaries["en-US"],
  },
  "ja-JP": {
    ...baseDictionaries["ja-JP"],
    ...extraDictionaries["ja-JP"],
    ...componentDictionaries["ja-JP"],
    ...libDictionaries["ja-JP"],
    ...providerDictionaries["ja-JP"],
    ...imageDictionaries["ja-JP"],
  },
  "ko-KR": {
    ...baseDictionaries["ko-KR"],
    ...extraDictionaries["ko-KR"],
    ...componentDictionaries["ko-KR"],
    ...libDictionaries["ko-KR"],
    ...providerDictionaries["ko-KR"],
    ...imageDictionaries["ko-KR"],
  },
};

export const LANGUAGE_OPTIONS: { value: Language; label: string }[] = [
  { value: "zh-CN", label: "简体中文" },
  { value: "en-US", label: "English" },
  { value: "ja-JP", label: "日本語" },
  { value: "ko-KR", label: "한국어" },
];

const LANGUAGES = new Set<Language>(["zh-CN", "en-US", "ja-JP", "ko-KR"]);
const STORAGE_KEY = "basalt-language";
const CJK = /[\u3040-\u30ff\u3400-\u9fff\uac00-\ud7af]/;
const OBSERVE_OPTIONS: MutationObserverInit = {
  subtree: true,
  childList: true,
  characterData: true,
  attributes: true,
  attributeFilter: ["placeholder", "title", "aria-label", "alt"],
};

const TEXT_EXCLUDE = [
  ".msg-body",
  ".md",
  ".markdown",
  "pre",
  "code",
  "kbd",
  ".md-code",
  ".diff",
  ".terminal",
  ".monaco-editor",
  ".code-host",
  ".tool-output",
  ".tool-step-args",
  ".tool-step-target",
  ".task-test-command",
  ".path",
  ".truncate",
  ".name",
  ".side-row .label",
  ".file-tree",
  ".git-row .path",
  ".change-row .path",
  ".rail-change .path",
  ".mp-model-id",
  ".mp-model-name",
  ".mp-provider-name",
  ".provider-model-id",
  ".provider-name",
  ".model-name",
  ".image-prompt",
  ".gen-prompt",
  ".history-prompt",
  ".run-stats-model",
  ".run-stats-metric",
  ".rail-plan-copy",
  ".modal-plan",
  ".task-title",
  ".crumb-title",
  ".attachment-chip .name",
  "input",
  "textarea",
  "[contenteditable='true']",
].join(",");

const ATTR_EXCLUDE = [
  ".path",
  ".truncate",
  ".name",
  ".file-tree",
  ".mp-model",
  ".provider-item",
  ".rail-change",
  ".change-row",
  ".git-row",
  ".image-history",
  ".msg-body",
  ".monaco-editor",
  ".diff",
  ".terminal",
  ".gen-prompt",
  ".image-prompt",
].join(",");

const USER_TITLE_SELECTOR =
  ".side-row, .project-item, .project-card, .mp-model, .provider-item, .file-node, .rail-change";

let currentLanguage: Language = loadInitialLanguage();
let observer: MutationObserver | null = null;
const textSources = new WeakMap<Text, string>();
const attrSources = new WeakMap<Element, Map<string, string>>();
const renderedText = new WeakMap<Text, string>();
const renderedAttrs = new WeakMap<Element, Map<string, string>>();

export function isLanguage(value: unknown): value is Language {
  return typeof value === "string" && LANGUAGES.has(value as Language);
}

function loadInitialLanguage(): Language {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (isLanguage(stored)) return stored;
  } catch {
    /* ignore unavailable storage */
  }
  return "zh-CN";
}

function normalize(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

function interpolateNumbers(key: string, dict: Record<string, string>): string | null {
  const values: string[] = [];
  const template = key.replace(/\d+(?:[.,]\d+)*/g, (match) => {
    values.push(match);
    return `{${values.length - 1}}`;
  });
  const translated = dict[template];
  if (!translated) return null;
  return translated.replace(/\{(\d+)\}/g, (_, index) => values[Number(index)] ?? "");
}

export function translateFor(language: Language, raw: string): string {
  if (language === "zh-CN" || !CJK.test(raw)) return raw;
  const leading = raw.match(/^\s*/)?.[0] ?? "";
  const trailing = raw.match(/\s*$/)?.[0] ?? "";
  const key = normalize(raw);
  if (!key) return raw;
  const dict = dictionaries[language];
  const translated = Object.prototype.hasOwnProperty.call(dict, key)
    ? dict[key]
    : (interpolateNumbers(key, dict) ?? undefined);
  if (translated === undefined) return raw;
  return `${leading}${translated}${trailing}`;
}

export function translateText(raw: string): string {
  return translateFor(currentLanguage, raw);
}

export function getLanguage(): Language {
  return currentLanguage;
}

export function setLanguage(language: string) {
  currentLanguage = isLanguage(language) ? language : "zh-CN";
  try {
    localStorage.setItem(STORAGE_KEY, currentLanguage);
  } catch {
    /* ignore unavailable storage */
  }
  document.documentElement.lang = currentLanguage;
  retranslateAll();
}

function isTextExcluded(node: Text): boolean {
  const parent = node.parentElement;
  if (!parent) return true;
  if (parent.closest("[data-i18n-skip]")) return true;
  if (parent.closest("[data-i18n-opt-in]")) return false;
  if (parent.closest(TEXT_EXCLUDE)) return true;
  return ["SCRIPT", "STYLE", "NOSCRIPT", "TEXTAREA", "CANVAS"].includes(
    parent.tagName,
  );
}

function isAttrExcluded(element: Element, name: string): boolean {
  if (element.closest("[data-i18n-skip]")) return true;
  if (element.closest(ATTR_EXCLUDE)) return true;
  if (name === "title" && element.matches(USER_TITLE_SELECTOR)) return true;
  return false;
}

function applyText(node: Text) {
  if (isTextExcluded(node)) return;
  const value = node.data;
  if (!CJK.test(value)) return;
  const key = normalize(value);
  if (!key) return;
  textSources.set(node, value);
  const translated = translateText(value);
  if (translated !== value) node.data = translated;
  renderedText.set(node, translated);
}

function applyAttribute(element: Element, name: string) {
  if (isAttrExcluded(element, name)) return;
  const value = element.getAttribute(name);
  if (!value || !CJK.test(value)) return;
  const key = normalize(value);
  if (!key) return;
  let sources = attrSources.get(element);
  if (!sources) {
    sources = new Map();
    attrSources.set(element, sources);
  }
  sources.set(name, value);
  const translated = translateText(value);
  if (translated !== value) element.setAttribute(name, translated);
  let rendered = renderedAttrs.get(element);
  if (!rendered) {
    rendered = new Map();
    renderedAttrs.set(element, rendered);
  }
  rendered.set(name, translated);
}

function translateSubtree(root: Node) {
  if (root.nodeType === Node.TEXT_NODE) {
    applyText(root as Text);
    return;
  }
  if (!(root instanceof Element) && !(root instanceof DocumentFragment)) return;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();
  while (node) {
    applyText(node as Text);
    node = walker.nextNode();
  }
  if (root instanceof Element) {
    translateAttributes(root);
  }
  const elements =
    root instanceof Element
      ? root.querySelectorAll("[placeholder], [title], [aria-label], [alt]")
      : [];
  elements.forEach(translateAttributes);
}

function translateAttributes(element: Element) {
  for (const name of ["placeholder", "title", "aria-label", "alt"]) {
    if (element.hasAttribute(name)) applyAttribute(element, name);
  }
}

function retranslateText(node: Text) {
  if (isTextExcluded(node)) return;
  const current = node.data;
  const lastRendered = renderedText.get(node);
  let source = textSources.get(node);
  if (source === undefined || (lastRendered !== undefined && current !== lastRendered)) {
    source = current;
    textSources.set(node, source);
  }
  const translated = translateText(source);
  if (current !== translated) node.data = translated;
  renderedText.set(node, translated);
}

function retranslateAttribute(element: Element, name: string) {
  if (isAttrExcluded(element, name)) return;
  const current = element.getAttribute(name) ?? "";
  const lastRendered = renderedAttrs.get(element)?.get(name);
  const sources = attrSources.get(element);
  let source = sources?.get(name);
  if (source === undefined || (lastRendered !== undefined && current !== lastRendered)) {
    source = current;
    if (!attrSources.has(element)) attrSources.set(element, new Map());
    attrSources.get(element)!.set(name, source);
  }
  const translated = translateText(source);
  if (current !== translated) element.setAttribute(name, translated);
  if (!renderedAttrs.has(element)) renderedAttrs.set(element, new Map());
  renderedAttrs.get(element)!.set(name, translated);
}

function retranslateAll() {
  if (!document.body) return;
  observer?.disconnect();
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();
  while (node) {
    retranslateText(node as Text);
    node = walker.nextNode();
  }
  document
    .querySelectorAll("[placeholder], [title], [aria-label], [alt]")
    .forEach((element) => {
      for (const name of ["placeholder", "title", "aria-label", "alt"]) {
        if (element.hasAttribute(name)) retranslateAttribute(element, name);
      }
    });
  if (observer) observer.observe(document.body, OBSERVE_OPTIONS);
}

export function startDomTranslation() {
  if (typeof window === "undefined" || !document.body) return;
  document.documentElement.lang = currentLanguage;
  if (observer) return;
  retranslateAll();
  observer = new MutationObserver((records) => {
    observer?.disconnect();
    for (const record of records) {
      if (record.type === "characterData") {
        applyText(record.target as Text);
      } else if (record.type === "childList") {
        record.addedNodes.forEach((node) => translateSubtree(node));
      } else if (record.type === "attributes" && record.attributeName) {
        applyAttribute(record.target as Element, record.attributeName);
      }
    }
    if (observer) observer.observe(document.body, OBSERVE_OPTIONS);
  });
  observer.observe(document.body, OBSERVE_OPTIONS);
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    observer?.disconnect();
    observer = null;
  });
}
