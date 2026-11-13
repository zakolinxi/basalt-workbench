// Package diff implements a small line-level LCS diff used for edit previews,
// approval cards and /undo summaries.
package diff

import (
	"fmt"
	"strings"
)

// Op is one diff line.
type Op struct {
	Kind byte // ' ' context, '-' delete, '+' add
	Text string
}

// Lines computes the diff between two texts.
func Lines(before, after string) []Op {
	a := splitLines(before)
	b := splitLines(after)

	// Trim the common prefix and suffix so the DP table stays small for edits
	// in the middle of large files.
	start := 0
	for start < len(a) && start < len(b) && a[start] == b[start] {
		start++
	}
	endA, endB := len(a), len(b)
	for endA > start && endB > start && a[endA-1] == b[endB-1] {
		endA--
		endB--
	}
	midA, midB := a[start:endA], b[start:endB]

	ops := make([]Op, 0, len(a)+len(b))
	for _, line := range a[:start] {
		ops = append(ops, Op{' ', line})
	}
	if len(midA) > 0 || len(midB) > 0 {
		// Guard against pathological inputs from generated files.
		if len(midA)*len(midB) > 4_000_000 {
			for _, l := range midA {
				ops = append(ops, Op{'-', l})
			}
			for _, l := range midB {
				ops = append(ops, Op{'+', l})
			}
		} else {
			ops = append(ops, lcsDiff(midA, midB)...)
		}
	}
	for _, line := range a[endA:] {
		ops = append(ops, Op{' ', line})
	}
	return ops
}

func lcsDiff(a, b []string) []Op {
	n, m := len(a), len(b)
	dp := make([][]int, n+1)
	for i := range dp {
		dp[i] = make([]int, m+1)
	}
	for i := n - 1; i >= 0; i-- {
		for j := m - 1; j >= 0; j-- {
			if a[i] == b[j] {
				dp[i][j] = dp[i+1][j+1] + 1
			} else if dp[i+1][j] >= dp[i][j+1] {
				dp[i][j] = dp[i+1][j]
			} else {
				dp[i][j] = dp[i][j+1]
			}
		}
	}
	ops := make([]Op, 0, n+m)
	i, j := 0, 0
	for i < n && j < m {
		switch {
		case a[i] == b[j]:
			ops = append(ops, Op{' ', a[i]})
			i++
			j++
		case dp[i+1][j] >= dp[i][j+1]:
			ops = append(ops, Op{'-', a[i]})
			i++
		default:
			ops = append(ops, Op{'+', b[j]})
			j++
		}
	}
	for ; i < n; i++ {
		ops = append(ops, Op{'-', a[i]})
	}
	for ; j < m; j++ {
		ops = append(ops, Op{'+', b[j]})
	}
	return ops
}

func splitLines(s string) []string {
	s = strings.ReplaceAll(s, "\r\n", "\n")
	if s == "" {
		return nil
	}
	lines := strings.Split(s, "\n")
	// A trailing newline should not produce a phantom empty line.
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	return lines
}

// Stats counts added and removed lines.
func Stats(ops []Op) (added, removed int) {
	for _, op := range ops {
		switch op.Kind {
		case '+':
			added++
		case '-':
			removed++
		}
	}
	return
}

type hunk struct{ from, to int }

// Unified renders a unified diff with hunks and correct line numbers.
func Unified(path string, ops []Op, context int) string {
	if context <= 0 {
		context = 3
	}
	added, removed := Stats(ops)
	if added == 0 && removed == 0 {
		return ""
	}
	var b strings.Builder
	fmt.Fprintf(&b, "--- a/%s\n+++ b/%s\n", path, path)

	// Build hunks: expand each run of changes by `context`, then merge runs
	// that touch so overlapping context is not printed twice.
	var hunks []hunk
	for i, op := range ops {
		if op.Kind == ' ' {
			continue
		}
		from, to := i-context, i+context
		if from < 0 {
			from = 0
		}
		if to > len(ops)-1 {
			to = len(ops) - 1
		}
		if n := len(hunks); n > 0 && from <= hunks[n-1].to+1 {
			hunks[n-1].to = to
		} else {
			hunks = append(hunks, hunk{from: from, to: to})
		}
	}

	oldPos, newPos := 1, 1
	cursor := 0
	for _, h := range hunks {
		for ; cursor < h.from; cursor++ {
			switch ops[cursor].Kind {
			case ' ':
				oldPos++
				newPos++
			case '-':
				oldPos++
			case '+':
				newPos++
			}
		}
		oldStart, newStart := oldPos, newPos
		oldCount, newCount := 0, 0
		for i := h.from; i <= h.to; i++ {
			if ops[i].Kind != '+' {
				oldCount++
			}
			if ops[i].Kind != '-' {
				newCount++
			}
		}
		fmt.Fprintf(&b, "@@ -%d,%d +%d,%d @@\n", oldStart, oldCount, newStart, newCount)
		oldPos, newPos = oldStart, newStart
		for i := h.from; i <= h.to; i++ {
			fmt.Fprintf(&b, "%c%s\n", ops[i].Kind, ops[i].Text)
			switch ops[i].Kind {
			case ' ':
				oldPos++
				newPos++
			case '-':
				oldPos++
			case '+':
				newPos++
			}
		}
		cursor = h.to + 1
	}
	return b.String()
}
