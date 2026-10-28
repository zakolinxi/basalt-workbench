const ENDPOINT_SUFFIXES: Record<string, string[]> = {
  openai_chat: ["/chat/completions", "/completions", "/models"],
  openai_responses: ["/responses", "/models"],
  anthropic: ["/messages", "/models"],
  openai_images: [
    "/images/generations",
    "/images/edits",
    "/images/variations",
    "/models",
  ],
  ollama: [
    "/api/chat",
    "/api/generate",
    "/api/tags",
    "/api/ps",
    "/api/show",
    "/api/embed",
    "/v1/chat/completions",
    "/v1/models",
    "/api",
    "/v1",
  ],
};

function isVersionSegment(segment: string): boolean {
  return /^v\d[0-9a-z._-]*$/i.test(segment);
}

function isLocalHost(host: string): boolean {
  const value = host.toLowerCase();
  if (
    value.startsWith("localhost") ||
    value.startsWith("127.") ||
    value.startsWith("0.0.0.0") ||
    value.startsWith("[::1]") ||
    value.startsWith("10.") ||
    value.startsWith("192.168.") ||
    value.startsWith("host.docker.internal") ||
    value.endsWith(".local")
  ) {
    return true;
  }
  if (value.startsWith("172.")) {
    const second = Number(value.split(".")[1]);
    return Number.isInteger(second) && second >= 16 && second <= 31;
  }
  return false;
}

/** Fills in the scheme and API version path so only a host is required. */
export function normalizeBaseUrl(raw: string, protocol: string): string {
  const value = raw.trim();
  if (!value) return "";
  let input = value;
  if (!input.includes("://")) {
    const local = protocol === "ollama" || isLocalHost(input);
    input = `${local ? "http" : "https"}://${input}`;
  }
  let url: URL;
  try {
    url = new URL(input);
  } catch {
    return value;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") return value;
  url.search = "";
  url.hash = "";

  let path = url.pathname.replace(/\/+$/, "");
  const suffixes = ENDPOINT_SUFFIXES[protocol] ?? [];
  for (;;) {
    let stripped = false;
    for (const suffix of suffixes) {
      if (path.endsWith(suffix)) {
        path = path.slice(0, -suffix.length).replace(/\/+$/, "");
        stripped = true;
        break;
      }
    }
    if (!stripped) break;
  }
  if (protocol === "gemini" || protocol === "gemini_images") {
    const index = path.indexOf("/models");
    if (index >= 0) path = path.slice(0, index);
  }

  const version =
    protocol === "gemini" || protocol === "gemini_images" ? "v1beta" : "v1";
  const last = (path.split("/").pop() ?? "").toLowerCase();
  const incompleteVersion =
    last === "v" ||
    (version === "v1beta" && ["v1b", "v1be", "v1bet"].includes(last));
  if (incompleteVersion) {
    const index = path.lastIndexOf("/");
    path = index >= 0 ? `${path.slice(0, index)}/${version}` : `/${version}`;
  }

  const hasVersion = path.split("/").some(isVersionSegment);
  if (protocol !== "ollama" && !hasVersion) {
    path = path ? `${path}/${version}` : `/${version}`;
  }
  url.pathname = path || "/";
  return url.toString().replace(/\/+$/, "");
}
