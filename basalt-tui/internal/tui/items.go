package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/juang/basalt-tui/internal/agent"
)

// ItemKind classifies a conversation entry.
type ItemKind int

const (
	ItemUser ItemKind = iota
	ItemAssistant
	ItemThinking
	ItemTool
	ItemNotice
	ItemError
)

// Item is one rendered conversation entry.
type Item struct {
	Kind      ItemKind
	Text      string
	Tool      *agent.ToolEvent
	At        time.Time
	Streaming bool
	Model     string
}

const gutter = 2

// renderItem draws one entry at the given total width.
func renderItem(it Item, width int) string {
	if width < 20 {
		width = 20
	}
	switch it.Kind {
	case ItemUser:
		return renderUser(it, width)
	case ItemAssistant:
		return renderAssistant(it, width)
	case ItemThinking:
		return renderThinking(it, width)
	case ItemTool:
		return renderTool(it, width)
	case ItemError:
		return renderError(it, width)
	default:
		return renderNotice(it, width)
	}
}

func renderUser(it Item, width int) string {
	inner := width - gutter - 2
	block := lipgloss.NewStyle().
		Background(lipgloss.Color(cUserBg)).
		Foreground(lipgloss.Color(cFg)).
		Padding(0, 1).
		Width(inner)
	body := wrapText(it.Text, inner-2)
	bar := styleUserBar.Render("▌")
	var b strings.Builder
	for i, line := range body {
		if i > 0 {
			b.WriteString("\n")
		}
		b.WriteString(spaces(gutter))
		b.WriteString(bar)
		b.WriteString(block.Render(line))
	}
	return b.String()
}

func renderAssistant(it Item, width int) string {
	inner := width - gutter - 2
	var b strings.Builder
	b.WriteString(spaces(gutter))
	b.WriteString(styleAccent.Render("◆ "))
	b.WriteString(styleHeaderBrand.Render("basalt"))
	if it.Model != "" {
		b.WriteString(styleHeaderDim.Render("  " + it.Model))
	}
	if !it.At.IsZero() {
		b.WriteString(styleHeaderDim.Render("  " + it.At.Format("15:04")))
	}
	if it.Streaming {
		b.WriteString(styleAccent.Render("  ✻"))
	}
	b.WriteString("\n")

	text := it.Text
	if strings.TrimSpace(text) == "" {
		if it.Streaming {
			text = "…"
		}
	}
	rendered := renderMarkdown(text, inner)
	for _, line := range strings.Split(rendered, "\n") {
		b.WriteString(spaces(gutter))
		b.WriteString(line)
		b.WriteString("\n")
	}
	return strings.TrimRight(b.String(), "\n")
}

func renderThinking(it Item, width int) string {
	inner := width - gutter - 4
	body := wrapText(it.Text, inner)
	maxLines := 4
	var lines []string
	for i, line := range body {
		if i >= maxLines {
			lines = append(lines, styleFaintText.Render(fmt.Sprintf("… 还有 %d 行思考", len(body)-i)))
			break
		}
		lines = append(lines, line)
	}
	var b strings.Builder
	b.WriteString(spaces(gutter))
	b.WriteString(styleFaintText.Render("✻ 思考"))
	if it.Streaming {
		b.WriteString(styleAccent.Render(" …"))
	}
	for _, line := range lines {
		b.WriteString("\n")
		b.WriteString(spaces(gutter + 2))
		b.WriteString(styleThink.Render(line))
	}
	return b.String()
}

func renderTool(it Item, width int) string {
	t := it.Tool
	if t == nil {
		return ""
	}
	inner := width - gutter
	contentW := inner - 4
	var borderColor, icon string
	switch {
	case t.Err != "":
		borderColor, icon = cRed, "✗"
	case !t.ReadOnly:
		borderColor, icon = cClay, "⚙"
	default:
		borderColor, icon = cBorder2, "◇"
	}
	rail := lipgloss.NewStyle().Foreground(lipgloss.Color(borderColor))
	if t.ReadOnly && t.Err == "" {
		rail = lipgloss.NewStyle().Foreground(lipgloss.Color(cFaint))
	}

	name := lipgloss.NewStyle().Foreground(lipgloss.Color(cBlue)).Bold(true).Render(t.Name)
	if t.Err != "" {
		name = lipgloss.NewStyle().Foreground(lipgloss.Color(cRed)).Bold(true).Render(t.Name)
	}
	title := styleToolTitle.Render(truncateWidth(t.Title, contentW-16))
	metaParts := []string{}
	if t.Elapsed > 0 {
		metaParts = append(metaParts, t.Elapsed.Round(time.Millisecond).String())
	}
	if t.Added > 0 || t.Removed > 0 {
		metaParts = append(metaParts, fmt.Sprintf("+%d -%d", t.Added, t.Removed))
	}
	if t.Err != "" {
		metaParts = append(metaParts, "failed")
	}
	meta := styleToolMeta.Render(strings.Join(metaParts, " · "))

	headLeft := styleToolMeta.Render(icon) + " " + name
	if title != "" {
		headLeft += styleToolMeta.Render("  ") + title
	}
	gap := contentW - displayWidth(headLeft) - displayWidth(meta)
	if gap < 1 {
		gap = 1
	}

	var b strings.Builder
	b.WriteString(spaces(gutter))
	b.WriteString(rail.Render("╭ "))
	b.WriteString(headLeft)
	b.WriteString(spaces(gap))
	b.WriteString(meta)
	b.WriteString("\n")

	lines := toolBodyLines(t, contentW)
	for _, line := range lines {
		b.WriteString(spaces(gutter))
		b.WriteString(rail.Render("│ "))
		b.WriteString(line)
		b.WriteString("\n")
	}

	b.WriteString(spaces(gutter))
	b.WriteString(rail.Render("╰ "))
	if len(lines) == 0 && t.Err == "" {
		b.WriteString(styleFaintText.Render("完成"))
	}
	if t.Err != "" {
		b.WriteString(styleErr.Render(truncateWidth(t.Err, contentW)))
	}
	b.WriteString(styleFaintText.Render(toolFooter(t)))
	return strings.TrimRight(b.String(), "\n")
}

func toolFooter(t *agent.ToolEvent) string {
	if t.Added > 0 || t.Removed > 0 {
		return ""
	}
	if t.Output != "" {
		return ""
	}
	return ""
}

// toolBodyLines returns the preview body of a tool card.
func toolBodyLines(t *agent.ToolEvent, width int) []string {
	var raw []string
	if t.Diff != "" {
		raw = strings.Split(strings.TrimRight(t.Diff, "\n"), "\n")
	} else if t.Output != "" {
		raw = strings.Split(strings.TrimRight(t.Output, "\n"), "\n")
	} else if t.Preview != "" && t.Elapsed == 0 {
		raw = strings.Split(strings.TrimRight(t.Preview, "\n"), "\n")
	}
	if len(raw) == 0 {
		return nil
	}
	const maxBody = 14
	truncated := false
	if len(raw) > maxBody {
		raw = raw[:maxBody]
		truncated = true
	}
	out := make([]string, 0, len(raw)+1)
	for _, line := range raw {
		if t.Diff != "" {
			out = append(out, renderDiffLine(line, width))
			continue
		}
		out = append(out, styleCode.Render(truncateWidth(line, width)))
	}
	if truncated {
		out = append(out, styleFaintText.Render(fmt.Sprintf("… 还有更多输出")))
	}
	return out
}

func renderDiffLine(line string, width int) string {
	switch {
	case strings.HasPrefix(line, "@@"):
		return styleDiffHdr.Render(truncateWidth(line, width))
	case strings.HasPrefix(line, "---"), strings.HasPrefix(line, "+++"):
		return styleDiffHdr.Render(truncateWidth(line, width))
	case strings.HasPrefix(line, "+"):
		return styleDiffAdd.Width(width).Render(truncateWidth(line, width-1))
	case strings.HasPrefix(line, "-"):
		return styleDiffDel.Width(width).Render(truncateWidth(line, width-1))
	default:
		return styleCode.Render(truncateWidth(line, width))
	}
}

func renderNotice(it Item, width int) string {
	body := wrapText(it.Text, width-gutter-2)
	var b strings.Builder
	for i, line := range body {
		if i > 0 {
			b.WriteString("\n")
		}
		b.WriteString(spaces(gutter))
		b.WriteString(styleFaintText.Render("· "))
		b.WriteString(styleDimText.Render(line))
	}
	return b.String()
}

func renderError(it Item, width int) string {
	inner := width - gutter - 4
	border := lipgloss.NewStyle().Foreground(lipgloss.Color(cRed))
	body := wrapText(it.Text, inner-2)
	var b strings.Builder
	b.WriteString(spaces(gutter) + border.Render("╭ ") + lipgloss.NewStyle().Foreground(lipgloss.Color(cRed)).Bold(true).Render("错误") + "\n")
	for _, line := range body {
		b.WriteString(spaces(gutter) + border.Render("│ ") + styleErr.Render(line) + "\n")
	}
	b.WriteString(spaces(gutter) + border.Render("╰"))
	return b.String()
}
