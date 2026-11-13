package llm

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	"github.com/juang/basalt-tui/internal/config"
)

// OpenAI implements the widely-cloned Chat Completions streaming protocol.
type OpenAI struct {
	preset config.ProviderConfig
}

func NewOpenAI(p config.ProviderConfig) *OpenAI { return &OpenAI{preset: p} }

func (c *OpenAI) Label() string { return c.preset.Model }

type oaToolCall struct {
	ID       string `json:"id,omitempty"`
	Type     string `json:"type,omitempty"`
	Function struct {
		Name      string `json:"name"`
		Arguments string `json:"arguments"`
	} `json:"function"`
}

type oaMessage struct {
	Role       string       `json:"role"`
	Content    string       `json:"content"`
	ToolCalls  []oaToolCall `json:"tool_calls,omitempty"`
	ToolCallID string       `json:"tool_call_id,omitempty"`
	Name       string       `json:"name,omitempty"`
}

type oaRequest struct {
	Model         string          `json:"model"`
	Messages      []oaMessage     `json:"messages"`
	Tools         []oaTool        `json:"tools,omitempty"`
	Stream        bool            `json:"stream"`
	StreamOptions *oaStreamOption `json:"stream_options,omitempty"`
	MaxTokens     int             `json:"max_tokens,omitempty"`
	Temperature   *float64        `json:"temperature,omitempty"`
	Reasoning     *oaReasoning    `json:"reasoning,omitempty"`
}

type oaStreamOption struct {
	IncludeUsage bool `json:"include_usage"`
}

type oaReasoning struct {
	Effort string `json:"effort"`
}

type oaTool struct {
	Type     string `json:"type"`
	Function struct {
		Name        string          `json:"name"`
		Description string          `json:"description"`
		Parameters  json.RawMessage `json:"parameters"`
	} `json:"function"`
}

func toOAMessages(system string, in []Message) []oaMessage {
	out := make([]oaMessage, 0, len(in)+1)
	if strings.TrimSpace(system) != "" {
		out = append(out, oaMessage{Role: "system", Content: system})
	}
	for _, m := range in {
		om := oaMessage{Role: m.Role, Content: m.Content, ToolCallID: m.ToolCallID, Name: m.Name}
		for _, tc := range m.ToolCalls {
			oc := oaToolCall{ID: tc.ID, Type: "function"}
			oc.Function.Name = tc.Name
			oc.Function.Arguments = tc.Arguments
			om.ToolCalls = append(om.ToolCalls, oc)
		}
		if om.Role == "" {
			om.Role = "user"
		}
		out = append(out, om)
	}
	return out
}

func (c *OpenAI) Stream(ctx context.Context, req Request, emit func(Event) error) error {
	payload := oaRequest{
		Model:         firstNonEmpty(req.Model, c.preset.Model),
		Messages:      toOAMessages(req.System, req.Messages),
		Stream:        true,
		StreamOptions: &oaStreamOption{IncludeUsage: true},
		MaxTokens:     firstNonZero(req.MaxTokens, c.preset.MaxTokens),
	}
	if s := firstNonEmpty(req.Reasoning, c.preset.Reasoning); s != "" {
		payload.Reasoning = &oaReasoning{Effort: s}
	}
	if req.Temperature > 0 {
		t := req.Temperature
		payload.Temperature = &t
	}
	for _, t := range req.Tools {
		var ot oaTool
		ot.Type = "function"
		ot.Function.Name = t.Name
		ot.Function.Description = t.Description
		ot.Function.Parameters = t.Parameters
		payload.Tools = append(payload.Tools, ot)
	}

	headers := map[string]string{}
	if c.preset.APIKey != "" {
		headers["Authorization"] = "Bearer " + c.preset.APIKey
	}

	return withRetry(ctx, emit, func() error {
		resp, err := post(ctx, c.preset.BaseURL+"/chat/completions", headers, payload)
		if err != nil {
			return err
		}
		defer resp.Body.Close()

		type accumulator struct {
			id    string
			name  string
			args  strings.Builder
			index int
		}
		partial := map[int]*accumulator{}
		stop := ""
		var usage *Event

		err = scanSSE(resp.Body, func(data []byte) error {
			var chunk struct {
				Choices []struct {
					Delta struct {
						Content          string    `json:"content"`
						ReasoningContent string    `json:"reasoning_content"`
						Reasoning        string    `json:"reasoning"`
						ToolCalls        []oaDelta `json:"tool_calls"`
					} `json:"delta"`
					FinishReason *string `json:"finish_reason"`
				} `json:"choices"`
				Usage *struct {
					PromptTokens        int `json:"prompt_tokens"`
					CompletionTokens    int `json:"completion_tokens"`
					PromptTokensDetails *struct {
						CachedTokens int `json:"cached_tokens"`
					} `json:"prompt_tokens_details"`
				} `json:"usage"`
				Error *struct {
					Message string `json:"message"`
				} `json:"error"`
			}
			if err := json.Unmarshal(data, &chunk); err != nil {
				return nil // tolerate keep-alives and vendor noise
			}
			if chunk.Error != nil && chunk.Error.Message != "" {
				return fmt.Errorf("%s", chunk.Error.Message)
			}
			if chunk.Usage != nil {
				u := Event{Kind: KindUsage, InputTokens: chunk.Usage.PromptTokens, OutputTokens: chunk.Usage.CompletionTokens}
				if chunk.Usage.PromptTokensDetails != nil {
					u.CachedTokens = chunk.Usage.PromptTokensDetails.CachedTokens
				}
				usage = &u
			}
			for _, choice := range chunk.Choices {
				if t := choice.Delta.ReasoningContent + choice.Delta.Reasoning; t != "" {
					if err := emit(Event{Kind: KindThinking, Text: t}); err != nil {
						return err
					}
				}
				if choice.Delta.Content != "" {
					if err := emit(Event{Kind: KindText, Text: choice.Delta.Content}); err != nil {
						return err
					}
				}
				for _, tc := range choice.Delta.ToolCalls {
					acc := partial[tc.Index]
					if acc == nil {
						acc = &accumulator{index: tc.Index}
						partial[tc.Index] = acc
					}
					if tc.ID != "" {
						acc.id = tc.ID
					}
					if tc.Function.Name != "" && acc.name == "" {
						acc.name = tc.Function.Name
					}
					acc.args.WriteString(tc.Function.Arguments)
				}
				if choice.FinishReason != nil && *choice.FinishReason != "" {
					stop = *choice.FinishReason
				}
			}
			return nil
		})
		if err != nil {
			return err
		}

		indexes := make([]int, 0, len(partial))
		for i := range partial {
			indexes = append(indexes, i)
		}
		sort.Ints(indexes)
		for _, i := range indexes {
			acc := partial[i]
			if acc.name == "" {
				continue
			}
			id := acc.id
			if id == "" {
				id = fmt.Sprintf("call_%d", i)
			}
			args := strings.TrimSpace(acc.args.String())
			if args == "" {
				args = "{}"
			}
			if err := emit(Event{Kind: KindToolCall, ToolCall: &ToolCall{ID: id, Name: acc.name, Arguments: args}}); err != nil {
				return err
			}
		}
		if usage != nil {
			usage.StopReason = stop
			if err := emit(*usage); err != nil {
				return err
			}
		}
		return emit(Event{Kind: KindDone, StopReason: stop})
	})
}

type oaDelta struct {
	Index    int    `json:"index"`
	ID       string `json:"id"`
	Type     string `json:"type"`
	Function struct {
		Name      string `json:"name"`
		Arguments string `json:"arguments"`
	} `json:"function"`
}

func firstNonEmpty(a, b string) string {
	if a != "" {
		return a
	}
	return b
}

func firstNonZero(a, b int) int {
	if a != 0 {
		return a
	}
	return b
}
