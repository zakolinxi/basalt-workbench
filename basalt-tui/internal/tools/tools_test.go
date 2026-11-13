package tools

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func newTestEnv(t *testing.T) (*Registry, *Env, string) {
	t.Helper()
	dir := t.TempDir()
	env := NewEnv(dir, "test", t.TempDir())
	return NewRegistry(), env, dir
}

func run(t *testing.T, reg *Registry, env *Env, name string, args map[string]any) Result {
	t.Helper()
	raw, err := json.Marshal(args)
	if err != nil {
		t.Fatal(err)
	}
	res, err := reg.Execute(context.Background(), name, raw, env)
	if err != nil {
		t.Fatalf("%s failed: %v", name, err)
	}
	return res
}

func TestWriteReadEdit(t *testing.T) {
	reg, env, dir := newTestEnv(t)

	res := run(t, reg, env, "write", map[string]any{
		"path":    "src/auth.ts",
		"content": "export function login() {\n  return false;\n}\n",
	})
	if res.Diff == "" {
		t.Fatal("write should return a diff for new content")
	}

	read := run(t, reg, env, "read", map[string]any{"path": "src/auth.ts"})
	if !strings.Contains(read.Output, "1│export function login()") {
		t.Fatalf("unexpected read output: %s", read.Output)
	}

	edit := run(t, reg, env, "edit", map[string]any{
		"path":      "src/auth.ts",
		"oldString": "return false;",
		"newString": "return true;",
	})
	if !strings.Contains(edit.Output, "+1 -1") {
		t.Fatalf("unexpected edit summary: %s", edit.Output)
	}

	raw, err := os.ReadFile(filepath.Join(dir, "src", "auth.ts"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(raw), "return true;") {
		t.Fatalf("file not updated: %s", raw)
	}

	if env.SnapshotCount() != 2 {
		t.Fatalf("expected 2 undo snapshots, got %d", env.SnapshotCount())
	}
	snap, ok := env.PopSnapshot()
	if !ok || !strings.Contains(snap.Content, "return false;") {
		t.Fatalf("undo snapshot should hold the pre-edit content: %+v", snap)
	}
}

func TestEditRequiresUniqueMatch(t *testing.T) {
	reg, env, _ := newTestEnv(t)
	run(t, reg, env, "write", map[string]any{
		"path": "a.txt", "content": "foo\nfoo\n",
	})
	raw, _ := json.Marshal(map[string]any{"path": "a.txt", "oldString": "foo", "newString": "bar"})
	if _, err := reg.Execute(context.Background(), "edit", raw, env); err == nil {
		t.Fatal("expected ambiguity error")
	}
	res := run(t, reg, env, "edit", map[string]any{
		"path": "a.txt", "oldString": "foo", "newString": "bar", "replaceAll": true,
	})
	if !strings.Contains(res.Output, "替换 2 处") {
		t.Fatalf("unexpected replaceAll output: %s", res.Output)
	}
}

func TestPathEscapeRejected(t *testing.T) {
	reg, env, _ := newTestEnv(t)
	raw, _ := json.Marshal(map[string]any{"path": "../outside.txt", "content": "x"})
	if _, err := reg.Execute(context.Background(), "write", raw, env); err == nil {
		t.Fatal("expected path escape to be rejected")
	}
}

func TestGlobAndGrep(t *testing.T) {
	reg, env, _ := newTestEnv(t)
	run(t, reg, env, "write", map[string]any{"path": "src/a.ts", "content": "export const alpha = 1;\n"})
	run(t, reg, env, "write", map[string]any{"path": "src/b.go", "content": "package main\n"})
	run(t, reg, env, "write", map[string]any{"path": "README.md", "content": "# demo\n"})

	glob := run(t, reg, env, "glob", map[string]any{"pattern": "**/*.ts"})
	if !strings.Contains(glob.Output, "src/a.ts") || strings.Contains(glob.Output, "b.go") {
		t.Fatalf("unexpected glob output: %s", glob.Output)
	}

	grep := run(t, reg, env, "grep", map[string]any{"pattern": "alpha", "include": "*.ts"})
	if !strings.Contains(grep.Output, "src/a.ts:1:") {
		t.Fatalf("unexpected grep output: %s", grep.Output)
	}
}

func TestBashRunsInWorkspace(t *testing.T) {
	reg, env, dir := newTestEnv(t)
	res := run(t, reg, env, "bash", map[string]any{"command": "echo basalt-ok", "timeout": 10000})
	if !strings.Contains(res.Output, "basalt-ok") || !strings.Contains(res.Output, "[exit 0") {
		t.Fatalf("unexpected bash output: %s", res.Output)
	}
	if _, err := os.Stat(dir); err != nil {
		t.Fatal(err)
	}
}

func TestSchemasAreValidJSON(t *testing.T) {
	reg := NewRegistry()
	for _, tool := range reg.Schemas() {
		var v map[string]any
		if err := json.Unmarshal(tool.Parameters, &v); err != nil {
			t.Fatalf("%s schema is not valid JSON: %v", tool.Name, err)
		}
		if v["type"] != "object" {
			t.Fatalf("%s schema must be an object", tool.Name)
		}
	}
}
