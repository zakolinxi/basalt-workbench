package tools

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"strings"
	"time"
)

func toolBash() *Tool {
	return &Tool{
		Name: "bash",
		Description: "在工作区执行 shell 命令（Windows 下优先使用 Git Bash，其次 cmd）。" +
			"用于构建、测试、git 与查看环境。输出包含退出码；不要用它做需要交互的长驻进程。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"command":{"type":"string","description":"要执行的命令"},
				"description":{"type":"string","description":"一句话说明这条命令的用途"},
				"timeout":{"type":"integer","description":"超时毫秒，默认 120000，最大 600000"}
			},
			"required":["command"]
		}`),
		Run: runBash,
	}
}

func shellFor() (string, []string) {
	if runtime.GOOS != "windows" {
		return "/bin/sh", []string{"-lc"}
	}
	if bash := findBash(); bash != "" {
		return bash, []string{"-lc"}
	}
	return "cmd.exe", []string{"/C"}
}

func findBash() string {
	candidates := []string{
		`C:\Program Files\Git\bin\bash.exe`,
		`C:\Program Files\Git\usr\bin\bash.exe`,
		`C:\Program Files (x86)\Git\bin\bash.exe`,
	}
	for _, p := range candidates {
		if st, err := os.Stat(p); err == nil && !st.IsDir() {
			return p
		}
	}
	if p, err := exec.LookPath("bash.exe"); err == nil {
		return p
	}
	if p, err := exec.LookPath("bash"); err == nil {
		return p
	}
	return ""
}

func runBash(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	command := strings.TrimSpace(argString(args, "command", "cmd"))
	if command == "" {
		return Result{}, fmt.Errorf("缺少 command")
	}
	timeout := argInt(args, 120000, "timeout", "timeoutMs", "timeout_ms")
	if timeout < 1000 {
		timeout = 120000
	}
	if timeout > 600000 {
		timeout = 600000
	}
	runCtx, cancel := context.WithTimeout(ctx, time.Duration(timeout)*time.Millisecond)
	defer cancel()

	shell, flags := shellFor()
	argv := append(append([]string{}, flags...), command)
	cmd := exec.CommandContext(runCtx, shell, argv...)
	cmd.Dir = env.Root
	// strip common secrets from child env
	cmd.Env = sanitizedEnv()

	var buf bytes.Buffer
	cmd.Stdout = &buf
	cmd.Stderr = &buf
	cmd.WaitDelay = 2 * time.Second
	start := time.Now()
	err := cmd.Run()
	elapsed := time.Since(start)

	exitCode := 0
	if err != nil {
		if ee, ok := err.(*exec.ExitError); ok {
			exitCode = ee.ExitCode()
		} else if runCtx.Err() != nil {
			return Result{}, fmt.Errorf("命令超时（%s）：%s", time.Duration(timeout)*time.Millisecond, command)
		} else {
			return Result{}, fmt.Errorf("无法执行命令：%v", err)
		}
	}

	out := strings.ReplaceAll(buf.String(), "\r\n", "\n")
	out = strings.TrimRight(out, "\n")
	out, trimmed := truncate(out, 60000)
	label := strings.TrimSpace(argString(args, "description"))
	if label == "" {
		label = command
	}
	if len(label) > 80 {
		label = label[:80] + "…"
	}

	body := out
	if body == "" {
		body = "（无输出）"
	}
	status := "✓"
	if exitCode != 0 {
		status = "✗"
	}
	display := out
	if display == "" {
		display = "（无输出）"
	}
	return Result{
		Title:   fmt.Sprintf("%s bash %s", status, label),
		Output:  fmt.Sprintf("$ %s\n%s\n[exit %d · %s]", command, body, exitCode, elapsed.Round(time.Millisecond)),
		Display: display,
		Meta: map[string]any{
			"exit": exitCode, "ms": elapsed.Milliseconds(), "truncated": trimmed,
		},
	}, nil
}

// sanitizedEnv inherits PATH and platform essentials but drops API keys so a
// shell command cannot exfiltrate them by accident.
func sanitizedEnv() []string {
	secrets := map[string]bool{
		"OPENAI_API_KEY": true, "ANTHROPIC_API_KEY": true, "DEEPSEEK_API_KEY": true,
		"MOONSHOT_API_KEY": true, "ZHIPU_API_KEY": true, "GROQ_API_KEY": true,
		"OPENROUTER_API_KEY": true, "BASALT_API_KEY": true,
	}
	var out []string
	for _, kv := range os.Environ() {
		name, _, ok := strings.Cut(kv, "=")
		if ok && secrets[strings.ToUpper(name)] {
			continue
		}
		out = append(out, kv)
	}
	return out
}

func toolTodo() *Tool {
	return &Tool{
		Name:        "update_plan",
		ReadOnly:    true,
		Description: "提交或更新当前任务的执行计划，最多 12 步。复杂任务开始时调用一次，之后随进度更新 status。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"steps":{
					"type":"array",
					"items":{
						"type":"object",
						"properties":{
							"title":{"type":"string"},
							"detail":{"type":"string"},
							"status":{"type":"string","enum":["pending","in_progress","done","failed"]}
						},
						"required":["title"]
					}
				}
			},
			"required":["steps"]
		}`),
		Run: runTodo,
	}
}

// PlanStep mirrors the plan entries surfaced in the TUI rail.
type PlanStep struct {
	Title  string `json:"title"`
	Detail string `json:"detail"`
	Status string `json:"status"`
}

func runTodo(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	var payload struct {
		Steps []PlanStep `json:"steps"`
	}
	if err := json.Unmarshal(args, &payload); err != nil {
		return Result{}, fmt.Errorf("steps 解析失败：%v", err)
	}
	if len(payload.Steps) > 12 {
		payload.Steps = payload.Steps[:12]
	}
	var lines []string
	done := 0
	for _, s := range payload.Steps {
		icon := "○"
		switch s.Status {
		case "in_progress":
			icon = "◐"
		case "done":
			icon = "●"
			done++
		case "failed":
			icon = "✗"
		}
		lines = append(lines, fmt.Sprintf("%s %s", icon, s.Title))
	}
	raw, _ := json.Marshal(payload.Steps)
	return Result{
		Title:   fmt.Sprintf("plan · %d/%d", done, len(payload.Steps)),
		Output:  fmt.Sprintf("计划已更新：\n%s\n---\n%s", strings.Join(lines, "\n"), string(raw)),
		Display: strings.Join(lines, "\n"),
		Meta:    map[string]any{"progress": fmt.Sprintf("%d/%d", done, len(payload.Steps)), "steps": raw},
	}, nil
}
