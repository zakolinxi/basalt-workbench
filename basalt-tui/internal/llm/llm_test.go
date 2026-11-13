package llm

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/juang/basalt-tui/internal/config"
)

func TestOpenAIStream(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasSuffix(r.URL.Path, "/chat/completions") {
			t.Errorf("unexpected path %s", r.URL.Path)
		}
		if got := r.Header.Get("Authorization"); got != "Bearer test-key" {
			t.Errorf("missing bearer token, got %q", got)
		}
		w.Header().Set("Content-Type", "text/event-stream")
		chunks := []string{
			`{"choices":[{"delta":{"content":"你好，"}}]}`,
			`{"choices":[{"delta":{"content":"世界"}}]}`,
			`{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read","arguments":"{\"path\":"}}]}}]}`,
			`{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"a.txt\"}"}}]},"finish_reason":"tool_calls"}]}`,
			`{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":7,"prompt_tokens_details":{"cached_tokens":3}}}`,
		}
		for _, c := range chunks {
			_, _ = w.Write([]byte("data: " + c + "\n\n"))
		}
		_, _ = w.Write([]byte("data: [DONE]\n\n"))
	}))
	defer srv.Close()

	client := NewOpenAI(config.ProviderConfig{
		Kind: "openai", BaseURL: srv.URL, Model: "test", APIKey: "test-key", MaxTokens: 128,
	})
	var text strings.Builder
	var calls []ToolCall
	var usage Event
	var done bool
	err := client.Stream(context.Background(), Request{Model: "test"}, func(ev Event) error {
		switch ev.Kind {
		case KindText:
			text.WriteString(ev.Text)
		case KindToolCall:
			calls = append(calls, *ev.ToolCall)
		case KindUsage:
			usage = ev
		case KindDone:
			done = true
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if text.String() != "你好，世界" {
		t.Fatalf("unexpected text %q", text.String())
	}
	if len(calls) != 1 || calls[0].Name != "read" || calls[0].Arguments != `{"path":"a.txt"}` {
		t.Fatalf("unexpected tool calls %+v", calls)
	}
	if usage.InputTokens != 12 || usage.OutputTokens != 7 || usage.CachedTokens != 3 {
		t.Fatalf("unexpected usage %+v", usage)
	}
	if !done {
		t.Fatal("expected done event")
	}
}

func TestAnthropicStream(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasSuffix(r.URL.Path, "/v1/messages") {
			t.Errorf("unexpected path %s", r.URL.Path)
		}
		if r.Header.Get("x-api-key") != "anth-key" {
			t.Errorf("missing api key")
		}
		w.Header().Set("Content-Type", "text/event-stream")
		events := []string{
			`{"type":"message_start","message":{"usage":{"input_tokens":21,"cache_read_input_tokens":4}}}`,
			`{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}`,
			`{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hello"}}`,
			`{"type":"content_block_stop","index":0}`,
			`{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"edit"}}`,
			`{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"a.ts\"}"}}`,
			`{"type":"content_block_stop","index":1}`,
			`{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":9}}`,
			`{"type":"message_stop"}`,
		}
		for _, e := range events {
			_, _ = w.Write([]byte("event: x\ndata: " + e + "\n\n"))
		}
	}))
	defer srv.Close()

	client := NewAnthropic(config.ProviderConfig{
		Kind: "anthropic", BaseURL: srv.URL, Model: "claude", APIKey: "anth-key", MaxTokens: 128,
	})
	var text strings.Builder
	var calls []ToolCall
	var in, out, cached int
	stop := ""
	err := client.Stream(context.Background(), Request{Model: "claude"}, func(ev Event) error {
		switch ev.Kind {
		case KindText:
			text.WriteString(ev.Text)
		case KindToolCall:
			calls = append(calls, *ev.ToolCall)
		case KindUsage:
			if ev.InputTokens > 0 {
				in = ev.InputTokens
				cached = ev.CachedTokens
			}
			if ev.OutputTokens > 0 {
				out = ev.OutputTokens
			}
		case KindDone:
			stop = ev.StopReason
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if text.String() != "hello" {
		t.Fatalf("unexpected text %q", text.String())
	}
	if len(calls) != 1 || calls[0].Name != "edit" || calls[0].ID != "toolu_1" {
		t.Fatalf("unexpected tool calls %+v", calls)
	}
	if in != 21 || out != 9 || cached != 4 {
		t.Fatalf("unexpected usage in=%d out=%d cached=%d", in, out, cached)
	}
	if stop != "tool_use" {
		t.Fatalf("unexpected stop reason %q", stop)
	}
}

func TestHTTPErrorSurfaced(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, `{"error":{"message":"bad key"}}`, http.StatusUnauthorized)
	}))
	defer srv.Close()
	client := NewOpenAI(config.ProviderConfig{Kind: "openai", BaseURL: srv.URL, Model: "x", APIKey: "k"})
	err := client.Stream(context.Background(), Request{}, func(Event) error { return nil })
	if err == nil || !strings.Contains(err.Error(), "401") {
		t.Fatalf("expected 401 error, got %v", err)
	}
}
