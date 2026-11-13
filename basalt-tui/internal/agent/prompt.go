package agent

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strings"

	"github.com/juang/basalt-tui/internal/tools"
)

const maxInstructionBytes = 12 << 10

// SystemPrompt builds the system prompt for this workspace. It mirrors the
// product rules of the Basalt desktop agent so behaviour stays consistent.
func SystemPrompt(root string, reg *tools.Registry) string {
	git := "当前目录不是 Git 仓库。"
	if isGitRepo(root) {
		git = "当前目录是 Git 仓库，可用 `git status` / `git diff` 查看改动。"
	}

	tree := shallowTree(root, 48)
	instructions := projectInstructions(root)
	shell := "PowerShell/CMD"
	if runtime.GOOS == "windows" {
		if _, err := exec.LookPath("bash"); err == nil {
			shell = "Git Bash（优先）/ CMD"
		}
	}

	var names []string
	for _, n := range reg.Names() {
		if t, ok := reg.Get(n); ok {
			mark := "只读"
			if !t.ReadOnly {
				mark = "需审批"
			}
			names = append(names, fmt.Sprintf("%s(%s)", n, mark))
		}
	}

	return fmt.Sprintf(`你是 Basalt，一个运行在终端里的编程代理（basalt-tui），风格类似 opencode。

工作区根目录（唯一允许操作的目录）：%s
操作系统：%s。命令通过 %s 执行。
%s

工作要求：
1. 先理解需求和现有代码，再动手。复杂任务先调用 update_plan 提交分步骤计划；简单任务直接执行。
2. 只使用工具完成实际操作。不要声称执行了未调用的工具，也不要把计划当成结果。
3. 修改文件前必须先 read 目标文件。精确修改用 edit（oldString 必须唯一），整体重写才用 write。
4. 所有路径都使用项目内相对路径（如 src/main.rs），不要访问工作区之外的文件。
5. write/edit/bash/webfetch 会请求用户批准；被拒绝时不要绕过，而是解释原因或换方案。
6. 不要请求、打印或写入 API Key、密码、令牌等敏感信息。
7. 测试失败时先读真实报错再修复。只有工具返回成功并通过验证，才能说任务完成。
8. 结束时给出简洁总结：改了哪些文件、跑了哪些验证命令、还有哪些未解决。
9. 使用中文交流；代码、命令、路径和标识符保持原样。
10. 不要重复相同的失败调用；已有信息足够时直接开始修改。

可用工具：%s

当前工作区文件（浅层）：
%s
%s`, root, runtime.GOOS, shell, git, strings.Join(names, "、"), tree, instructions)
}

func projectInstructions(root string) string {
	var out strings.Builder
	for _, name := range []string{"AGENTS.md", "BASALT.md", "CLAUDE.md"} {
		path := filepath.Join(root, name)
		raw, err := os.ReadFile(path)
		if err != nil {
			continue
		}
		text := strings.TrimSpace(string(raw))
		if text == "" {
			continue
		}
		if len(text) > maxInstructionBytes {
			text = text[:maxInstructionBytes] + "\n…[已截断]"
		}
		fmt.Fprintf(&out, "\n项目说明（来自 %s，必须遵守）：\n%s\n", name, text)
	}
	return out.String()
}

func shallowTree(root string, limit int) string {
	entries, err := os.ReadDir(root)
	if err != nil {
		return "（无法读取目录）"
	}
	sort.Slice(entries, func(i, j int) bool {
		if entries[i].IsDir() != entries[j].IsDir() {
			return entries[i].IsDir()
		}
		return entries[i].Name() < entries[j].Name()
	})
	skip := map[string]bool{
		".git": true, "node_modules": true, "target": true, "dist": true,
		"build": true, ".next": true, ".venv": true, "__pycache__": true,
	}
	var lines []string
	for _, e := range entries {
		if len(lines) >= limit {
			lines = append(lines, "…")
			break
		}
		name := e.Name()
		if skip[name] {
			continue
		}
		if e.IsDir() {
			lines = append(lines, name+"/")
		} else {
			lines = append(lines, name)
		}
	}
	if len(lines) == 0 {
		return "（空目录）"
	}
	return strings.Join(lines, "  ")
}

func isGitRepo(root string) bool {
	for dir := root; ; {
		if _, err := os.Stat(filepath.Join(dir, ".git")); err == nil {
			return true
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return false
		}
		dir = parent
	}
}
