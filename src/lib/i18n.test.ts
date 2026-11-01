import { describe, expect, it } from "vitest";
import { translateFor } from "./i18n";

describe("i18n", () => {
  it("keeps source text for Chinese", () => {
    expect(translateFor("zh-CN", "设置")).toBe("设置");
  });

  it("translates exact UI strings into all target languages", () => {
    expect(translateFor("en-US", "设置")).toBe("Settings");
    expect(translateFor("ja-JP", "设置")).toBe("設定");
    expect(translateFor("ko-KR", "设置")).toBe("설정");
  });

  it("interpolates numeric phrases", () => {
    expect(translateFor("en-US", "12 分钟前")).toBe("12 min ago");
    expect(translateFor("ja-JP", "12 分钟前")).toBe("12 分前");
    expect(translateFor("ko-KR", "12 分钟前")).toBe("12분 전");
    expect(translateFor("en-US", "执行工具 3 次")).toBe("Tools run: 3");
  });

  it("handles tool count fragments and empty translations", () => {
    expect(translateFor("en-US", "执行工具 3 次，其中 1 次失败")).toBe(
      "Tools run: 3, 1 failed",
    );
    expect(translateFor("en-US", "次")).toBe("");
    expect(translateFor("ja-JP", "次")).toBe("回");
  });

  it("falls back to the source for unknown strings", () => {
    expect(translateFor("en-US", "未收录的自定义文案")).toBe("未收录的自定义文案");
  });
});
