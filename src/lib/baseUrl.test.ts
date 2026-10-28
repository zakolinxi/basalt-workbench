import { describe, expect, it } from "vitest";
import { normalizeBaseUrl } from "./baseUrl";

const openai = (value: string) => normalizeBaseUrl(value, "openai_chat");

describe("normalizeBaseUrl", () => {
  it("adds scheme and v1", () => {
    expect(openai("api.deepseek.com")).toBe("https://api.deepseek.com/v1");
    expect(openai("https://api.deepseek.com")).toBe(
      "https://api.deepseek.com/v1",
    );
    expect(openai("https://api.deepseek.com/v1/")).toBe(
      "https://api.deepseek.com/v1",
    );
  });

  it("keeps version paths", () => {
    expect(openai("https://open.bigmodel.cn/api/paas/v4")).toBe(
      "https://open.bigmodel.cn/api/paas/v4",
    );
    expect(openai("https://dashscope.aliyuncs.com/compatible-mode/v1")).toBe(
      "https://dashscope.aliyuncs.com/compatible-mode/v1",
    );
  });

  it("strips pasted endpoint paths", () => {
    expect(openai("https://api.openai.com/v1/chat/completions")).toBe(
      "https://api.openai.com/v1",
    );
    expect(
      normalizeBaseUrl("https://api.anthropic.com/v1/messages", "anthropic"),
    ).toBe("https://api.anthropic.com/v1");
  });

  it("uses v1beta for gemini", () => {
    expect(
      normalizeBaseUrl("https://generativelanguage.googleapis.com", "gemini"),
    ).toBe("https://generativelanguage.googleapis.com/v1beta");
    expect(
      normalizeBaseUrl(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent",
        "gemini",
      ),
    ).toBe("https://generativelanguage.googleapis.com/v1beta");
  });

  it("keeps the ollama root on http", () => {
    expect(normalizeBaseUrl("127.0.0.1:11434", "ollama")).toBe(
      "http://127.0.0.1:11434",
    );
    expect(normalizeBaseUrl("http://127.0.0.1:11434/api", "ollama")).toBe(
      "http://127.0.0.1:11434",
    );
    expect(normalizeBaseUrl("http://127.0.0.1:11434/api/v1", "ollama")).toBe(
      "http://127.0.0.1:11434",
    );
  });

  it("uses http for local gateways and fixes partial versions", () => {
    expect(openai("192.168.1.8:8080")).toBe("http://192.168.1.8:8080/v1");
    expect(openai("https://api.openai.com/v")).toBe(
      "https://api.openai.com/v1",
    );
  });

  it("drops query strings", () => {
    expect(openai("https://api.openai.com/?key=1")).toBe(
      "https://api.openai.com/v1",
    );
    expect(openai("  ")).toBe("");
  });
});
