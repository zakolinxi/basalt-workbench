package tools

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/juang/basalt-tui/internal/diff"
)

func toolRead() *Tool {
	return &Tool{
		Name:     "read",
		ReadOnly: true,
		Description: "读取工作区内的文本文件。返回带行号的内容；大文件用 offset/limit 分段读取。" +
			"修改文件前必须先读取，确认当前内容。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"path":{"type":"string","description":"相对工作区的文件路径"},
				"offset":{"type":"integer","description":"起始行（从 1 开始）"},
				"limit":{"type":"integer","description":"最多读取行数，默认 800，最大 2000"}
			},
			"required":["path"]
		}`),
		Run: runRead,
	}
}

func runRead(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	abs, err := env.Resolve(argString(args, "path", "file", "filePath"))
	if err != nil {
		return Result{}, err
	}
	info, err := os.Stat(abs)
	if err != nil {
		return Result{}, fmt.Errorf("无法读取 %s：%v", env.Rel(abs), err)
	}
	if info.IsDir() {
		return Result{}, fmt.Errorf("%s 是目录，请用 list 工具", env.Rel(abs))
	}
	raw, err := os.ReadFile(abs)
	if err != nil {
		return Result{}, err
	}
	if bytes.IndexByte(raw[:min(len(raw), 8192)], 0) >= 0 {
		return Result{}, fmt.Errorf("%s 是二进制文件", env.Rel(abs))
	}
	text := strings.ReplaceAll(string(raw), "\r\n", "\n")
	lines := strings.Split(text, "\n")
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}

	offset := argInt(args, 1, "offset", "start")
	if offset < 1 {
		offset = 1
	}
	limit := argInt(args, 800, "limit")
	if limit <= 0 {
		limit = 800
	}
	if limit > 2000 {
		limit = 2000
	}
	if offset > len(lines) {
		return Result{Title: fmt.Sprintf("read %s", env.Rel(abs)), Output: fmt.Sprintf("（文件只有 %d 行，offset %d 超出范围）", len(lines), offset)}, nil
	}
	end := offset - 1 + limit
	if end > len(lines) {
		end = len(lines)
	}

	var b strings.Builder
	for i := offset - 1; i < end; i++ {
		fmt.Fprintf(&b, "%5d│%s\n", i+1, lines[i])
	}
	out := strings.TrimRight(b.String(), "\n")
	if end < len(lines) {
		out += fmt.Sprintf("\n… 还有 %d 行，继续读取请用 offset=%d", len(lines)-end, end+1)
	}
	out, _ = truncate(out, 90000)

	title := fmt.Sprintf("read %s", env.Rel(abs))
	if offset > 1 || end < len(lines) {
		title += fmt.Sprintf("  L%d-%d", offset, end)
	}
	return Result{
		Title:  title,
		Output: out,
		Meta:   map[string]any{"lines": len(lines), "shown": end - offset + 1},
	}, nil
}

func toolWrite() *Tool {
	return &Tool{
		Name:        "write",
		Description: "写入（新建或整体覆盖）工作区文件。覆盖已有文件前应先 read 确认内容。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"path":{"type":"string"},
				"content":{"type":"string","description":"完整文件内容"}
			},
			"required":["path","content"]
		}`),
		Run: runWrite,
	}
}

func runWrite(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	abs, err := env.Resolve(argString(args, "path", "filePath", "file"))
	if err != nil {
		return Result{}, err
	}
	content := argString(args, "content", "text")
	if content == "" && !strings.Contains(string(args), "\"content\"") {
		return Result{}, fmt.Errorf("缺少 content 参数")
	}
	before, readErr := os.ReadFile(abs)
	existed := readErr == nil
	env.Remember(abs)
	if err := os.MkdirAll(filepath.Dir(abs), 0o755); err != nil {
		return Result{}, err
	}
	if err := os.WriteFile(abs, []byte(content), 0o644); err != nil {
		return Result{}, err
	}
	rel := env.Rel(abs)
	ops := diff.Lines(string(before), content)
	added, removed := diff.Stats(ops)
	verb := "新建"
	if existed {
		verb = "写入"
	}
	return Result{
		Title:  fmt.Sprintf("%s %s  +%d -%d", verb, rel, added, removed),
		Output: fmt.Sprintf("%s %s 完成（%d 行，+%d -%d）", verb, rel, strings.Count(content, "\n")+1, added, removed),
		Diff:   diff.Unified(rel, ops, 3),
		Meta:   map[string]any{"added": added, "removed": removed, "existed": existed},
	}, nil
}

func toolEdit() *Tool {
	return &Tool{
		Name: "edit",
		Description: "对文件做精确字符串替换。oldString 必须在文件中唯一出现（跨行也可以），" +
			"否则请提供更长上下文或设置 replaceAll。保持缩进与原始格式一致。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"path":{"type":"string"},
				"oldString":{"type":"string","description":"要被替换的原文，必须唯一"},
				"newString":{"type":"string","description":"替换后的内容；为空表示删除"},
				"replaceAll":{"type":"boolean","description":"替换所有匹配"}
			},
			"required":["path","oldString","newString"]
		}`),
		Run: runEdit,
	}
}

func runEdit(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	abs, err := env.Resolve(argString(args, "path", "filePath", "file"))
	if err != nil {
		return Result{}, err
	}
	oldStr := argString(args, "oldString", "old_string", "old")
	newStr := argString(args, "newString", "new_string", "new")
	replaceAll := argBool(args, "replaceAll", "replace_all")
	if oldStr == "" {
		return Result{}, fmt.Errorf("oldString 不能为空；新建文件请用 write")
	}
	raw, err := os.ReadFile(abs)
	if err != nil {
		return Result{}, fmt.Errorf("无法读取 %s：%v", env.Rel(abs), err)
	}
	before := strings.ReplaceAll(string(raw), "\r\n", "\n")
	normalizedOld := strings.ReplaceAll(oldStr, "\r\n", "\n")
	normalizedNew := strings.ReplaceAll(newStr, "\r\n", "\n")

	count := strings.Count(before, normalizedOld)
	if count == 0 {
		return Result{}, fmt.Errorf("%s 中找不到 oldString；请先 read 确认原文（注意缩进与空白）", env.Rel(abs))
	}
	if count > 1 && !replaceAll {
		return Result{}, fmt.Errorf("oldString 在 %s 中出现 %d 次，请提供更多上下文或设置 replaceAll=true", env.Rel(abs), count)
	}
	after := before
	if replaceAll {
		after = strings.ReplaceAll(before, normalizedOld, normalizedNew)
	} else {
		after = strings.Replace(before, normalizedOld, normalizedNew, 1)
	}
	env.Remember(abs)
	if err := os.WriteFile(abs, []byte(after), 0o644); err != nil {
		return Result{}, err
	}
	rel := env.Rel(abs)
	ops := diff.Lines(before, after)
	added, removed := diff.Stats(ops)
	return Result{
		Title:  fmt.Sprintf("edit %s  +%d -%d", rel, added, removed),
		Output: fmt.Sprintf("edit %s 完成（替换 %d 处，+%d -%d）", rel, count, added, removed),
		Diff:   diff.Unified(rel, ops, 3),
		Meta:   map[string]any{"added": added, "removed": removed, "matches": count},
	}, nil
}
