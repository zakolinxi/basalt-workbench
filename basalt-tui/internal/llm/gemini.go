package llm

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/juang/basalt-tui/internal/config"
)

// Gemini talks to the Google Generative Language streaming endpoint.
type Gemini struct {
	preset config.ProviderConfig
}

func NewGemini(p config.ProviderConfig) *Gemini { return &Gemini{preset: p} }

func (c *Gemini) Label() string { return c.preset.Model }

type geminiPart struct {
	Text             string          `json:"text,omitempty"`
	FunctionCall     *geminiCall     `json:"functionCall,omitempty"`
	FunctionResponse *geminiResponse `json:"functionResponse,omitempty"`
}

type geminiCall struct {
	Name string          `json:"name"`
	Args json.RawMessage `json:"args,omitempty"`
}

type geminiResponse struct {
	Name     string         `json:"name"`
	Response map[string]any `json:"response"`
}

type geminiContent struct {
	Role  string       `json:"role"`
	Parts []geminiPart `json:"parts"`
}

func toGeminiContents(in []Message) []geminiContent {
	var out []geminiContent
	appendContent := func(role string, parts []geminiPart) {
		if len(parts) == 0 {
			return
		}
		if n := len(out); n > 0 && out[n-1].Role == role {
			out[n-1].Parts = append(out[n-1].Parts, parts...)
			return
		}
		out = append(out, geminiContent{Role: role, Parts: parts})
	}
	for _, m := range in {
		switch m.Role {
		case "system":
			continue
		case "tool":
			appendContent("user", []geminiPart{{
				FunctionResponse: &geminiResponse{
					Name:     m.Name,
					Response: map[string]any{"result": m.Content},
				},
			}})
		case "assistant":
			var parts []geminiPart
			if strings.TrimSpace(m.Content) != "" {
				parts = append(parts, geminiPart{Text: m.Content})
			}
			for _, tc := range m.ToolCalls {
				parts = append(parts, geminiPart{FunctionCall: &geminiCall{
					Name: tc.Name, Args: json.RawMessage(firstNonEmpty(tc.Arguments, "{}")),
				}})
			}
			appendContent("model", parts)
		default:
			appendContent("user", []geminiPart{{Text: m.Content}})
		}
	}
	return out
}

func (c *Gemini) Stream(ctx context.Context, req Request, emit func(Event) error) error {
	model := firstNonEmpty(req.Model, c.preset.Model)
	payload := map[string]any{
		"contents": toGeminiContents(req.Messages),
		"generationConfig": map[string]any{
			"maxOutputTokens": firstNonZero(req.MaxTokens, c.preset.MaxTokens),
		},
	}
	if req.Temperature > 0 {
		payload["generationConfig"].(map[string]any)["temperature"] = req.Temperature
	}
	if strings.TrimSpace(req.System) != "" {
		payload["systemInstruction"] = map[string]any{
			"parts": []map[string]string{{"text": req.System}},
		}
	}
	if len(req.Tools) > 0 {
		decls := make([]map[string]any, 0, len(req.Tools))
		for _, t := range req.Tools {
			decls = append(decls, map[string]any{
				"name": t.Name, "description": t.Description, "parameters": t.Parameters,
			})
		}
		payload["tools"] = []map[string]any{{"functionDeclarations": decls}}
	}

	url := fmt.Sprintf("%s/models/%s:streamGenerateContent?alt=sse", c.preset.BaseURL, model)
	headers := map[string]string{}
	if c.preset.APIKey != "" {
		headers["x-goog-api-key"] = c.preset.APIKey
	}
	headers = withHeaders(headers, c.preset.Headers)

	return withRetry(ctx, emit, func() error {
		resp, err := post(ctx, url, headers, payload)
		if err != nil {
			return err
		}
		defer resp.Body.Close()

		stop := ""
		totalIn, totalOut, totalCached := 0, 0, 0
		seen := map[string]bool{}
		callIndex := 0

		err = scanSSE(resp.Body, func(data []byte) error {
			var chunk struct {
				Candidates []struct {
					Content struct {
						Parts []struct {
							Text         string      `json:"text"`
							FunctionCall *geminiCall `json:"functionCall"`
						} `json:"parts"`
					} `json:"content"`
					FinishReason string `json:"finishReason"`
				} `json:"candidates"`
				UsageMetadata *struct {
					PromptTokenCount        int `json:"promptTokenCount"`
					CandidatesTokenCount    int `json:"candidatesTokenCount"`
					CachedContentTokenCount int `json:"cachedContentTokenCount"`
				} `json:"usageMetadata"`
				Error *struct {
					Message string `json:"message"`
				} `json:"error"`
			}
			if err := json.Unmarshal(data, &chunk); err != nil {
				return nil
			}
			if chunk.Error != nil && chunk.Error.Message != "" {
				return fmt.Errorf("%s", chunk.Error.Message)
			}
			for _, cand := range chunk.Candidates {
				for _, part := range cand.Content.Parts {
					if part.Text != "" {
						if err := emit(Event{Kind: KindText, Text: part.Text}); err != nil {
							return err
						}
					}
					if part.FunctionCall != nil && part.FunctionCall.Name != "" {
						args := strings.TrimSpace(string(part.FunctionCall.Args))
						if args == "" {
							args = "{}"
						}
						sig := part.FunctionCall.Name + "|" + args
						if seen[sig] {
							continue
						}
						seen[sig] = true
						callIndex++
						if err := emit(Event{Kind: KindToolCall, ToolCall: &ToolCall{
							ID: fmt.Sprintf("gemini_%d", callIndex), Name: part.FunctionCall.Name, Arguments: args,
						}}); err != nil {
							return err
						}
					}
				}
				if cand.FinishReason != "" {
					stop = cand.FinishReason
				}
			}
			if chunk.UsageMetadata != nil {
				if chunk.UsageMetadata.PromptTokenCount > totalIn {
					totalIn = chunk.UsageMetadata.PromptTokenCount
				}
				if chunk.UsageMetadata.CandidatesTokenCount > totalOut {
					totalOut = chunk.UsageMetadata.CandidatesTokenCount
				}
				if chunk.UsageMetadata.CachedContentTokenCount > totalCached {
					totalCached = chunk.UsageMetadata.CachedContentTokenCount
				}
			}
			return nil
		})
		if err != nil {
			return err
		}
		if totalIn > 0 || totalOut > 0 {
			if err := emit(Event{Kind: KindUsage, InputTokens: totalIn, OutputTokens: totalOut, CachedTokens: totalCached}); err != nil {
				return err
			}
		}
		return emit(Event{Kind: KindDone, StopReason: stop})
	})
}
