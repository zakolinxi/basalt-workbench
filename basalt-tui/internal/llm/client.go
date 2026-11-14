package llm

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/juang/basalt-tui/internal/config"
)

// New builds the right client for a provider preset.
func New(p config.ProviderConfig) (Client, error) {
	switch p.Kind {
	case "openai_chat", "openai", "":
		return NewOpenAI(p), nil
	case "openai_responses":
		return NewResponses(p), nil
	case "anthropic":
		return NewAnthropic(p), nil
	case "gemini":
		return NewGemini(p), nil
	case "ollama":
		return NewOllama(p), nil
	default:
		return nil, fmt.Errorf("unsupported provider kind %q", p.Kind)
	}
}

// withHeaders merges provider-configured headers over protocol headers.
func withHeaders(base, extra map[string]string) map[string]string {
	if len(extra) == 0 {
		return base
	}
	out := make(map[string]string, len(base)+len(extra))
	for k, v := range base {
		out[k] = v
	}
	for k, v := range extra {
		if v != "" {
			out[k] = v
		}
	}
	return out
}

var httpClient = &http.Client{
	// No global timeout: streams run for as long as the model needs. Callers
	// cancel through the context.
	Transport: &http.Transport{
		Proxy:                 http.ProxyFromEnvironment,
		MaxIdleConns:          32,
		MaxIdleConnsPerHost:   8,
		IdleConnTimeout:       90 * time.Second,
		ResponseHeaderTimeout: 90 * time.Second,
	},
}

type httpError struct {
	Status int
	Body   string
}

func (e *httpError) Error() string {
	body := strings.TrimSpace(e.Body)
	if len(body) > 400 {
		body = body[:400] + "…"
	}
	return fmt.Sprintf("HTTP %d: %s", e.Status, body)
}

func retryable(err error) bool {
	var he *httpError
	if errors.As(err, &he) {
		return he.Status == 408 || he.Status == 409 || he.Status == 429 || he.Status >= 500
	}
	return errors.Is(err, io.ErrUnexpectedEOF) || errors.Is(err, context.DeadlineExceeded)
}

// withRetry runs fn up to three times for transient failures. Streaming is
// restarted from scratch, so the caller must discard partial text it saw.
func withRetry(ctx context.Context, emit func(Event) error, fn func() error) error {
	var last error
	for attempt := 0; attempt < 3; attempt++ {
		if attempt > 0 {
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(time.Duration(attempt) * 1200 * time.Millisecond):
			}
			_ = emit(Event{Kind: KindThinking, Text: fmt.Sprintf("\n· 连接中断，正在重试 (%d/2)…\n", attempt)})
		}
		last = fn()
		if last == nil || ctx.Err() != nil || !retryable(last) {
			return last
		}
	}
	return last
}

func post(ctx context.Context, url string, headers map[string]string, payload any) (*http.Response, error) {
	raw, err := json.Marshal(payload)
	if err != nil {
		return nil, err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(raw))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "text/event-stream")
	for k, v := range headers {
		req.Header.Set(k, v)
	}
	resp, err := httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode < 200 || resp.StatusCode > 299 {
		defer resp.Body.Close()
		body, _ := io.ReadAll(io.LimitReader(resp.Body, 8192))
		return nil, &httpError{Status: resp.StatusCode, Body: string(body)}
	}
	return resp, nil
}

// scanSSE reads a server-sent-event stream and calls onData for each payload.
func scanSSE(r io.Reader, onData func([]byte) error) error {
	sc := bufio.NewScanner(r)
	sc.Buffer(make([]byte, 0, 64*1024), 4*1024*1024)
	for sc.Scan() {
		line := strings.TrimRight(sc.Text(), "\r")
		if line == "" || strings.HasPrefix(line, ":") {
			continue
		}
		data, ok := strings.CutPrefix(line, "data:")
		if !ok {
			continue
		}
		data = strings.TrimSpace(data)
		if data == "[DONE]" {
			return nil
		}
		if err := onData([]byte(data)); err != nil {
			return err
		}
	}
	return sc.Err()
}
