import { describe, expect, it } from "vitest";
import {
  APPROVAL_OPTIONS,
  approvalMode,
  approvalOption,
  approvalPatch,
} from "./permissions";
import type { ApprovalMode, Settings } from "./types";

const settings = (writes: boolean, commands: boolean) =>
  ({ autoApproveWrites: writes, autoApproveCommands: commands }) as Settings;

describe("approval policy", () => {
  it("maps the two backend flags to a single mode", () => {
    expect(approvalMode(settings(false, false))).toBe("ask");
    expect(approvalMode(settings(true, false))).toBe("auto");
    expect(approvalMode(settings(true, true))).toBe("full");
    // Command auto-approval is the strictest capability, so it wins.
    expect(approvalMode(settings(false, true))).toBe("full");
  });

  it("maps every mode back to the backend flags", () => {
    expect(approvalPatch("ask")).toEqual({
      autoApproveWrites: false,
      autoApproveCommands: false,
    });
    expect(approvalPatch("auto")).toEqual({
      autoApproveWrites: true,
      autoApproveCommands: false,
    });
    expect(approvalPatch("full")).toEqual({
      autoApproveWrites: true,
      autoApproveCommands: true,
    });
  });

  it("round-trips modes through the backend flags", () => {
    for (const option of APPROVAL_OPTIONS) {
      const patch = approvalPatch(option.value);
      expect(approvalMode({ ...patch } as Settings)).toBe(option.value);
      expect(approvalOption(option.value).label).toBe(option.label);
    }
  });

  it("falls back to the safest option for unknown modes", () => {
    expect(approvalOption("unknown" as ApprovalMode).value).toBe("ask");
  });
});
