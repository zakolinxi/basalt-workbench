package llm

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/juang/basalt-tui/internal/config"
)

func TestGeminiStream(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.Contains(r.URL.Path, ":streamGenerateContent") {
			t.Errorf("unexpected path %s", r.URL.Path)
		}
		if r.Header.Get("x-goog-api-key") != "g-key" {
			t.Errorf("missing gemini api key")
		}
		w.Header().Set("Content-Type", "text/event-stream")
		chunks := []string{
			`{"candidates":[{"content":{"parts":[{"text":"你好"}]}}]}`,
			`{"candidates":[{"content":{"parts":[{"text":"，世界"},{"functionCall":{"name":"read","args":{"path":"a.txt"}}}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":9,"candidatesTokenCount":4,"cachedContentTokenCount":2}}`,
		}
		for _, c := range chunks {
			_, _ = w.Write([]byte("data: " + c + "\n\n"))
		}
	}))
	defer srv.Close()

	client := NewGemini(config.ProviderConfig{
		Kind: "gemini", BaseURL: srv.URL, Model: "gemini-test", APIKey: "g-key", MaxTokens: 128,
	})
	var text strings.Builder
	var calls []ToolCall
	in, out, cached := 0, 0, 0
	err := client.Stream(context.Background(), Request{Model: "gemini-test"}, func(ev Event) error {
		switch ev.Kind {
		case KindText:
			text.WriteString(ev.Text)
		case KindToolCall:
			calls = append(calls, *ev.ToolCall)
		case KindUsage:
			in, out, cached = ev.InputTokens, ev.OutputTokens, ev.CachedTokens
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
		t.Fatalf("unexpected calls %+v", calls)
	}
	if in != 9 || out != 4 || cached != 2 {
		t.Fatalf("unexpected usage in=%d out=%d cached=%d", in, out, cached)
	}
}

func TestResponsesStream(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasSuffix(r.URL.Path, "/responses") {
			t.Errorf("unexpected path %s", r.URL.Path)
		}
		if r.Header.Get("Authorization") != "Bearer r-key" {
			t.Errorf("missing bearer")
		}
		w.Header().Set("Content-Type", "text/event-stream")
		events := []string{
			`{"type":"response.output_text.delta","delta":"我来"}`,
			`{"type":"response.output_text.delta","delta":"读文件。"}`,
			`{"type":"response.output_item.added","item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"read"}}`,
			`{"type":"response.function_call_arguments.delta","item_id":"fc_1","delta":"{\"path\":"}`,
			`{"type":"response.function_call_arguments.delta","item_id":"fc_1","delta":"\"a.txt\"}"}`,
			`{"type":"response.completed","response":{"status":"completed","usage":{"input_tokens":11,"output_tokens":6,"input_tokens_details":{"cached_tokens":2}}}}`,
		}
		for _, e := range events {
			_, _ = w.Write([]byte("data: " + e + "\n\n"))
		}
	}))
	defer srv.Close()

	client := NewResponses(config.ProviderConfig{
		Kind: "openai_responses", BaseURL: srv.URL, Model: "resp-test", APIKey: "r-key", MaxTokens: 128,
	})
	var text strings.Builder
	var calls []ToolCall
	in, out, cached := 0, 0, 0
	stop := ""
	err := client.Stream(context.Background(), Request{Model: "resp-test"}, func(ev Event) error {
		switch ev.Kind {
		case KindText:
			text.WriteString(ev.Text)
		case KindToolCall:
			calls = append(calls, *ev.ToolCall)
		case KindUsage:
			in, out, cached = ev.InputTokens, ev.OutputTokens, ev.CachedTokens
		case KindDone:
			stop = ev.StopReason
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if text.String() != "我来读文件。" {
		t.Fatalf("unexpected text %q", text.String())
	}
	if len(calls) != 1 || calls[0].Name != "read" || calls[0].Arguments != `{"path":"a.txt"}` || calls[0].ID != "call_1" {
		t.Fatalf("unexpected calls %+v", calls)
	}
	if in != 11 || out != 6 || cached != 2 {
		t.Fatalf("unexpected usage in=%d out=%d cached=%d", in, out, cached)
	}
	if stop != "completed" {
		t.Fatalf("unexpected stop %q", stop)
	}
}
