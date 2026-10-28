import { diffLines } from "diff";

export interface FlatLine {
  kind: "context" | "add" | "del";
  text: string;
  oldLine: number | null;
  newLine: number | null;
}

export interface UnifiedRow {
  kind: "context" | "add" | "del" | "hunk";
  text: string;
  oldLine?: number | null;
  newLine?: number | null;
}

export interface SplitLine {
  kind: "context" | "add" | "del" | "empty";
  text: string;
  line: number | null;
}

export interface SplitRow {
  left: SplitLine;
  right: SplitLine;
}

function splitValue(value: string): string[] {
  const lines = value.split("\n");
  if (lines.length > 0 && lines[lines.length - 1] === "") lines.pop();
  return lines.map((line) => line.replace(/\r$/, ""));
}

export function computeFlatLines(oldText: string, newText: string): FlatLine[] {
  const changes = diffLines(oldText, newText);
  const out: FlatLine[] = [];
  let oldLine = 1;
  let newLine = 1;
  for (const change of changes) {
    for (const text of splitValue(change.value)) {
      if (change.added) {
        out.push({ kind: "add", text, oldLine: null, newLine: newLine++ });
      } else if (change.removed) {
        out.push({ kind: "del", text, oldLine: oldLine++, newLine: null });
      } else {
        out.push({ kind: "context", text, oldLine: oldLine++, newLine: newLine++ });
      }
    }
  }
  return out;
}

export function computeUnified(
  oldText: string,
  newText: string,
  context = 3,
): { rows: UnifiedRow[]; additions: number; deletions: number } {
  const flat = computeFlatLines(oldText, newText);
  const additions = flat.filter((line) => line.kind === "add").length;
  const deletions = flat.filter((line) => line.kind === "del").length;
  if (additions === 0 && deletions === 0) {
    return {
      rows: flat.map((line) => ({
        kind: line.kind,
        text: line.text,
        oldLine: line.oldLine,
        newLine: line.newLine,
      })),
      additions,
      deletions,
    };
  }
  const included = new Set<number>();
  flat.forEach((line, index) => {
    if (line.kind !== "context") {
      for (
        let i = Math.max(0, index - context);
        i <= Math.min(flat.length - 1, index + context);
        i += 1
      ) {
        included.add(i);
      }
    }
  });
  const rows: UnifiedRow[] = [];
  let index = 0;
  while (index < flat.length) {
    if (!included.has(index)) {
      let end = index;
      while (end < flat.length && !included.has(end)) end += 1;
      const skipped = flat.slice(index, end);
      const oldStart = skipped[0]?.oldLine ?? skipped[0]?.newLine ?? 0;
      const newStart = skipped[0]?.newLine ?? skipped[0]?.oldLine ?? 0;
      rows.push({
        kind: "hunk",
        text: `@@ ${oldStart} → ${newStart} · ${skipped.length} 行未显示 @@`,
      });
      index = end;
      continue;
    }
    if (index === 0 || (index > 0 && !included.has(index - 1) && rows.length > 1)) {
      const first = flat[index];
      let end = index;
      while (end + 1 < flat.length && included.has(end + 1)) end += 1;
      const count = end - index + 1;
      const oldStart = first.oldLine ?? 0;
      const newStart = first.newLine ?? 0;
      rows.push({
        kind: "hunk",
        text: `@@ -${oldStart},${count} +${newStart},${count} @@`,
      });
    }
    const line = flat[index];
    rows.push({
      kind: line.kind,
      text: line.text,
      oldLine: line.oldLine,
      newLine: line.newLine,
    });
    index += 1;
  }
  return { rows, additions, deletions };
}

export function computeSplit(
  oldText: string,
  newText: string,
): { rows: SplitRow[]; additions: number; deletions: number } {
  const flat = computeFlatLines(oldText, newText);
  const additions = flat.filter((line) => line.kind === "add").length;
  const deletions = flat.filter((line) => line.kind === "del").length;
  const rows: SplitRow[] = [];
  let index = 0;
  const empty = (): SplitLine => ({ kind: "empty", text: "", line: null });
  while (index < flat.length) {
    const line = flat[index];
    if (line.kind === "context") {
      rows.push({
        left: { kind: "context", text: line.text, line: line.oldLine },
        right: { kind: "context", text: line.text, line: line.newLine },
      });
      index += 1;
      continue;
    }
    const removed: FlatLine[] = [];
    const added: FlatLine[] = [];
    while (index < flat.length && flat[index].kind === "del") {
      removed.push(flat[index]);
      index += 1;
    }
    while (index < flat.length && flat[index].kind === "add") {
      added.push(flat[index]);
      index += 1;
    }
    const max = Math.max(removed.length, added.length);
    for (let i = 0; i < max; i += 1) {
      const del = removed[i];
      const add = added[i];
      rows.push({
        left: del
          ? { kind: "del", text: del.text, line: del.oldLine }
          : empty(),
        right: add
          ? { kind: "add", text: add.text, line: add.newLine }
          : empty(),
      });
    }
  }
  return { rows, additions, deletions };
}
