package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/juang/basalt-tui/internal/config"
)

// pickerItem is one row of the model picker. Provider rows are headers and
// cannot be selected; model rows are the selectable entries.
type pickerItem struct {
	group   string
	name    string
	model   string
	kind    string
	source  string
	hasKey  bool
	header  bool
	current bool
}

type pickerState struct {
	items  []pickerItem
	filter []rune
	index  int
}

func newModelPicker(cfg config.Config, currentKey, currentModel string) *pickerState {
	p := &pickerState{}
	for _, prov := range cfg.List() {
		models := prov.Models
		if len(models) == 0 {
			models = []string{prov.Model}
		}
		p.items = append(p.items, pickerItem{
			header: true, group: prov.ID, name: prov.Name,
			kind: prov.Kind, source: prov.Source, hasKey: prov.APIKey != "",
		})
		for _, model := range models {
			p.items = append(p.items, pickerItem{
				group: prov.ID, name: prov.Name, model: model, kind: prov.Kind,
				hasKey: prov.APIKey != "", current: prov.ID == currentKey && model == currentModel,
			})
		}
	}
	p.resetIndex()
	return p
}

func (p *pickerState) query() string {
	return strings.ToLower(strings.TrimSpace(string(p.filter)))
}

// visible filters provider groups by provider name or model name.
func (p *pickerState) visible() []pickerItem {
	q := p.query()
	if q == "" {
		return p.items
	}
	var out []pickerItem
	for i := 0; i < len(p.items); {
		if !p.items[i].header {
			i++
			continue
		}
		head := p.items[i]
		providerMatch := strings.Contains(strings.ToLower(head.name), q)
		j := i + 1
		var rows []pickerItem
		for j < len(p.items) && !p.items[j].header {
			if providerMatch || strings.Contains(strings.ToLower(p.items[j].model), q) {
				rows = append(rows, p.items[j])
			}
			j++
		}
		if len(rows) > 0 {
			out = append(out, head)
			out = append(out, rows...)
		}
		i = j
	}
	return out
}

func (p *pickerState) resetIndex() {
	p.index = 0
	vis := p.visible()
	for i, item := range vis {
		if item.current {
			p.index = i
			return
		}
	}
	for i, item := range vis {
		if !item.header {
			p.index = i
			return
		}
	}
}

func (p *pickerState) move(delta int) {
	vis := p.visible()
	if len(vis) == 0 {
		return
	}
	if p.index >= len(vis) {
		p.index = 0
	}
	for step := 1; step <= len(vis); step++ {
		idx := (p.index + delta*step) % len(vis)
		if idx < 0 {
			idx += len(vis)
		}
		if !vis[idx].header {
			p.index = idx
			return
		}
	}
}

func (m *Model) openModelPicker() {
	m.picker = newModelPicker(m.cfg, m.providerKey, m.preset.Model)
	m.slash.open = false
	m.refreshViewport(false)
}

// sourceLabel turns internal source markers into short UI labels.
func sourceLabel(source string) string {
	switch {
	case strings.Contains(source, "basalt"):
		return "工作台"
	case source == "config":
		return "配置"
	case source == "builtin":
		return "内置"
	default:
		return source
	}
}

func (m *Model) handlePickerKey(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	p := m.picker
	switch msg.String() {
	case "esc", "ctrl+c":
		m.picker = nil
		m.refreshViewport(false)
		return m, nil
	case "up", "ctrl+p":
		p.move(-1)
	case "down", "ctrl+n":
		p.move(1)
	case "pgup":
		p.move(-5)
	case "pgdown":
		p.move(5)
	case "enter":
		vis := p.visible()
		if len(vis) > 0 && p.index < len(vis) && !vis[p.index].header {
			item := vis[p.index]
			m.picker = nil
			return m, m.switchProvider(item.group, item.model)
		}
	case "backspace":
		if len(p.filter) > 0 {
			p.filter = p.filter[:len(p.filter)-1]
			p.resetIndex()
		}
	case "ctrl+u":
		p.filter = nil
		p.resetIndex()
	default:
		if msg.Type == tea.KeyRunes {
			p.filter = append(p.filter, msg.Runes...)
			p.resetIndex()
		}
	}
	m.refreshViewport(false)
	return m, nil
}

func (m *Model) renderModelPicker() string {
	p := m.picker
	vis := p.visible()
	width := min(78, m.width-8)
	if width < 46 {
		width = m.width - 4
	}
	inner := width - 4

	title := lipgloss.NewStyle().Foreground(lipgloss.Color(cClay)).Bold(true).Render("选择模型")
	current := styleFaintText.Render(m.displayProvider() + "/" + m.preset.Model)

	var b strings.Builder
	b.WriteString(title)
	gap := inner - displayWidth(title) - displayWidth(current)
	if gap < 1 {
		gap = 1
	}
	b.WriteString(spaces(gap) + current)
	b.WriteString("\n")

	if len(p.filter) > 0 {
		b.WriteString(styleAccent.Render("/ ") + styleBold.Render(string(p.filter)) +
			lipgloss.NewStyle().Foreground(lipgloss.Color(cFgDim)).Render("▏"))
	} else {
		b.WriteString(styleFaintText.Render("输入以过滤供应商或模型"))
	}
	b.WriteString("\n\n")

	maxRows := min(14, m.height-12)
	if maxRows < 5 {
		maxRows = 5
	}
	start := 0
	if p.index >= maxRows {
		start = p.index - maxRows + 1
	}
	if start > len(vis)-maxRows {
		start = len(vis) - maxRows
	}
	if start < 0 {
		start = 0
	}
	end := min(start+maxRows, len(vis))
	if len(vis) == 0 {
		b.WriteString(styleFaintText.Render("没有匹配的模型"))
		b.WriteString("\n")
	}
	for i := start; i < end; i++ {
		item := vis[i]
		if item.header {
			if i > start {
				b.WriteString("\n")
			}
			dot := styleFaintText.Render("●")
			nameStyle := styleBold
			if item.current {
				dot = styleAccent.Render("●")
				nameStyle = lipgloss.NewStyle().Foreground(lipgloss.Color(cClay)).Bold(true)
			}
			line := dot + " " + nameStyle.Render(item.name) +
				styleFaintText.Render("  "+item.kind)
			if item.source != "" {
				line += styleFaintText.Render("  " + sourceLabel(item.source))
			}
			if !item.hasKey && item.kind != "ollama" {
				line += styleWarn.Render("  无 key")
			}
			b.WriteString(line + "\n")
			continue
		}
		check := "  "
		if item.current {
			check = styleOK.Render("✓ ")
		}
		if i == p.index {
			plain := "▸ " + stripANSI(check) + item.model
			if item.current {
				plain += "   当前"
			}
			b.WriteString(lipgloss.NewStyle().
				Background(lipgloss.Color(cClayBg)).
				Foreground(lipgloss.Color(cFg)).
				Bold(true).
				Width(inner).
				Render(plain) + "\n")
			continue
		}
		line := "  " + check + styleDimText.Render(item.model)
		if item.current {
			line += styleFaintText.Render("   当前")
		}
		b.WriteString(line + "\n")
	}
	if start > 0 {
		b.WriteString(styleFaintText.Render(fmt.Sprintf("   ↑ 还有 %d 项", start)))
		b.WriteString("\n")
	}
	if end < len(vis) {
		b.WriteString(styleFaintText.Render(fmt.Sprintf("   ↓ 还有 %d 项", len(vis)-end)))
		b.WriteString("\n")
	}

	b.WriteString("\n")
	b.WriteString(stylePillAccent.Render("↑↓") + styleFaintText.Render(" 选择    ") +
		stylePill.Render("Enter") + styleFaintText.Render(" 切换    ") +
		stylePill.Render("Esc") + styleFaintText.Render(" 关闭"))

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(cClay)).
		Background(lipgloss.Color(cSurf2)).
		Padding(1, 2).
		Width(width).
		Render(strings.TrimRight(b.String(), "\n"))
}
