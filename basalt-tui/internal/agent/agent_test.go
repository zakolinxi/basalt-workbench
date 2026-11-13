package agent

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/juang/basalt-tui/internal/config"
	"github.com/juang/basalt-tui/internal/llm"
	"github.com/juang/basalt-tui/internal/session"
	"github.com/juang/basalt-tui/internal/tools"
)

type fakeClient struct {
	rounds [][]llm.Event
	round  int
}

func (f *fakeClient) Label() string { return "fake" }

func (f *fakeClient) Stream(ctx context.Context, req llm.Request, emit func(llm.Event) error) error {
	if f.round >= len(f.rounds) {
		return emit(llm.Event{Kind: llm.KindDone})
	}
	for _, ev := range f.rounds[f.round] {
		if err := emit(ev); err != nil {
			return err
		}
	}
	f.round++
	return nil
}

type allowAll struct{ asked int }

func (a *allowAll) Approve(ctx context.Context, req Approval) Decision {
	a.asked++
	return DecisionOnce
}

func TestRunExecutesToolsAndLoops(t *testing.T) {
	dir := t.TempDir()
	env := tools.NewEnv(dir, "s", t.TempDir())
	reg := tools.NewRegistry()
	sess := session.New(dir, "fake", "fake-model")
	ag := New(&fakeClient{rounds: [][]llm.Event{
		{
			{Kind: llm.KindText, Text: "先写文件。"},
			{Kind: llm.KindToolCall, ToolCall: &llm.ToolCall{
				ID: "c1", Name: "write",
				Arguments: `{"path":"hello.txt","content":"hello basalt\n"}`,
			}},
			{Kind: llm.KindUsage, InputTokens: 10, OutputTokens: 5},
		},
		{
			{Kind: llm.KindText, Text: "已写入 hello.txt。"},
		},
	}}, reg, env, sess)
	ag.System = SystemPrompt(dir, reg)
	approver := &allowAll{}
	ag.Approver = approver

	var toolEnds int
	ag.Emit = func(ev Event) {
		if ev.Kind == EvToolEnd {
			toolEnds++
		}
	}
	if err := ag.Run(context.Background(), "创建一个文件"); err != nil {
		t.Fatal(err)
	}
	if approver.asked != 1 {
		t.Fatalf("expected one approval, got %d", approver.asked)
	}
	if toolEnds != 1 {
		t.Fatalf("expected one tool end event, got %d", toolEnds)
	}
	raw, err := os.ReadFile(filepath.Join(dir, "hello.txt"))
	if err != nil || string(raw) != "hello basalt\n" {
		t.Fatalf("file not written: %q %v", raw, err)
	}
	if sess.Title != "创建一个文件" {
		t.Fatalf("unexpected title %q", sess.Title)
	}
	// user + assistant(tool call) + tool + assistant(final)
	if len(sess.Messages) != 4 {
		t.Fatalf("unexpected message count %d", len(sess.Messages))
	}
	if sess.Messages[2].Role != "tool" || !strings.Contains(sess.Messages[2].Content, "hello.txt") {
		t.Fatalf("unexpected tool message %+v", sess.Messages[2])
	}
	if sess.Usage.Input != 10 || sess.Usage.Output != 5 {
		t.Fatalf("unexpected usage %+v", sess.Usage)
	}
}

type denyAll struct{}

func (denyAll) Approve(ctx context.Context, req Approval) Decision { return DecisionDeny }

func TestDeniedToolIsReportedBack(t *testing.T) {
	dir := t.TempDir()
	env := tools.NewEnv(dir, "s", t.TempDir())
	reg := tools.NewRegistry()
	sess := session.New(dir, "fake", "fake-model")
	ag := New(&fakeClient{rounds: [][]llm.Event{
		{{Kind: llm.KindToolCall, ToolCall: &llm.ToolCall{
			ID: "c1", Name: "write", Arguments: `{"path":"x.txt","content":"x"}`,
		}}},
		{{Kind: llm.KindText, Text: "好的，不写。"}},
	}}, reg, env, sess)
	ag.Approver = denyAll{}
	ag.Emit = func(Event) {}
	if err := ag.Run(context.Background(), "写个文件"); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(dir, "x.txt")); !os.IsNotExist(err) {
		t.Fatal("file must not be written after denial")
	}
	found := false
	for _, m := range sess.Messages {
		if m.Role == "tool" && strings.Contains(m.Content, "用户拒绝") {
			found = true
		}
	}
	if !found {
		t.Fatalf("denial should be fed back to the model: %+v", sess.Messages)
	}
}

func TestAutoApproveSkipsSecondPrompt(t *testing.T) {
	dir := t.TempDir()
	env := tools.NewEnv(dir, "s", t.TempDir())
	reg := tools.NewRegistry()
	sess := session.New(dir, "fake", "fake-model")
	ag := New(&fakeClient{rounds: [][]llm.Event{
		{{Kind: llm.KindToolCall, ToolCall: &llm.ToolCall{
			ID: "c1", Name: "write", Arguments: `{"path":"a.txt","content":"1"}`,
		}}},
		{{Kind: llm.KindToolCall, ToolCall: &llm.ToolCall{
			ID: "c2", Name: "write", Arguments: `{"path":"b.txt","content":"2"}`,
		}}},
		{{Kind: llm.KindText, Text: "done"}},
	}}, reg, env, sess)
	approver := &allowAll{}
	ag.Approver = approver
	ag.AutoApprove("write")
	ag.Emit = func(Event) {}
	if err := ag.Run(context.Background(), "写两个文件"); err != nil {
		t.Fatal(err)
	}
	if approver.asked != 0 {
		t.Fatalf("auto-approved tool must not prompt, got %d prompts", approver.asked)
	}
	for _, name := range []string{"a.txt", "b.txt"} {
		if _, err := os.Stat(filepath.Join(dir, name)); err != nil {
			t.Fatalf("%s missing: %v", name, err)
		}
	}
}

func TestSystemPromptIncludesInstructions(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "AGENTS.md"), []byte("必须使用中文回答"), 0o644); err != nil {
		t.Fatal(err)
	}
	reg := tools.NewRegistry()
	p := SystemPrompt(dir, reg)
	if !strings.Contains(p, "必须使用中文回答") {
		t.Fatal("project instructions missing from system prompt")
	}
	if !strings.Contains(p, "update_plan") {
		t.Fatal("tool list missing from system prompt")
	}
	_ = config.Config{}
}
