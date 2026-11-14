package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
)

func pickerModel(t *testing.T) *Model {
	t.Helper()
	m := newTestModel(t)
	m.cfg.Providers = demoProviders()
	m.cfg.Default = "demo-deepseek"
	m.providerKey = "demo-deepseek"
	m.preset.Model = "deepseek-chat"
	m.preset.Name = "DeepSeek"
	return m
}

func TestModelPickerOpens(t *testing.T) {
	m := pickerModel(t)
	m.submit("/model")
	if m.picker == nil {
		t.Fatal("/model should open the picker")
	}
	vis := m.picker.visible()
	if len(vis) == 0 {
		t.Fatal("picker has no items")
	}
	if !vis[0].header {
		t.Fatal("first row should be a provider header")
	}
	// The current model should be preselected.
	if vis[m.picker.index].model != "deepseek-chat" {
		t.Fatalf("expected current model preselected, got %+v", vis[m.picker.index])
	}
}

func TestModelPickerSkipsHeaders(t *testing.T) {
	m := pickerModel(t)
	m.openModelPicker()
	for i := 0; i < 12; i++ {
		m.picker.move(1)
		vis := m.picker.visible()
		if vis[m.picker.index].header {
			t.Fatalf("selection landed on a header at step %d", i)
		}
	}
	m.picker.move(-1)
	if m.picker.visible()[m.picker.index].header {
		t.Fatal("selection landed on a header while moving up")
	}
}

func TestModelPickerFilter(t *testing.T) {
	m := pickerModel(t)
	m.openModelPicker()
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("gemini")})
	vis := m.picker.visible()
	if len(vis) != 3 {
		t.Fatalf("expected one header and two gemini models, got %d", len(vis))
	}
	for _, item := range vis[1:] {
		if !strings.Contains(strings.ToLower(item.model), "gemini") {
			t.Fatalf("unexpected filtered item %+v", item)
		}
	}
	// Backspace clears the filter and restores every group.
	for i := 0; i < 8; i++ {
		m.Update(tea.KeyMsg{Type: tea.KeyBackspace})
	}
	if len(m.picker.visible()) <= 3 {
		t.Fatal("backspace should broaden the result set")
	}
}

func TestModelPickerEnterSwitches(t *testing.T) {
	m := pickerModel(t)
	m.openModelPicker()
	m.picker.filter = []rune("deepseek-reasoner")
	m.picker.resetIndex()
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if m.picker != nil {
		t.Fatal("picker should close after selection")
	}
	if m.preset.Model != "deepseek-reasoner" {
		t.Fatalf("model not switched: %q", m.preset.Model)
	}
	if m.providerKey != "demo-deepseek" {
		t.Fatalf("provider not switched: %q", m.providerKey)
	}
}

func TestModelPickerEscCloses(t *testing.T) {
	m := pickerModel(t)
	m.openModelPicker()
	m.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if m.picker != nil {
		t.Fatal("esc should close the picker")
	}
	// The model must be unchanged.
	if m.preset.Model != "deepseek-chat" {
		t.Fatalf("esc changed the model: %q", m.preset.Model)
	}
}

func TestModelPickerRenders(t *testing.T) {
	m := pickerModel(t)
	m.openModelPicker()
	m.prepare(120, 38)
	view := stripANSI(m.View())
	if !strings.Contains(view, "选择模型") {
		t.Fatal("picker title missing from view")
	}
	if !strings.Contains(view, "deepseek-chat") || !strings.Contains(view, "claude-sonnet-4-5") {
		t.Fatal("picker rows missing from view")
	}
}
