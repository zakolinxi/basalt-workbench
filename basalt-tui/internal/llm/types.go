// Package llm contains the streaming chat clients used by the agent loop.
package llm

import (
	"context"
	"encoding/json"
)

// Message is the provider-neutral chat message.
type Message struct {
	Role       string     `json:"role"` // system | user | assistant | tool
	Content    string     `json:"content"`
	ToolCalls  []ToolCall `json:"toolCalls,omitempty"`
	ToolCallID string     `json:"toolCallId,omitempty"`
	Name       string     `json:"name,omitempty"`
}

// ToolCall is a single requested function invocation.
type ToolCall struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Arguments string `json:"arguments"` // raw JSON object
}

// Tool is a JSON-schema tool definition.
type Tool struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	Parameters  json.RawMessage `json:"parameters"`
}

// Request describes one completion turn.
type Request struct {
	Model       string
	System      string
	Messages    []Message
	Tools       []Tool
	MaxTokens   int
	Temperature float64
	Reasoning   string
}

// Kind identifies a stream event.
type Kind int

const (
	KindText Kind = iota
	KindThinking
	KindToolCall
	KindUsage
	KindDone
)

// Event is emitted by Client.Stream.
type Event struct {
	Kind         Kind
	Text         string
	ToolCall     *ToolCall
	InputTokens  int
	OutputTokens int
	CachedTokens int
	StopReason   string
}

// Client is a streaming chat backend.
type Client interface {
	// Stream blocks until the completion finishes, the context is cancelled,
	// or an error occurs. emit is called synchronously.
	Stream(ctx context.Context, req Request, emit func(Event) error) error
	// Label describes the backend for the status bar.
	Label() string
}
