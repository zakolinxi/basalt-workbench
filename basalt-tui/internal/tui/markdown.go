package tui

import (
	"regexp"
	"strings"
	"unicode/utf8"

	"github.com/charmbracelet/lipgloss"
	"github.com/mattn/go-runewidth"
)

var (
	reBold     = regexp.MustCompile(`\*\*([^*\n]+)\*\*`)
	reItalic   = regexp.MustCompile(`(^|[\s(])\*([^*\n]+)\*`)
	reInline   = regexp.MustCompile("`([^`\n]+)`")
	reLink     = regexp.MustCompile(`\[([^\]]+)\]\(([^)]+)\)`)
	reHeading  = regexp.MustCompile(`^(#{1,6})\s+(.*)$`)
	reOrdered  = regexp.MustCompile(`^(\s*)(\d+)\.\s+(.*)$`)
	reBullet   = regexp.MustCompile(`^(\s*)[-*+]\s+(.*)$`)
	reQuote    = regexp.MustCompile(`^\s*>\s?(.*)$`)
	reRule     = regexp.MustCompile(`^\s*(-{3,}|\*{3,}|_{3,})\s*$`)
	reFence    = regexp.MustCompile("^\\s*```(.*)$")
	reTaskItem = regexp.MustCompile(`^\[( |x|X)\]\s+(.*)$`)
)

var styleCodeBlock = lipgloss.NewStyle().
	Background(lipgloss.Color(cCodeBg)).
	Foreground(lipgloss.Color("#cfcfd6")).
	Padding(0, 2)

var styleQuote = lipgloss.NewStyle().Foreground(lipgloss.Color(cFgDim)).Italic(true)

// renderMarkdown renders a practical subset of Markdown for the terminal.
func renderMarkdown(text string, width int) string {
	if width < 16 {
		width = 16
	}
	lines := strings.Split(strings.ReplaceAll(text, "\r\n", "\n"), "\n")
	var out []string
	var code []string
	lang := ""
	inCode := false

	flushCode := func() {
		if len(code) == 0 && lang == "" {
			return
		}
		if lang != "" {
			out = append(out, styleHeaderDim.Render("  "+lang))
		}
		var highlighted []string
		for _, line := range code {
			line = truncateWidth(line, width-6)
			highlighted = append(highlighted, highlightLine(line, lang))
		}
		block := strings.Join(highlighted, "\n")
		out = append(out, styleCodeBlock.Width(width).Render(block))
		code = nil
		lang = ""
	}

	for _, raw := range lines {
		line := strings.TrimRight(raw, " \t")
		if m := reFence.FindStringSubmatch(line); m != nil {
			if inCode {
				flushCode()
				inCode = false
				continue
			}
			inCode = true
			lang = strings.ToLower(strings.TrimSpace(m[1]))
			if i := strings.IndexByte(lang, ' '); i > 0 {
				lang = lang[:i]
			}
			continue
		}
		if inCode {
			code = append(code, raw)
			continue
		}

		switch {
		case strings.TrimSpace(line) == "":
			out = append(out, "")
		case reRule.MatchString(line):
			out = append(out, styleRule.Render(strings.Repeat("─", min(width, 56))))
		case reHeading.MatchString(line):
			m := reHeading.FindStringSubmatch(line)
			title := inline(m[2], width-4)
			if len(m[1]) <= 2 {
				out = append(out, styleAccent.Render("▎")+" "+styleBold.Render(stripANSI(title)))
			} else {
				out = append(out, styleAccent.Render("▸")+" "+styleBold.Render(stripANSI(title)))
			}
		case reQuote.MatchString(line):
			m := reQuote.FindStringSubmatch(line)
			body := inline(m[1], width-4)
			for _, wrapped := range wrapText(body, width-4) {
				out = append(out, styleRail.Render("│ ")+styleQuote.Render(wrapped))
			}
		case reBullet.MatchString(line):
			m := reBullet.FindStringSubmatch(line)
			indent := len(m[1])
			body := inline(m[2], width-indent-4)
			task := reTaskItem.FindStringSubmatch(m[2])
			marker := styleAccent.Render("•")
			if task != nil {
				if strings.EqualFold(task[1], "x") {
					marker = styleOK.Render("✓")
				} else {
					marker = styleFaintText.Render("○")
				}
				body = inline(task[2], width-indent-4)
			}
			wrapped := wrapText(body, width-indent-4)
			for i, w := range wrapped {
				if i == 0 {
					out = append(out, strings.Repeat(" ", indent)+marker+" "+w)
				} else {
					out = append(out, strings.Repeat(" ", indent+2)+w)
				}
			}
		case reOrdered.MatchString(line):
			m := reOrdered.FindStringSubmatch(line)
			indent := len(m[1])
			body := inline(m[3], width-indent-5)
			wrapped := wrapText(body, width-indent-5)
			for i, w := range wrapped {
				if i == 0 {
					out = append(out, strings.Repeat(" ", indent)+styleAccent.Render(m[2]+".")+" "+w)
				} else {
					out = append(out, strings.Repeat(" ", indent+len(m[2])+2)+w)
				}
			}
		default:
			body := inline(line, width)
			for _, w := range wrapText(body, width) {
				out = append(out, w)
			}
		}
	}
	if inCode {
		flushCode()
	}
	return strings.Join(out, "\n")
}

// inline applies character-level Markdown styles.
func inline(text string, width int) string {
	if width < 8 {
		width = 8
	}
	text = reLink.ReplaceAllStringFunc(text, func(m string) string {
		parts := reLink.FindStringSubmatch(m)
		if len(parts) != 3 {
			return m
		}
		return styleLink.Render(parts[1]) + styleFaintText.Render(" ("+parts[2]+")")
	})
	text = reInline.ReplaceAllStringFunc(text, func(m string) string {
		parts := reInline.FindStringSubmatch(m)
		return styleInline.Render(" " + parts[1] + " ")
	})
	text = reBold.ReplaceAllStringFunc(text, func(m string) string {
		parts := reBold.FindStringSubmatch(m)
		return styleBold.Render(parts[1])
	})
	text = reItalic.ReplaceAllStringFunc(text, func(m string) string {
		parts := reItalic.FindStringSubmatch(m)
		return parts[1] + styleItalic.Render(parts[2])
	})
	return text
}

// wrapText wraps ANSI text by display width. ANSI escape sequences are treated
// as zero width so styled content still wraps correctly.
func wrapText(s string, width int) []string {
	if width < 4 {
		width = 4
	}
	if strings.TrimSpace(s) == "" {
		return []string{""}
	}
	var lines []string
	var line strings.Builder
	lineWidth := 0
	lastBreak := -1 // byte offset in line where a space break is possible

	flush := func(at int) {
		text := line.String()
		if at >= 0 && at < len(text) {
			head := text[:at]
			tail := strings.TrimLeft(text[at:], " ")
			lines = append(lines, head)
			line.Reset()
			line.WriteString(tail)
			lineWidth = displayWidth(tail)
		} else {
			lines = append(lines, strings.TrimRight(text, " "))
			line.Reset()
			lineWidth = 0
		}
		lastBreak = -1
	}

	for i := 0; i < len(s); {
		if s[i] == 0x1b {
			j := i
			for j < len(s) && s[j] != 'm' {
				j++
			}
			if j < len(s) {
				j++
			}
			line.WriteString(s[i:j])
			i = j
			continue
		}
		r, size := decodeRune(s[i:])
		w := runewidth.RuneWidth(r)
		if lineWidth+w > width {
			if lastBreak > 0 {
				flush(lastBreak)
			} else {
				lines = append(lines, strings.TrimRight(line.String(), " "))
				line.Reset()
				lineWidth = 0
				lastBreak = -1
			}
		}
		line.WriteString(s[i : i+size])
		lineWidth += w
		if r == ' ' || r == '，' || r == '。' || r == '、' || r == '；' || r == '：' || r == '）' || r == '】' {
			lastBreak = line.Len()
		}
		i += size
	}
	if line.Len() > 0 {
		lines = append(lines, strings.TrimRight(line.String(), " "))
	}
	if len(lines) == 0 {
		lines = append(lines, "")
	}
	return lines
}

func displayWidth(s string) int {
	return runewidth.StringWidth(stripANSI(s))
}

func truncateWidth(s string, width int) string {
	if displayWidth(s) <= width {
		return s
	}
	var b strings.Builder
	w := 0
	for i := 0; i < len(s); {
		r, size := decodeRune(s[i:])
		rw := runewidth.RuneWidth(r)
		if w+rw > width-1 {
			b.WriteString("…")
			break
		}
		b.WriteString(s[i : i+size])
		w += rw
		i += size
	}
	return b.String()
}

func decodeRune(s string) (rune, int) {
	r, size := utf8.DecodeRuneInString(s)
	return r, size
}

func spaces(n int) string {
	if n <= 0 {
		return ""
	}
	return strings.Repeat(" ", n)
}

func padRight(s string, width int) string {
	gap := width - displayWidth(s)
	if gap <= 0 {
		return s
	}
	return s + spaces(gap)
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
