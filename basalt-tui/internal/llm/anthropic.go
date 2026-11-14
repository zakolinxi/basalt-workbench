package llm

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/juang/basalt-tui/internal/config"
)

// Anthropic implements the Messages API streaming protocol.
type Anthropic struct {
	preset config.ProviderConfig
}

func NewAnthropic(p config.ProviderConfig) *Anthropic { return &Anthropic{preset: p} }

func (c *Anthropic) Label() string { return c.preset.Model }

type anBlock struct {
	Type      string          `json:"type"`
	Text      string          `json:"text,omitempty"`
	ID        string          `json:"id,omitempty"`
	Name      string          `json:"name,omitempty"`
	Input     json.RawMessage `json:"input,omitempty"`
	ToolUseID string          `json:"tool_use_id,omitempty"`
	Content   any             `json:"content,omitempty"`
	IsError   bool            `json:"is_error,omitempty"`
}

type anMessage struct {
	Role    string    `json:"role"`
	Content []anBlock `json:"content"`
}

type anTool struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	InputSchema json.RawMessage `json:"input_schema"`
}

func toAnMessages(in []Message) []anMessage {
	out := make([]anMessage, 0, len(in))
	appendMsg := func(role string, blocks []anBlock) {
		if len(blocks) == 0 {
			return
		}
		// The API requires alternating roles; merge neighbours defensively.
		if n := len(out); n > 0 && out[n-1].Role == role {
			out[n-1].Content = append(out[n-1].Content, blocks...)
			return
		}
		out = append(out, anMessage{Role: role, Content: blocks})
	}
	for _, m := range in {
		switch m.Role {
		case "system":
			continue
		case "tool":
			appendMsg("user", []anBlock{{
				Type:      "tool_result",
				ToolUseID: m.ToolCallID,
				Content:   m.Content,
			}})
		case "assistant":
			var blocks []anBlock
			if strings.TrimSpace(m.Content) != "" {
				blocks = append(blocks, anBlock{Type: "text", Text: m.Content})
			}
			for _, tc := range m.ToolCalls {
				blocks = append(blocks, anBlock{
					Type: "tool_use", ID: tc.ID, Name: tc.Name,
					Input: json.RawMessage(firstNonEmpty(tc.Arguments, "{}")),
				})
			}
			appendMsg("assistant", blocks)
		default:
			appendMsg("user", []anBlock{{Type: "text", Text: m.Content}})
		}
	}
	return out
}

func (c *Anthropic) Stream(ctx context.Context, req Request, emit func(Event) error) error {
	payload := map[string]any{
		"model":      firstNonEmpty(req.Model, c.preset.Model),
		"max_tokens": firstNonZero(req.MaxTokens, c.preset.MaxTokens),
		"messages":   toAnMessages(req.Messages),
		"stream":     true,
	}
	if strings.TrimSpace(req.System) != "" {
		payload["system"] = req.System
	}
	if req.Temperature > 0 {
		payload["temperature"] = req.Temperature
	}
	if len(req.Tools) > 0 {
		tools := make([]anTool, 0, len(req.Tools))
		for _, t := range req.Tools {
			tools = append(tools, anTool{Name: t.Name, Description: t.Description, InputSchema: t.Parameters})
		}
		payload["tools"] = tools
	}

	headers := map[string]string{
		"x-api-key":         c.preset.APIKey,
		"anthropic-version": "2023-06-01",
	}
	headers = withHeaders(headers, c.preset.Headers)

	return withRetry(ctx, emit, func() error {
		resp, err := post(ctx, c.preset.BaseURL+"/v1/messages", headers, payload)
		if err != nil {
			return err
		}
		defer resp.Body.Close()

		type blockState struct {
			kind  string
			name  string
			id    string
			args  strings.Builder
			index int
		}
		blocks := map[int]*blockState{}
		stop := ""

		err = scanSSE(resp.Body, func(data []byte) error {
			var ev struct {
				Type    string `json:"type"`
				Index   int    `json:"index"`
				Message *struct {
					Usage struct {
						InputTokens          int `json:"input_tokens"`
						CacheReadInputTokens int `json:"cache_read_input_tokens"`
					} `json:"usage"`
				} `json:"message"`
				ContentBlock *struct {
					Type string `json:"type"`
					ID   string `json:"id"`
					Name string `json:"name"`
				} `json:"content_block"`
				Delta *struct {
					Type        string `json:"type"`
					Text        string `json:"text"`
					Thinking    string `json:"thinking"`
					PartialJSON string `json:"partial_json"`
					StopReason  string `json:"stop_reason"`
				} `json:"delta"`
				Usage *struct {
					OutputTokens int `json:"output_tokens"`
				} `json:"usage"`
				Error *struct {
					Message string `json:"message"`
				} `json:"error"`
			}
			if err := json.Unmarshal(data, &ev); err != nil {
				return nil
			}
			switch ev.Type {
			case "error":
				if ev.Error != nil {
					return fmt.Errorf("%s", ev.Error.Message)
				}
				return fmt.Errorf("anthropic stream error")
			case "message_start":
				if ev.Message != nil {
					if err := emit(Event{Kind: KindUsage, InputTokens: ev.Message.Usage.InputTokens, CachedTokens: ev.Message.Usage.CacheReadInputTokens}); err != nil {
						return err
					}
				}
			case "content_block_start":
				if ev.ContentBlock != nil {
					blocks[ev.Index] = &blockState{kind: ev.ContentBlock.Type, id: ev.ContentBlock.ID, name: ev.ContentBlock.Name, index: ev.Index}
				}
			case "content_block_delta":
				if ev.Delta == nil {
					return nil
				}
				switch ev.Delta.Type {
				case "text_delta":
					if ev.Delta.Text != "" {
						if err := emit(Event{Kind: KindText, Text: ev.Delta.Text}); err != nil {
							return err
						}
					}
				case "thinking_delta":
					if ev.Delta.Thinking != "" {
						if err := emit(Event{Kind: KindThinking, Text: ev.Delta.Thinking}); err != nil {
							return err
						}
					}
				case "input_json_delta":
					if b := blocks[ev.Index]; b != nil {
						b.args.WriteString(ev.Delta.PartialJSON)
					}
				}
			case "content_block_stop":
				if b := blocks[ev.Index]; b != nil && b.kind == "tool_use" && b.name != "" {
					args := strings.TrimSpace(b.args.String())
					if args == "" {
						args = "{}"
					}
					if err := emit(Event{Kind: KindToolCall, ToolCall: &ToolCall{ID: b.id, Name: b.name, Arguments: args}}); err != nil {
						return err
					}
					delete(blocks, ev.Index)
				}
			case "message_delta":
				if ev.Delta != nil && ev.Delta.StopReason != "" {
					stop = ev.Delta.StopReason
				}
				if ev.Usage != nil && ev.Usage.OutputTokens > 0 {
					if err := emit(Event{Kind: KindUsage, OutputTokens: ev.Usage.OutputTokens}); err != nil {
						return err
					}
				}
			}
			return nil
		})
		if err != nil {
			return err
		}
		return emit(Event{Kind: KindDone, StopReason: stop})
	})
}
