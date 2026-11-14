package llm

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/juang/basalt-tui/internal/config"
)

// Responses implements the OpenAI Responses API.
type Responses struct {
	preset config.ProviderConfig
}

func NewResponses(p config.ProviderConfig) *Responses { return &Responses{preset: p} }

func (c *Responses) Label() string { return c.preset.Model }

func toResponsesInput(in []Message) []any {
	var out []any
	for _, m := range in {
		switch m.Role {
		case "system":
			continue
		case "tool":
			out = append(out, map[string]any{
				"type": "function_call_output", "call_id": m.ToolCallID, "output": m.Content,
			})
		case "assistant":
			if strings.TrimSpace(m.Content) != "" {
				out = append(out, map[string]any{
					"role": "assistant",
					"content": []map[string]string{
						{"type": "output_text", "text": m.Content},
					},
				})
			}
			for _, tc := range m.ToolCalls {
				out = append(out, map[string]any{
					"type": "function_call", "call_id": tc.ID,
					"name": tc.Name, "arguments": tc.Arguments,
				})
			}
		default:
			out = append(out, map[string]any{
				"role": "user",
				"content": []map[string]string{
					{"type": "input_text", "text": m.Content},
				},
			})
		}
	}
	return out
}

func (c *Responses) Stream(ctx context.Context, req Request, emit func(Event) error) error {
	payload := map[string]any{
		"model":             firstNonEmpty(req.Model, c.preset.Model),
		"input":             toResponsesInput(req.Messages),
		"stream":            true,
		"max_output_tokens": firstNonZero(req.MaxTokens, c.preset.MaxTokens),
	}
	if strings.TrimSpace(req.System) != "" {
		payload["instructions"] = req.System
	}
	if req.Temperature > 0 {
		payload["temperature"] = req.Temperature
	}
	if len(req.Tools) > 0 {
		tools := make([]map[string]any, 0, len(req.Tools))
		for _, t := range req.Tools {
			tools = append(tools, map[string]any{
				"type": "function", "name": t.Name,
				"description": t.Description, "parameters": t.Parameters,
			})
		}
		payload["tools"] = tools
	}

	headers := map[string]string{}
	if c.preset.APIKey != "" {
		headers["Authorization"] = "Bearer " + c.preset.APIKey
	}
	headers = withHeaders(headers, c.preset.Headers)

	return withRetry(ctx, emit, func() error {
		resp, err := post(ctx, c.preset.BaseURL+"/responses", headers, payload)
		if err != nil {
			return err
		}
		defer resp.Body.Close()

		type acc struct {
			callID string
			name   string
			args   strings.Builder
		}
		calls := map[string]*acc{}
		var order []string
		stop := ""
		in, out, cached := 0, 0, 0

		ensure := func(itemID string) *acc {
			a := calls[itemID]
			if a == nil {
				a = &acc{}
				calls[itemID] = a
				order = append(order, itemID)
			}
			return a
		}

		err = scanSSE(resp.Body, func(data []byte) error {
			var ev struct {
				Type   string `json:"type"`
				ItemID string `json:"item_id"`
				Delta  string `json:"delta"`
				Item   *struct {
					ID     string `json:"id"`
					Type   string `json:"type"`
					CallID string `json:"call_id"`
					Name   string `json:"name"`
				} `json:"item"`
				Response *struct {
					Status string `json:"status"`
					Usage  *struct {
						InputTokens        int `json:"input_tokens"`
						OutputTokens       int `json:"output_tokens"`
						InputTokensDetails *struct {
							CachedTokens int `json:"cached_tokens"`
						} `json:"input_tokens_details"`
					} `json:"usage"`
				} `json:"response"`
				Message string `json:"message"`
			}
			if err := json.Unmarshal(data, &ev); err != nil {
				return nil
			}
			switch ev.Type {
			case "response.output_text.delta":
				if ev.Delta != "" {
					return emit(Event{Kind: KindText, Text: ev.Delta})
				}
			case "response.output_item.added":
				if ev.Item != nil && ev.Item.Type == "function_call" {
					a := ensure(ev.Item.ID)
					a.callID = ev.Item.CallID
					a.name = ev.Item.Name
				}
			case "response.function_call_arguments.delta":
				ensure(ev.ItemID).args.WriteString(ev.Delta)
			case "response.function_call_arguments.done":
				if ev.Delta != "" {
					a := ensure(ev.ItemID)
					a.args.Reset()
					a.args.WriteString(ev.Delta)
				}
			case "response.completed":
				if ev.Response != nil {
					if ev.Response.Usage != nil {
						in = ev.Response.Usage.InputTokens
						out = ev.Response.Usage.OutputTokens
						if ev.Response.Usage.InputTokensDetails != nil {
							cached = ev.Response.Usage.InputTokensDetails.CachedTokens
						}
					}
					stop = ev.Response.Status
				}
			case "response.failed", "error":
				msg := ev.Message
				if msg == "" {
					msg = "responses stream failed"
				}
				return fmt.Errorf("%s", msg)
			}
			return nil
		})
		if err != nil {
			return err
		}

		n := 0
		for _, id := range order {
			a := calls[id]
			if a.name == "" {
				continue
			}
			args := strings.TrimSpace(a.args.String())
			if args == "" {
				args = "{}"
			}
			callID := a.callID
			if callID == "" {
				n++
				callID = fmt.Sprintf("call_%d", n)
			}
			if err := emit(Event{Kind: KindToolCall, ToolCall: &ToolCall{
				ID: callID, Name: a.name, Arguments: args,
			}}); err != nil {
				return err
			}
		}
		if in > 0 || out > 0 {
			if err := emit(Event{Kind: KindUsage, InputTokens: in, OutputTokens: out, CachedTokens: cached}); err != nil {
				return err
			}
		}
		return emit(Event{Kind: KindDone, StopReason: stop})
	})
}
