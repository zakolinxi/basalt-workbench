package tui

import (
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/juang/basalt-tui/internal/agent"
	"github.com/juang/basalt-tui/internal/config"
)

func newTestModel(t *testing.T) *Model {
	t.Helper()
	m, err := New(t.TempDir(), config.Load(""), true)
	if err != nil {
		t.Fatal(err)
	}
	m.Update(tea.WindowSizeMsg{Width: 120, Height: 38})
	return m
}

func TestSlashHelpOpensOverlay(t *testing.T) {
	m := newTestModel(t)
	m.submit("/help")
	if !m.showHelp {
		t.Fatal("/help should open the help overlay")
	}
	if m.streaming {
		t.Fatal("/help must not start an agent run")
	}
}

func TestSlashToolsAndModelListing(t *testing.T) {
	m := newTestModel(t)
	m.submit("/tools")
	m.submit("/cost")
	if len(m.items) != 2 {
		t.Fatalf("expected two notices, got %d", len(m.items))
	}
	for _, it := range m.items {
		if it.Kind != ItemNotice {
			t.Fatalf("expected notice items, got %v", it.Kind)
		}
	}
	m.submit("/model")
	if m.picker == nil {
		t.Fatal("/model should open the interactive picker")
	}
}

func TestSlashMenuFilters(t *testing.T) {
	m := newTestModel(t)
	m.slash.open = true
	m.slash.query = "mod"
	m.filterSlash()
	if len(m.slash.filter) == 0 {
		t.Fatal("expected slash matches for 'mod'")
	}
	if m.slash.filter[0].cmd != "/model" {
		t.Fatalf("unexpected first match %s", m.slash.filter[0].cmd)
	}
}

func TestApprovalKeys(t *testing.T) {
	m := newTestModel(t)
	reply := make(chan agent.Decision, 1)
	m.approval = &approvalState{
		req:   agent.Approval{Tool: "write", Title: "x.txt"},
		reply: reply,
	}
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}})
	select {
	case d := <-reply:
		if d != agent.DecisionAlways {
			t.Fatalf("expected always decision, got %v", d)
		}
	default:
		t.Fatal("approval channel was not answered")
	}
	if m.approval != nil {
		t.Fatal("approval state should be cleared")
	}
}

func TestUndoDeletesNewFile(t *testing.T) {
	m := newTestModel(t)
	path := filepath.Join(m.cwd, "demo.txt")
	m.env.Remember(path)
	if err := os.WriteFile(path, []byte("changed\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	m.submit("/undo")
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatal("undo should remove a file that did not exist before")
	}
}

func TestViewFitsTerminal(t *testing.T) {
	m := newTestModel(t)
	m.prepare(120, 38)
	view := m.View()
	if got := len(strings.Split(view, "\n")); got != 38 {
		t.Fatalf("view should be exactly 38 lines, got %d", got)
	}
	for i, line := range strings.Split(view, "\n") {
		if w := displayWidth(line); w > 120 {
			t.Fatalf("line %d is %d columns wide", i, w)
		}
	}
}

func TestProgramLifecycle(t *testing.T) {
	m := newTestModel(t)
	p := tea.NewProgram(m,
		tea.WithInput(strings.NewReader("\x03")),
		tea.WithOutput(io.Discard),
		tea.WithoutSignalHandler(),
	)
	done := make(chan error, 1)
	go func() {
		_, err := p.Run()
		done <- err
	}()
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(5 * time.Second):
		p.Kill()
		t.Fatal("program did not exit on ctrl+c")
	}
}

func TestWelcomeAndConversationViews(t *testing.T) {
	scenes, err := DemoScenes(120, 38, "..")
	if err != nil {
		t.Fatal(err)
	}
	if len(scenes) != 7 {
		t.Fatalf("expected 7 demo scenes, got %d", len(scenes))
	}
	for _, sc := range scenes {
		frame := sc.RenderFrame()
		if strings.TrimSpace(frame) == "" {
			t.Fatalf("%s rendered nothing", sc.Name)
		}
		if !strings.Contains(frame, "basalt") {
			t.Fatalf("%s is missing the brand header", sc.Name)
		}
	}
}
