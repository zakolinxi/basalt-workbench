package tui

import (
	"strings"
	"testing"
)

func TestContextBadge(t *testing.T) {
	m := newTestModel(t)
	m.preset.ContextLength = 131072
	m.ctxTokens = 65536
	m.cacheTokens = 32768
	badge := stripANSI(m.contextBadge(false))
	if !strings.Contains(badge, "64k/128k") {
		t.Fatalf("unexpected usage label: %q", badge)
	}
	if !strings.Contains(badge, "缓存 50%") {
		t.Fatalf("unexpected cache label: %q", badge)
	}
	if !strings.Contains(badge, "▰") {
		t.Fatalf("expected a filled bar: %q", badge)
	}
}

func TestContextBadgeEmpty(t *testing.T) {
	m := newTestModel(t)
	m.preset.ContextLength = 131072
	badge := stripANSI(m.contextBadge(false))
	if !strings.Contains(badge, "0/128k") {
		t.Fatalf("empty badge should show the limit: %q", badge)
	}
	if strings.Contains(badge, "缓存") {
		t.Fatalf("empty badge should not show cache: %q", badge)
	}
}

func TestContextBadgeCompactAtHighUsage(t *testing.T) {
	m := newTestModel(t)
	m.preset.ContextLength = 100000
	m.ctxTokens = 90000
	badge := stripANSI(m.contextBadge(true))
	if !strings.Contains(badge, "ctx 90%") {
		t.Fatalf("unexpected compact badge: %q", badge)
	}
}

func TestFooterFitsWidth(t *testing.T) {
	for _, width := range []int{120, 100, 80, 60} {
		m := newTestModel(t)
		m.preset.ContextLength = 131072
		m.ctxTokens = 90000
		m.cacheTokens = 30000
		m.prepare(width, 38)
		footer := m.renderFooter()
		if got := displayWidth(footer); got > width {
			t.Fatalf("width %d: footer is %d columns: %q", width, got, stripANSI(footer))
		}
	}
}

func TestFooterBadgeOnRight(t *testing.T) {
	m := newTestModel(t)
	m.preset.ContextLength = 131072
	m.ctxTokens = 65536
	m.cacheTokens = 32768
	m.prepare(120, 38)
	footer := stripANSI(m.renderFooter())
	stateIdx := strings.Index(footer, "就绪")
	badgeIdx := strings.Index(footer, "ctx ")
	if stateIdx < 0 || badgeIdx < 0 {
		t.Fatalf("missing footer parts: %q", footer)
	}
	if badgeIdx < stateIdx {
		t.Fatalf("context badge should be on the right: %q", footer)
	}
	if badgeIdx < len(footer)/2 {
		t.Fatalf("context badge should sit near the right edge: %q", footer)
	}
}

func TestTokenLabel(t *testing.T) {
	cases := map[int]string{
		0:       "0",
		999:     "999",
		8420:    "8.2k",
		131072:  "128k",
		1048576: "1.0M",
	}
	for in, want := range cases {
		if got := tokenLabel(in); got != want {
			t.Fatalf("tokenLabel(%d)=%q, want %q", in, got, want)
		}
	}
}
