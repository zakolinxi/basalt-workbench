import { describe, expect, it } from "vitest";
import { toolLabel, toolTarget } from "./format";

describe("tool formatting", () => {
  it("maps tool names to readable labels", () => {
    expect(toolLabel("search_files")).toBe("搜索文件");
    expect(toolLabel("spawn_subagent")).toBe("子代理");
    expect(toolLabel("custom_tool")).toBe("custom_tool");
  });

  it("shows both query and path for file searches", () => {
    expect(
      toolTarget("search_files", { query: "session", path: "backend" }),
    ).toBe("session · backend");
    expect(toolTarget("search_files", { query: "session" })).toBe("session");
  });

  it("falls back to paths and commands", () => {
    expect(toolTarget("read_file", { path: "src/main.ts" })).toBe(
      "src/main.ts",
    );
    expect(toolTarget("run_command", { command: "npm test" })).toBe("npm test");
    expect(toolTarget("git_status", {})).toBe("工作区");
  });
});
