package tui

import (
	"fmt"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
)

func longItems(n int) []Item {
	items := make([]Item, 0, n)
	for i := 0; i < n; i++ {
		items = append(items, Item{Kind: ItemAssistant, Text: fmt.Sprintf("第 %d 条历史消息\n\n这是用来撑高对话区的正文内容，方便验证滚动行为。", i+1)})
	}
	return items
}

func TestThinkingCollapsedByDefault(t *testing.T) {
	m := newTestModel(t)
	m.items = []Item{{Kind: ItemThinking, Text: "先看登录实现\n第二行细节"}}
	m.prepare(120, 38)
	view := stripANSI(m.View())
	if !strings.Contains(view, "ctrl+t 展开") {
		t.Fatal("collapsed thinking hint missing")
	}
	if strings.Contains(view, "第二行细节") {
		t.Fatal("collapsed thinking should hide the body")
	}
}

func TestThinkingToggle(t *testing.T) {
	m := newTestModel(t)
	m.items = []Item{{Kind: ItemThinking, Text: "先看登录实现\n第二行细节"}}
	m.prepare(120, 38)
	m.Update(tea.KeyMsg{Type: tea.KeyCtrlT})
	if !m.showThinking {
		t.Fatal("ctrl+t should expand thinking")
	}
	view := stripANSI(m.View())
	if !strings.Contains(view, "第二行细节") {
		t.Fatal("expanded thinking should show the body")
	}
	if !strings.Contains(view, "ctrl+t 收起") {
		t.Fatal("expanded hint missing")
	}
	m.Update(tea.KeyMsg{Type: tea.KeyCtrlT})
	if m.showThinking {
		t.Fatal("second ctrl+t should collapse thinking")
	}
}

func TestScrollWithWheel(t *testing.T) {
	m := newTestModel(t)
	m.items = longItems(40)
	m.prepare(120, 38)
	m.refreshViewport(true)
	if !m.viewport.AtBottom() {
		t.Fatal("expected to start at the bottom")
	}
	m.Update(tea.MouseMsg{Button: tea.MouseButtonWheelUp, Action: tea.MouseActionPress})
	if m.autoScroll {
		t.Fatal("scrolling up should pause auto-follow")
	}
	if m.viewport.AtBottom() {
		t.Fatal("wheel up should move away from the bottom")
	}
	view := stripANSI(m.View())
	if !strings.Contains(view, "浏览历史") {
		t.Fatal("footer should report the scroll state")
	}
	m.Update(tea.KeyMsg{Type: tea.KeyEnd})
	if !m.autoScroll || !m.viewport.AtBottom() {
		t.Fatal("end should return to the bottom")
	}
}

func TestArrowScrollsWhenComposerEmpty(t *testing.T) {
	m := newTestModel(t)
	m.items = longItems(40)
	m.prepare(120, 38)
	m.refreshViewport(true)
	m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if m.viewport.AtBottom() {
		t.Fatal("up arrow should scroll the transcript")
	}
	if m.input.Value() != "" {
		t.Fatal("arrow scrolling must not touch the composer")
	}
}

func TestArrowsStayInComposerWhenTyping(t *testing.T) {
	m := newTestModel(t)
	m.items = longItems(40)
	m.prepare(120, 38)
	m.refreshViewport(true)
	m.input.SetValue("一段正在编辑的文字")
	m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if !m.viewport.AtBottom() {
		t.Fatal("the transcript should not scroll while editing")
	}
}

func TestStreamingDoesNotYankUserBack(t *testing.T) {
	m := newTestModel(t)
	m.items = longItems(40)
	m.prepare(120, 38)
	m.refreshViewport(true)
	m.Update(tea.MouseMsg{Button: tea.MouseButtonWheelUp, Action: tea.MouseActionPress})
	offset := m.viewport.YOffset
	m.streaming = true
	m.items = append(m.items, Item{Kind: ItemAssistant, Text: "流式输出中"})
	m.refreshViewport(m.autoScroll)
	if m.viewport.YOffset != offset {
		t.Fatalf("stream refresh moved the viewport: %d -> %d", offset, m.viewport.YOffset)
	}
}
