package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/mattn/go-runewidth"
	"unicode/utf8"
)

// View implements tea.Model.
func (m *Model) View() string {
	if !m.ready {
		return lipgloss.NewStyle().Foreground(lipgloss.Color(cClay)).Render("  ◆ 正在启动 basalt…")
	}
	header := headerLine(m.width, m.preset.Model, m.displayProvider(), m.relativeCwd(), m.usageLabel())
	rule := styleRule.Render(strings.Repeat("─", m.width))

	body := m.viewport.View()
	body = padLines(body, m.viewport.Height)

	out := strings.Join([]string{header, rule, body, m.renderComposer(), m.renderFooter()}, "\n")

	if m.approval != nil {
		y := m.approvalY()
		out = placeOverlay(out, m.renderApproval(), m.width, y)
	}
	if m.showHelp {
		out = placeOverlay(out, m.renderHelp(), m.width, max(1, (m.height-lipgloss.Height(m.renderHelp()))/2))
	}
	if m.slash.open && m.approval == nil && m.picker == nil {
		menu := m.renderSlashMenu()
		y := m.height - m.composerHeight() - 2 - lipgloss.Height(menu)
		out = placeOverlay(out, menu, m.width, max(1, y))
	}
	if m.picker != nil {
		popup := m.renderModelPicker()
		y := (m.height - lipgloss.Height(popup)) / 2
		if y < 1 {
			y = 1
		}
		out = placeOverlay(out, popup, m.width, y)
	}
	return truncateLines(out, m.height)
}

// displayProvider is the short provider label used by headers and footers.
func (m *Model) displayProvider() string {
	if m.preset.Name != "" {
		return m.preset.Name
	}
	return m.providerKey
}

func (m *Model) welcomeProviderLine() string {
	return m.displayProvider() + "/" + m.preset.Model
}

func (m *Model) providerSource() string {
	if m.demo {
		return "内置预设（演示）"
	}
	switch {
	case strings.Contains(m.cfg.Source, "basalt"):
		return fmt.Sprintf("Basalt 工作台 · %d 个供应商", len(m.cfg.Providers))
	case strings.Contains(m.cfg.Source, "config"):
		return fmt.Sprintf("本地配置 · %d 个供应商", len(m.cfg.Providers))
	default:
		return "内置预设"
	}
}

func (m *Model) usageLabel() string {
	if m.sess == nil || m.sess.Usage.Requests == 0 {
		return ""
	}
	return fmt.Sprintf("↑%s ↓%s", compactNumber(m.sess.Usage.Input), compactNumber(m.sess.Usage.Output))
}

func compactNumber(n int) string {
	switch {
	case n >= 1_000_000:
		return fmt.Sprintf("%.1fM", float64(n)/1_000_000)
	case n >= 1000:
		return fmt.Sprintf("%.1fk", float64(n)/1000)
	default:
		return fmt.Sprintf("%d", n)
	}
}

func (m *Model) composerHeight() int {
	h := m.input.Height() + 2
	if m.approval != nil {
		h = 3
	}
	return h
}

// renderBody composes the conversation, or the welcome screen when empty.
func (m *Model) renderBody() string {
	if len(m.items) == 0 {
		return m.renderWelcome()
	}
	var parts []string
	prev := ItemNotice
	for i, it := range m.items {
		if i > 0 && !(prev == ItemTool && it.Kind == ItemTool) {
			parts = append(parts, "")
		}
		parts = append(parts, renderItem(it, m.width))
		prev = it.Kind
	}
	return strings.Join(parts, "\n")
}

func (m *Model) renderWelcome() string {
	width := m.width
	pad := 4
	if width < 70 {
		pad = 2
	}
	shades := []string{"#a87a54", "#bb8861", "#cb976b", "#d8a273", "#e6b083", "#f2c69d"}
	brand := "B A S A L T"

	var b strings.Builder
	b.WriteString("\n")
	if width >= 58 && m.viewport.Height >= 14 {
		b.WriteString(spaces(pad))
		b.WriteString(styleHeaderBrand.Render("◆  "))
		for i, ch := range brand {
			b.WriteString(lipgloss.NewStyle().
				Foreground(lipgloss.Color(shades[i%len(shades)])).Bold(true).
				Render(string(ch)))
		}
		b.WriteString(styleFaintText.Render("    v" + Version))
		b.WriteString("\n")
		b.WriteString(spaces(pad + 3))
		b.WriteString(styleFaintText.Render("terminal coding agent  ·  local-first"))
		b.WriteString("\n")
		b.WriteString(spaces(pad + 3))
		b.WriteString(gradientRule(44, "#b8875f", "#4a382c"))
		b.WriteString("\n\n")
	} else {
		b.WriteString(spaces(pad) + styleHeaderBrand.Render("◆ BASALT") + styleFaintText.Render("  terminal coding agent  ·  v"+Version) + "\n\n")
	}

	b.WriteString(spaces(pad) + styleDimText.Render("读懂代码、修改文件、运行测试 —— 在终端里完成整个开发闭环。") + "\n\n")

	b.WriteString(spaces(pad) + styleBold.Render("快速开始") + "\n")
	rows := [][2]string{
		{"/help", "查看全部命令"},
		{"/model", "切换 provider / 模型"},
		{"Tab", "补全命令"},
		{"Esc", "中断生成"},
		{"Ctrl+J", "输入框内换行"},
	}
	for _, row := range rows {
		b.WriteString(spaces(pad + 2))
		b.WriteString(styleAccent.Render(padRight(row[0], 9)))
		b.WriteString(styleDimText.Render(row[1]))
		b.WriteString("\n")
	}
	b.WriteString("\n")
	b.WriteString(spaces(pad) + styleFaintText.Render("工作区  ") + styleDimText.Render(m.relativeCwd()) + "\n")
	b.WriteString(spaces(pad) + styleFaintText.Render("模型    ") + styleDimText.Render(m.welcomeProviderLine()) + "\n")
	b.WriteString(spaces(pad) + styleFaintText.Render("供应商  ") + styleDimText.Render(m.providerSource()) + "\n")
	b.WriteString(spaces(pad) + styleFaintText.Render("配置    ") + styleDimText.Render(shortPath(m.cfgPath, 64)) + "\n")
	b.WriteString("\n")
	b.WriteString(spaces(pad) + styleFaintText.Render("提示：复杂的任务会先给出计划；所有文件改动都会先展示 diff 并等待你批准。") + "\n")
	return b.String()
}

// gradientRule draws a hairline that fades between two colours.
func gradientRule(width int, from, to string) string {
	fr, fg, fb := parseHex(from)
	tr, tg, tb := parseHex(to)
	var b strings.Builder
	for i := 0; i < width; i++ {
		t := float64(i) / float64(max(1, width-1))
		r := int(float64(fr) + (float64(tr)-float64(fr))*t)
		g := int(float64(fg) + (float64(tg)-float64(fg))*t)
		bl := int(float64(fb) + (float64(tb)-float64(fb))*t)
		b.WriteString(lipgloss.NewStyle().
			Foreground(lipgloss.Color(fmt.Sprintf("#%02x%02x%02x", r, g, bl))).
			Render("─"))
	}
	return b.String()
}

func parseHex(s string) (int, int, int) {
	s = strings.TrimPrefix(s, "#")
	if len(s) != 6 {
		return 255, 255, 255
	}
	var r, g, b int
	fmt.Sscanf(s, "%02x%02x%02x", &r, &g, &b)
	return r, g, b
}

func shortPath(p string, limit int) string {
	if p == "" {
		return "内置默认值（未写入磁盘）"
	}
	return truncateWidth(p, limit)
}

func (m *Model) renderComposer() string {
	width := m.width - 2
	if m.approval != nil {
		text := styleWarn.Render("⚠ 等待确认  ") + styleDimText.Render(m.approval.req.Tool+" · "+m.approval.req.Title)
		text += styleFaintText.Render("        y 允许 · a 总是允许 · n 拒绝")
		return styleInputBorderFocus.BorderForeground(lipgloss.Color(cYellow)).Width(width).Render(text)
	}
	style := styleInputBorder
	if m.input.Focused() {
		style = styleInputBorderFocus
	}
	inner := m.input.View()
	return style.Width(width).Render(inner)
}

func (m *Model) renderFooter() string {
	var left string
	if m.streaming {
		elapsed := time.Since(m.runStarted).Seconds()
		left = styleAccent.Render(m.spinner.View()) + " " +
			styleDimText.Render("正在生成") + styleFaintText.Render(fmt.Sprintf("  %.1fs", elapsed))
		if m.streamChars > 0 {
			left += styleFaintText.Render(fmt.Sprintf("  %s 字", compactNumber(m.streamChars)))
		}
		if m.usage.Output > 0 {
			left += styleFaintText.Render(fmt.Sprintf("  ↓%s", compactNumber(m.usage.Output)))
		}
	} else {
		left = styleOK.Render("●") + " " + styleDimText.Render("就绪")
		left += styleFaintText.Render("  " + m.preset.Model)
		if m.env.SnapshotCount() > 0 {
			left += styleFaintText.Render(fmt.Sprintf("  ·  %d 个可撤销改动", m.env.SnapshotCount()))
		}
	}

	right := styleFaintText.Render("/help 命令") + styleFooterDesc.Render("  ·  ") + styleFaintText.Render("esc 中断") +
		styleFooterDesc.Render("  ·  ") + styleFaintText.Render("ctrl+c 退出")
	if m.notice != "" && time.Since(m.noticeAt) < 4*time.Second {
		right = styleWarn.Render(m.notice)
	}
	gap := m.width - displayWidth(left) - displayWidth(right)
	if gap < 1 {
		gap = 1
	}
	return left + spaces(gap) + right
}

func (m *Model) approvalY() int {
	h := lipgloss.Height(m.renderApproval())
	y := m.viewport.Height + 2 - h
	if y < 2 {
		y = 2
	}
	return y
}

func (m *Model) renderApproval() string {
	req := m.approval.req
	width := min(104, m.width-8)
	if width < 40 {
		width = m.width - 4
	}
	contentW := width - 6

	title := lipgloss.NewStyle().Foreground(lipgloss.Color(cYellow)).Bold(true).Render("需要确认")
	head := title + styleFaintText.Render("  ·  ") +
		lipgloss.NewStyle().Foreground(lipgloss.Color(cBlue)).Bold(true).Render(req.Tool) +
		styleDimText.Render("  "+truncateWidth(req.Title, contentW-24))

	var lines []string
	lines = append(lines, head)
	lines = append(lines, "")
	if req.Preview != "" {
		preview := strings.Split(strings.TrimRight(req.Preview, "\n"), "\n")
		if len(preview) > 10 {
			preview = append(preview[:10], styleFaintText.Render("…"))
		}
		for _, line := range preview {
			switch {
			case strings.HasPrefix(line, "+++"), strings.HasPrefix(line, "---"), strings.HasPrefix(line, "@@"):
				lines = append(lines, styleDiffHdr.Render(truncateWidth(line, contentW)))
			case strings.HasPrefix(line, "+"):
				lines = append(lines, styleDiffAdd.Render(padRight(truncateWidth(line, contentW-1), contentW+2)))
			case strings.HasPrefix(line, "-"):
				lines = append(lines, styleDiffDel.Render(padRight(truncateWidth(line, contentW-1), contentW+2)))
			default:
				lines = append(lines, styleCode.Render(truncateWidth(line, contentW)))
			}
		}
		lines = append(lines, "")
	}
	hint := stylePillAccent.Render(" y ") + styleFaintText.Render(" 允许一次    ") +
		stylePill.Render(" a ") + styleFaintText.Render(" 本会话总是允许    ") +
		stylePill.Render(" n ") + styleFaintText.Render(" 拒绝")
	lines = append(lines, hint)

	box := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(cYellow)).
		Background(lipgloss.Color(cSurf2)).
		Padding(0, 2).
		Width(width).
		Render(strings.Join(lines, "\n"))
	return box
}

func (m *Model) renderHelp() string {
	width := min(92, m.width-8)
	if width < 44 {
		width = m.width - 4
	}
	rows := [][2]string{
		{"/help", "显示这份帮助"},
		{"/model", "↑↓ 选择供应商与模型"},
		{"/new", "开始新会话"},
		{"/sessions", "列出历史会话"},
		{"/resume <id>", "恢复历史会话"},
		{"/clear", "清空当前屏幕"},
		{"/compact", "压缩上下文"},
		{"/undo", "撤销最近一次文件改动"},
		{"/tools", "查看可用工具"},
		{"/cost", "查看 token 用量"},
		{"/init", "生成 BASALT.md 项目说明"},
		{"/export", "导出当前会话为 Markdown"},
		{"/quit", "退出"},
	}
	var b strings.Builder
	b.WriteString(lipgloss.NewStyle().Foreground(lipgloss.Color(cClay)).Bold(true).Render("命令"))
	b.WriteString(styleFaintText.Render("   Enter 发送 · Ctrl+J 换行 · Esc 中断"))
	b.WriteString("\n\n")
	for _, row := range rows {
		b.WriteString(styleAccent.Render(padRight(row[0], 18)))
		b.WriteString(styleDimText.Render(row[1]))
		b.WriteString("\n")
	}
	b.WriteString("\n")
	b.WriteString(styleFaintText.Render("快捷键  "))
	b.WriteString(styleDimText.Render("Ctrl+L 清屏 · Ctrl+P 命令面板 · ↑/↓ 历史消息 · PgUp/PgDn 滚动"))
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(cBorder2)).
		Background(lipgloss.Color(cSurface)).
		Padding(1, 2).
		Width(width).
		Render(strings.TrimRight(b.String(), "\n"))
}

func (m *Model) renderSlashMenu() string {
	if len(m.slash.filter) == 0 {
		return ""
	}
	limit := min(7, len(m.slash.filter))
	start := 0
	if m.slash.index >= limit {
		start = m.slash.index - limit + 1
	}
	lineWidth := 0
	for _, item := range m.slash.filter {
		if w := 20 + displayWidth(item.desc); w > lineWidth {
			lineWidth = w
		}
	}
	var items []string
	for i := start; i < start+limit && i < len(m.slash.filter); i++ {
		item := m.slash.filter[i]
		line := styleAccent.Render(padRight(item.cmd, 20)) + styleDimText.Render(item.desc)
		if i == m.slash.index {
			line = lipgloss.NewStyle().Background(lipgloss.Color(cClayBg)).Width(lineWidth).Render(
				styleAccent.Render(padRight(" "+item.cmd, 20)) + styleBold.Render(item.desc))
		}
		items = append(items, line)
	}
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(cBorder2)).
		Background(lipgloss.Color(cSurface)).
		Padding(0, 1).
		Render(strings.Join(items, "\n"))
}

// ---- layout helpers -------------------------------------------------------

func padLines(s string, height int) string {
	lines := strings.Split(s, "\n")
	if len(lines) == 0 {
		lines = []string{""}
	}
	for len(lines) < height {
		lines = append(lines, "")
	}
	if len(lines) > height {
		lines = lines[:height]
	}
	for i, line := range lines {
		lines[i] = padRight(line, 0)
	}
	return strings.Join(lines, "\n")
}

func truncateLines(s string, height int) string {
	lines := strings.Split(s, "\n")
	if len(lines) > height {
		lines = lines[:height]
	}
	return strings.Join(lines, "\n")
}

// placeOverlay composites popup onto base at a vertical offset, centred.
func placeOverlay(base, popup string, width, y int) string {
	bx := strings.Split(base, "\n")
	px := strings.Split(popup, "\n")
	pw := 0
	for _, l := range px {
		if w := displayWidth(l); w > pw {
			pw = w
		}
	}
	x := (width - pw) / 2
	if x < 0 {
		x = 0
	}
	for i, pl := range px {
		row := y + i
		if row < 0 || row >= len(bx) {
			continue
		}
		// Clear the whole row so the dialog floats on an opaque backdrop
		// instead of leaving fragments of the conversation on both sides.
		bx[row] = spaces(x) + pl
	}
	return strings.Join(bx, "\n")
}

// cutWidth keeps [from, from+width) display columns while preserving ANSI.
func cutWidth(s string, from, width int) string {
	var b strings.Builder
	pos := 0
	for i := 0; i < len(s); {
		if s[i] == 0x1b {
			j := i
			for j < len(s) && s[j] != 'm' && s[j] != 'K' && s[j] != 'H' {
				j++
			}
			if j < len(s) {
				j++
			}
			b.WriteString(s[i:j])
			i = j
			continue
		}
		r, size := utf8.DecodeRuneInString(s[i:])
		w := runewidth.RuneWidth(r)
		if pos >= from && pos+w <= from+width {
			b.WriteString(s[i : i+size])
		}
		pos += w
		if pos >= from+width {
			break
		}
		i += size
	}
	b.WriteString("\x1b[0m")
	return b.String()
}
