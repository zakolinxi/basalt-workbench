import { describe, expect, it } from "vitest";
import { computeFlatLines, computeSplit, computeUnified } from "./diff";

describe("diff utilities", () => {
  const oldText = "alpha\nbeta\ngamma\n";
  const newText = "alpha\nBETA\ngamma\ndelta\n";

  it("classifies added, removed and context lines", () => {
    const flat = computeFlatLines(oldText, newText);
    expect(flat.filter((line) => line.kind === "del").map((line) => line.text)).toEqual([
      "beta",
    ]);
    expect(flat.filter((line) => line.kind === "add").map((line) => line.text)).toEqual([
      "BETA",
      "delta",
    ]);
    expect(flat.some((line) => line.kind === "context" && line.text === "alpha")).toBe(
      true,
    );
  });

  it("computes unified stats", () => {
    const unified = computeUnified(oldText, newText);
    expect(unified.additions).toBe(2);
    expect(unified.deletions).toBe(1);
    expect(unified.rows.some((row) => row.kind === "hunk")).toBe(true);
  });

  it("aligns split rows", () => {
    const split = computeSplit(oldText, newText);
    const changed = split.rows.find(
      (row) => row.left.kind === "del" && row.right.kind === "add",
    );
    expect(changed?.left.text).toBe("beta");
    expect(changed?.right.text).toBe("BETA");
  });

  it("handles identical content", () => {
    const unified = computeUnified("same\n", "same\n");
    expect(unified.additions).toBe(0);
    expect(unified.deletions).toBe(0);
    expect(unified.rows.every((row) => row.kind === "context")).toBe(true);
  });
});
