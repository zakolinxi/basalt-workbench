package agent

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"sync/atomic"
	"testing"

	"github.com/juang/basalt-tui/internal/config"
	"github.com/juang/basalt-tui/internal/llm"
	"github.com/juang/basalt-tui/internal/session"
	"github.com/juang/basalt-tui/internal/tools"
)

// TestEndToEndStreamingProvider runs the real OpenAI client against a fake
// server: first round asks for a file write, second round reports success.
func TestEndToEndStreamingProvider(t *testing.T) {
	var calls int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		n := atomic.AddInt32(&calls, 1)
		w.Header().Set("Content-Type", "text/event-stream")
		switch n {
		case 1:
			fmt.Fprint(w, "data: {\"choices\":[{\"delta\":{\"content\":\"我来创建文件。\"}}]}\n\n")
			fmt.Fprint(w, "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"write\",\"arguments\":\"{\\\"path\\\":\\\"note.md\\\",\\\"content\\\":\\\"# hi\\\\n\\\"}\"}}]}}]}\n\n")
			fmt.Fprint(w, "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n")
			fmt.Fprint(w, "data: [DONE]\n\n")
		default:
			fmt.Fprint(w, "data: {\"choices\":[{\"delta\":{\"content\":\"已创建 note.md。\"}}]}\n\n")
			fmt.Fprint(w, "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n")
			fmt.Fprint(w, "data: [DONE]\n\n")
		}
	}))
	defer srv.Close()

	dir := t.TempDir()
	env := tools.NewEnv(dir, "e2e", t.TempDir())
	reg := tools.NewRegistry()
	sess := session.New(dir, "fake", "test-model")
	client, err := llm.New(config.ProviderConfig{
		Kind: "openai", BaseURL: srv.URL, Model: "test-model", APIKey: "k", MaxTokens: 512,
	})
	if err != nil {
		t.Fatal(err)
	}
	ag := New(client, reg, env, sess)
	ag.System = "test"
	ag.Approver = &allowAll{}
	var text string
	ag.Emit = func(ev Event) {
		if ev.Kind == EvText {
			text += ev.Text
		}
	}
	if err := ag.Run(context.Background(), "写一个 note.md"); err != nil {
		t.Fatal(err)
	}
	raw, err := os.ReadFile(filepath.Join(dir, "note.md"))
	if err != nil || string(raw) != "# hi\n" {
		t.Fatalf("note.md not written correctly: %q %v", raw, err)
	}
	if text != "我来创建文件。已创建 note.md。" {
		t.Fatalf("unexpected streamed text %q", text)
	}
	if atomic.LoadInt32(&calls) != 2 {
		t.Fatalf("expected two rounds, got %d", calls)
	}
}
