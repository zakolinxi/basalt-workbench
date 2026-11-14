package tui

import (
	"strings"
	"testing"

	"github.com/juang/basalt-tui/internal/agent"
)

func TestLateThinkingIsInsertedBeforeAnswer(t *testing.T) {
	m := newTestModel(t)
	m.streaming = true
	m.handleAgentEvent(agent.Event{Kind: agent.EvText, Text: "这是最终回答。"})
	m.handleAgentEvent(agent.Event{Kind: agent.EvThinking, Text: "补充一段迟到的推理。"})

	if len(m.items) != 2 {
		t.Fatalf("expected two items, got %d", len(m.items))
	}
	if m.items[0].Kind != ItemThinking {
		t.Fatalf("thinking should be first, got %v", m.items[0].Kind)
	}
	if m.items[1].Kind != ItemAssistant || !strings.Contains(m.items[1].Text, "最终回答") {
		t.Fatalf("answer should follow thinking: %+v", m.items[1])
	}
}

func TestThinkingBeforeAnswerStaysFirst(t *testing.T) {
	m := newTestModel(t)
	m.streaming = true
	m.handleAgentEvent(agent.Event{Kind: agent.EvThinking, Text: "先推理。"})
	m.handleAgentEvent(agent.Event{Kind: agent.EvText, Text: "再回答。"})

	if len(m.items) != 2 || m.items[0].Kind != ItemThinking || m.items[1].Kind != ItemAssistant {
		t.Fatalf("unexpected order: %+v", m.items)
	}
}

func TestLateThinkingAfterToolRound(t *testing.T) {
	m := newTestModel(t)
	m.streaming = true
	m.handleAgentEvent(agent.Event{Kind: agent.EvThinking, Text: "第一轮思考"})
	m.handleAgentEvent(agent.Event{Kind: agent.EvText, Text: "第一轮回答"})
	m.handleAgentEvent(agent.Event{Kind: agent.EvToolStart, Tool: &agent.ToolEvent{ID: "c1", Name: "read"}})
	m.handleAgentEvent(agent.Event{Kind: agent.EvToolEnd, Tool: &agent.ToolEvent{ID: "c1", Name: "read", Title: "read"}})

	m.handleAgentEvent(agent.Event{Kind: agent.EvText, Text: "第二轮回答"})
	m.handleAgentEvent(agent.Event{Kind: agent.EvThinking, Text: "第二轮迟到的思考"})

	kinds := make([]ItemKind, len(m.items))
	for i, it := range m.items {
		kinds[i] = it.Kind
	}
	want := []ItemKind{ItemThinking, ItemAssistant, ItemTool, ItemThinking, ItemAssistant}
	if len(kinds) != len(want) {
		t.Fatalf("unexpected item count: %v", kinds)
	}
	for i := range want {
		if kinds[i] != want[i] {
			t.Fatalf("item %d = %v, want %v (%v)", i, kinds[i], want[i], kinds)
		}
	}
}
