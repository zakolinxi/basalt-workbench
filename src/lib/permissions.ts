import type { ApprovalMode, Settings } from "./types";

export const APPROVAL_OPTIONS: {
  value: ApprovalMode;
  label: string;
  short: string;
  description: string;
}[] = [
  {
    value: "ask",
    label: "每次询问",
    short: "ASK",
    description: "文件写入与终端命令都会暂停，等待你逐项批准。",
  },
  {
    value: "auto",
    label: "自动审批",
    short: "AUTO",
    description: "文件写入自动执行，终端命令仍需你批准。",
  },
  {
    value: "full",
    label: "完全访问",
    short: "FULL",
    description: "文件写入与终端命令自动执行；危险命令仍由安全策略强制拒绝。",
  },
];

export function approvalMode(settings: Settings): ApprovalMode {
  if (settings.autoApproveCommands) return "full";
  if (settings.autoApproveWrites) return "auto";
  return "ask";
}

export function approvalPatch(
  mode: ApprovalMode,
): Pick<Settings, "autoApproveWrites" | "autoApproveCommands"> {
  return {
    autoApproveWrites: mode !== "ask",
    autoApproveCommands: mode === "full",
  };
}

export function approvalOption(mode: ApprovalMode) {
  return APPROVAL_OPTIONS.find((option) => option.value === mode) ?? APPROVAL_OPTIONS[0];
}
