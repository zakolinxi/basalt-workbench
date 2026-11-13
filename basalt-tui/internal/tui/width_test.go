package tui

import (
	"strings"
	"testing"

	"github.com/charmbracelet/lipgloss"
)

func TestFrameWidths(t *testing.T) {
	scenes, err := DemoScenes(120, 38, "..")
	if err != nil {
		t.Fatal(err)
	}
	for _, sc := range scenes {
		frame := sc.RenderFrame()
		for i, line := range strings.Split(frame, "\n") {
			w := lipgloss.Width(line)
			if w > 120 {
				t.Errorf("%s line %d width=%d %q", sc.Name, i, w, stripANSI(line))
			}
		}
	}
}

func TestApprovalPopupWidths(t *testing.T) {
	scenes, err := DemoScenes(120, 38, "..")
	if err != nil {
		t.Fatal(err)
	}
	for _, sc := range scenes {
		if sc.Name != "03-approval" {
			continue
		}
		popup := sc.Model.renderApproval()
		for i, line := range strings.Split(popup, "\n") {
			t.Logf("popup line %2d width=%d %q", i, lipgloss.Width(line), stripANSI(line))
		}
	}
}
