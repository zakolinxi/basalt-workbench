package llm

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/juang/basalt-tui/internal/config"
)

// Ollama speaks the native /api/chat NDJSON protocol.
type Ollama struct {
	preset config.ProviderConfig
}

func NewOllama(p config.ProviderConfig) *Ollama { return &Ollama{preset: p} }

func (c *Ollama) Label() string { return c.preset.Model }

type olMessage struct {
	Role      string      `json:"role"`
	Content   string      `json:"content"`
	ToolCalls []olToolUse `json:"tool_calls,omitempty"`
	ToolName  string      `json:"tool_name,omitempty"`
}

type olToolUse struct {
	Function struct {
		Name      string          `json:"name"`
		Arguments json.RawMessage `json:"arguments"`
	} `json:"function"`
}

type olTool struct {
	Type     string `json:"type"`
	Function struct {
		Name        string          `json:"name"`
		Description string          `json:"description"`
		Parameters  json.RawMessage `json:"parameters"`
	} `json:"function"`
}

func (c *Ollama) Stream(ctx context.Context, req Request, emit func(Event) error) error {
	msgs := make([]olMessage, 0, len(req.Messages)+1)
	if strings.TrimSpace(req.System) != "" {
		msgs = append(msgs, olMessage{Role: "system", Content: req.System})
	}
	for _, m := range req.Messages {
		om := olMessage{Role: m.Role, Content: m.Content, ToolName: m.Name}
		for _, tc := range m.ToolCalls {
			var t olToolUse
			t.Function.Name = tc.Name
			t.Function.Arguments = json.RawMessage(firstNonEmpty(tc.Arguments, "{}"))
			om.ToolCalls = append(om.ToolCalls, t)
		}
		if om.Role == "tool" {
			// Ollama expects the tool name on the message it answers.
			if om.ToolName == "" {
				om.ToolName = m.Name
			}
		}
		msgs = append(msgs, om)
	}
	tools := make([]olTool, 0, len(req.Tools))
	for _, t := range req.Tools {
		var ot olTool
		ot.Type = "function"
		ot.Function.Name = t.Name
		ot.Function.Description = t.Description
		ot.Function.Parameters = t.Parameters
		tools = append(tools, ot)
	}
	payload := map[string]any{
		"model":    firstNonEmpty(req.Model, c.preset.Model),
		"messages": msgs,
		"stream":   true,
		"options": map[string]any{
			"num_predict": firstNonZero(req.MaxTokens, c.preset.MaxTokens),
		},
	}
	if len(tools) > 0 {
		payload["tools"] = tools
	}

	return withRetry(ctx, emit, func() error {
		resp, err := post(ctx, c.preset.BaseURL+"/api/chat", nil, payload)
		if err != nil {
			return err
		}
		defer resp.Body.Close()

		sc := bufio.NewScanner(resp.Body)
		sc.Buffer(make([]byte, 0, 64*1024), 8*1024*1024)
		for sc.Scan() {
			line := strings.TrimSpace(sc.Text())
			if line == "" {
				continue
			}
			var chunk struct {
				Message struct {
					Role      string      `json:"role"`
					Content   string      `json:"content"`
					Thinking  string      `json:"thinking"`
					ToolCalls []olToolUse `json:"tool_calls"`
				} `json:"message"`
				Done            bool   `json:"done"`
				DoneReason      string `json:"done_reason"`
				PromptEvalCount int    `json:"prompt_eval_count"`
				EvalCount       int    `json:"eval_count"`
				Error           string `json:"error"`
			}
			if err := json.Unmarshal([]byte(line), &chunk); err != nil {
				continue
			}
			if chunk.Error != "" {
				return fmt.Errorf("%s", chunk.Error)
			}
			if chunk.Message.Thinking != "" {
				if err := emit(Event{Kind: KindThinking, Text: chunk.Message.Thinking}); err != nil {
					return err
				}
			}
			if chunk.Message.Content != "" {
				if err := emit(Event{Kind: KindText, Text: chunk.Message.Content}); err != nil {
					return err
				}
			}
			for i, tc := range chunk.Message.ToolCalls {
				args := string(tc.Function.Arguments)
				if strings.TrimSpace(args) == "" {
					args = "{}"
				}
				call := &ToolCall{ID: fmt.Sprintf("ollama_%d_%d", len(c.preset.Model), i), Name: tc.Function.Name, Arguments: args}
				if err := emit(Event{Kind: KindToolCall, ToolCall: call}); err != nil {
					return err
				}
			}
			if chunk.Done {
				if err := emit(Event{Kind: KindUsage, InputTokens: chunk.PromptEvalCount, OutputTokens: chunk.EvalCount}); err != nil {
					return err
				}
				return emit(Event{Kind: KindDone, StopReason: chunk.DoneReason})
			}
		}
		if err := sc.Err(); err != nil {
			return err
		}
		return emit(Event{Kind: KindDone})
	})
}
