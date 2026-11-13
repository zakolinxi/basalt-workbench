package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/juang/basalt-tui/internal/diff"
	"github.com/juang/basalt-tui/internal/llm"
	"github.com/juang/basalt-tui/internal/session"
	"github.com/juang/basalt-tui/internal/tools"
)

// EventKind identifies agent stream events consumed by the TUI.
type EventKind int

const (
	EvRoundStart EventKind = iota
	EvText
	EvThinking
	EvToolStart
	EvToolEnd
	EvUsage
	EvNotice
	EvDone
	EvError
)

// ToolEvent describes one tool invocation.
type ToolEvent struct {
	ID       string
	Name     string
	Title    string
	Args     string
	Preview  string
	Diff     string
	Output   string
	ReadOnly bool
	Added    int
	Removed  int
	Elapsed  time.Duration
	Err      string
}

// Event is emitted while the agent runs.
type Event struct {
	Kind       EventKind
	Text       string
	Tool       *ToolEvent
	Input      int
	Output     int
	Cached     int
	StopReason string
	Err        error
}

// Decision is the user's answer to an approval request.
type Decision int

const (
	DecisionDeny Decision = iota
	DecisionOnce
	DecisionAlways
)

// Approval describes a pending mutating tool call.
type Approval struct {
	ID       string
	Tool     string
	Title    string
	Args     string
	Preview  string
	ReadOnly bool
}

// Approver decides whether a mutating tool may run. It blocks until answered.
type Approver interface {
	Approve(ctx context.Context, a Approval) Decision
}

// Agent owns the conversation loop.
type Agent struct {
	Client    llm.Client
	Registry  *tools.Registry
	Env       *tools.Env
	Approver  Approver
	Emit      func(Event)
	Session   *session.Session
	System    string
	MaxTokens int
	Temp      float64
	Reasoning string
	MaxRounds int

	// AutoApproveRead runs read-only tools without asking. When false every
	// tool call goes through the approver.
	AutoApproveRead bool
	// CompactAfter is the message-count threshold for context compaction.
	CompactAfter int

	mu           sync.Mutex
	autoApproved map[string]bool
}

// New wires an agent to a session.
func New(client llm.Client, reg *tools.Registry, env *tools.Env, sess *session.Session) *Agent {
	return &Agent{
		Client:          client,
		Registry:        reg,
		Env:             env,
		Session:         sess,
		MaxRounds:       24,
		MaxTokens:       8192,
		AutoApproveRead: true,
		CompactAfter:    30,
		autoApproved:    map[string]bool{},
	}
}

func (a *Agent) emit(ev Event) {
	if a.Emit != nil {
		a.Emit(ev)
	}
}

// AutoApprove marks a tool as approved for the rest of the session.
func (a *Agent) AutoApprove(tool string) {
	a.mu.Lock()
	defer a.mu.Unlock()
	a.autoApproved[tool] = true
}

// Run drives the model/tool loop until the model stops requesting tools.
func (a *Agent) Run(ctx context.Context, userText string) error {
	a.mu.Lock()
	if a.Session.Title == "新会话" || a.Session.Title == "" {
		a.Session.Title = session.TitleFrom(userText)
	}
	a.Session.Messages = append(a.Session.Messages, llm.Message{Role: "user", Content: userText})
	a.mu.Unlock()

	if dropped := a.compactIfNeeded(); dropped > 0 {
		a.emit(Event{Kind: EvNotice, Text: fmt.Sprintf("上下文较长，已压缩 %d 条旧消息", dropped)})
	}

	for round := 0; round < a.MaxRounds; round++ {
		if err := ctx.Err(); err != nil {
			return err
		}
		a.emit(Event{Kind: EvRoundStart, Text: fmt.Sprintf("round %d", round+1)})

		var text strings.Builder
		var calls []llm.ToolCall
		stop := ""
		req := llm.Request{
			Model:       a.Session.Model,
			System:      a.System,
			Messages:    a.history(),
			Tools:       a.Registry.Schemas(),
			MaxTokens:   a.MaxTokens,
			Temperature: a.Temp,
			Reasoning:   a.Reasoning,
		}
		err := a.Client.Stream(ctx, req, func(ev llm.Event) error {
			switch ev.Kind {
			case llm.KindText:
				text.WriteString(ev.Text)
				a.emit(Event{Kind: EvText, Text: ev.Text})
			case llm.KindThinking:
				a.emit(Event{Kind: EvThinking, Text: ev.Text})
			case llm.KindToolCall:
				if ev.ToolCall != nil {
					calls = append(calls, *ev.ToolCall)
				}
			case llm.KindUsage:
				a.Session.Usage.Add(ev.InputTokens, ev.OutputTokens, ev.CachedTokens)
				a.Session.Usage.Requests++
				a.emit(Event{Kind: EvUsage, Input: ev.InputTokens, Output: ev.OutputTokens, Cached: ev.CachedTokens})
			case llm.KindDone:
				stop = ev.StopReason
			}
			return nil
		})
		if err != nil {
			if ctx.Err() != nil {
				return ctx.Err()
			}
			a.emit(Event{Kind: EvError, Err: err})
			return err
		}

		assistant := llm.Message{Role: "assistant", Content: text.String(), ToolCalls: calls}
		a.mu.Lock()
		a.Session.Messages = append(a.Session.Messages, assistant)
		if len(calls) > 0 {
			a.Session.Usage.ToolCalls += len(calls)
		}
		a.mu.Unlock()

		if len(calls) == 0 {
			a.emit(Event{Kind: EvDone, StopReason: stop})
			return nil
		}

		for _, call := range calls {
			if err := ctx.Err(); err != nil {
				return err
			}
			result, toolErr := a.execTool(ctx, call)
			content := result.Output
			if toolErr != nil {
				content = "工具错误：" + toolErr.Error()
			}
			if content == "" {
				content = "（无输出）"
			}
			a.mu.Lock()
			a.Session.Messages = append(a.Session.Messages, llm.Message{
				Role: "tool", ToolCallID: call.ID, Name: call.Name, Content: content,
			})
			a.mu.Unlock()
		}
	}
	a.emit(Event{Kind: EvNotice, Text: fmt.Sprintf("已达到最大轮数 %d，停止继续调用工具", a.MaxRounds)})
	a.emit(Event{Kind: EvDone})
	return nil
}

func (a *Agent) history() []llm.Message {
	a.mu.Lock()
	defer a.mu.Unlock()
	out := make([]llm.Message, len(a.Session.Messages))
	copy(out, a.Session.Messages)
	return out
}

func (a *Agent) execTool(ctx context.Context, call llm.ToolCall) (tools.Result, error) {
	t, ok := a.Registry.Get(call.Name)
	if !ok {
		ev := &ToolEvent{ID: call.ID, Name: call.Name, Title: call.Name, Args: call.Arguments, Err: "未知工具"}
		a.emit(Event{Kind: EvToolStart, Tool: ev})
		a.emit(Event{Kind: EvToolEnd, Tool: ev})
		return tools.Result{}, fmt.Errorf("未知工具 %q", call.Name)
	}

	args := json.RawMessage(call.Arguments)
	if len(args) == 0 || !json.Valid(args) {
		args = json.RawMessage("{}")
	}
	preview, added, removed := a.preview(t, args)
	approval := Approval{
		ID: call.ID, Tool: call.Name, Title: previewTitle(t, args),
		Args: prettyArgs(call.Arguments), Preview: preview, ReadOnly: t.ReadOnly,
	}

	needApproval := !t.ReadOnly || !a.AutoApproveRead
	if needApproval && !a.approved(call.Name) {
		if a.Approver != nil {
			switch a.Approver.Approve(ctx, approval) {
			case DecisionAlways:
				a.AutoApprove(call.Name)
			case DecisionOnce:
				// continue
			default:
				ev := &ToolEvent{ID: call.ID, Name: call.Name, Title: approval.Title, Args: approval.Args, Preview: preview, Added: added, Removed: removed, Err: "用户已拒绝"}
				a.emit(Event{Kind: EvToolStart, Tool: ev})
				a.emit(Event{Kind: EvToolEnd, Tool: ev})
				return tools.Result{Output: "用户拒绝执行该操作。请说明原因或提出替代方案，不要重复调用被拒绝的工具。"}, nil
			}
		}
	}

	start := &ToolEvent{
		ID: call.ID, Name: call.Name, Title: approval.Title, Args: approval.Args,
		Preview: preview, ReadOnly: t.ReadOnly, Added: added, Removed: removed,
	}
	a.emit(Event{Kind: EvToolStart, Tool: start})

	runCtx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	began := time.Now()
	res, err := a.Registry.Execute(runCtx, call.Name, args, a.Env)
	done := &ToolEvent{
		ID: call.ID, Name: call.Name, Title: start.Title, Args: start.Args,
		Preview: preview, ReadOnly: t.ReadOnly, Elapsed: time.Since(began),
	}
	if err != nil {
		done.Err = err.Error()
		done.Title = call.Name + " · 失败"
		a.emit(Event{Kind: EvToolEnd, Tool: done})
		return res, err
	}
	done.Title = res.Title
	done.Diff = res.Diff
	done.Output = res.Display
	if done.Output == "" {
		done.Output = res.Output
	}
	if v, ok := res.Meta["added"].(int); ok {
		done.Added = v
	}
	if v, ok := res.Meta["removed"].(int); ok {
		done.Removed = v
	}
	a.emit(Event{Kind: EvToolEnd, Tool: done})
	return res, nil
}

func (a *Agent) approved(name string) bool {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.autoApproved[name]
}

// preview computes a diff for mutating file tools so the approval card can
// show exactly what will change before anything touches disk.
func (a *Agent) preview(t *tools.Tool, args json.RawMessage) (string, int, int) {
	var m map[string]any
	_ = json.Unmarshal(args, &m)
	str := func(k string) string {
		if v, ok := m[k].(string); ok {
			return v
		}
		return ""
	}
	path := str("path")
	if path == "" {
		path = str("filePath")
	}
	if path == "" {
		path = str("file")
	}
	switch t.Name {
	case "write", "edit":
		abs, err := a.Env.Resolve(path)
		if err != nil {
			return "", 0, 0
		}
		before, _ := os.ReadFile(abs)
		beforeText := strings.ReplaceAll(string(before), "\r\n", "\n")
		var afterText string
		if t.Name == "write" {
			afterText = str("content")
		} else {
			old := str("oldString")
			if old == "" {
				old = str("old_string")
			}
			nw := str("newString")
			afterText = beforeText
			if old != "" {
				afterText = strings.Replace(beforeText, old, nw, 1)
			}
		}
		ops := diff.Lines(beforeText, afterText)
		added, removed := diff.Stats(ops)
		unified := diff.Unified(a.Env.Rel(abs), ops, 3)
		if len(unified) > 12000 {
			unified = unified[:12000] + "\n…（diff 过长，已截断预览）"
		}
		return unified, added, removed
	case "bash":
		return "$ " + str("command"), 0, 0
	case "webfetch":
		return str("url"), 0, 0
	}
	return "", 0, 0
}

func previewTitle(t *tools.Tool, args json.RawMessage) string {
	var m map[string]any
	_ = json.Unmarshal(args, &m)
	str := func(k string) string {
		if v, ok := m[k].(string); ok {
			return v
		}
		return ""
	}
	switch t.Name {
	case "bash":
		if v := str("description"); v != "" {
			return v
		}
		return str("command")
	case "write", "edit":
		if p := str("path"); p != "" {
			return p
		}
		return str("filePath")
	case "webfetch":
		return str("url")
	}
	return t.Name
}

func prettyArgs(raw string) string {
	var v any
	if json.Unmarshal([]byte(raw), &v) != nil {
		return raw
	}
	out, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return raw
	}
	s := string(out)
	if len(s) > 2000 {
		s = s[:2000] + "…"
	}
	return s
}

// Compact trims old context immediately and returns how many messages were
// dropped. It is the manual /compact entry point.
func (a *Agent) Compact() int {
	return a.compactIfNeeded(true)
}

// compactIfNeeded trims old tool output when the history grows too large.
// Returns the number of messages removed.
func (a *Agent) compactIfNeeded(force ...bool) int {
	a.mu.Lock()
	defer a.mu.Unlock()
	forced := len(force) > 0 && force[0]
	msgs := a.Session.Messages
	if len(msgs) < 12 {
		return 0
	}
	threshold := a.CompactAfter
	if threshold <= 0 {
		threshold = 30
	}
	total := 0
	for _, m := range msgs {
		total += len(m.Content)
	}
	if !forced && (len(msgs) < threshold || total < 260_000) {
		return 0
	}
	keepTail := 24
	if forced {
		keepTail = 12
	}
	cut := len(msgs) - keepTail
	if cut <= 1 {
		return 0
	}
	// Keep the first user message for intent, then drop the middle chunk.
	head := msgs[:1]
	tail := msgs[cut:]
	note := llm.Message{
		Role:    "user",
		Content: fmt.Sprintf("（上下文已压缩：省略了 %d 条较早的消息与工具输出。继续完成任务，必要时重新读取文件。）", cut-1),
	}
	out := make([]llm.Message, 0, len(head)+len(tail)+1)
	out = append(out, head...)
	out = append(out, note)
	out = append(out, tail...)
	a.Session.Messages = out
	return cut - 1
}
