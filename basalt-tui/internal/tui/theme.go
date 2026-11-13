package tui

import (
	"github.com/charmbracelet/lipgloss"
)

// Palette follows the Basalt desktop workbench: near-black surfaces, warm clay
// accent, and semantic green/amber/red.
const (
	cBg      = "#0f0f11"
	cSurface = "#17171b"
	cSurf2   = "#1e1e24"
	cSurf3   = "#26262d"
	cBorder  = "#2c2c34"
	cBorder2 = "#3d3d47"

	cFg       = "#ececef"
	cFgDim    = "#a6a6b0"
	cFaint    = "#72727d"
	cGhost    = "#55555f"
	cClay     = "#d8a273"
	cClayHi   = "#f0c39a"
	cClayBg   = "#2e241c"
	cGreen    = "#5fd896"
	cGreenBg  = "#15261d"
	cYellow   = "#e9bd4f"
	cYellowBg = "#2a2415"
	cRed      = "#f27c7c"
	cRedBg    = "#2b1919"
	cBlue     = "#8db0f7"
	cBlueBg   = "#18202f"
	cPurple   = "#c9a2f2"

	cUserBg = "#1c2129"
	cCodeBg = "#141417"
)

var (
	styleHeaderBrand = lipgloss.NewStyle().Foreground(lipgloss.Color(cClay)).Bold(true)
	styleHeaderDim   = lipgloss.NewStyle().Foreground(lipgloss.Color(cFaint))
	styleHeaderVal   = lipgloss.NewStyle().Foreground(lipgloss.Color(cFgDim))
	styleRule        = lipgloss.NewStyle().Foreground(lipgloss.Color(cBorder))

	styleUserBar   = lipgloss.NewStyle().Foreground(lipgloss.Color(cClay))
	styleUserText  = lipgloss.NewStyle().Foreground(lipgloss.Color(cFg))
	styleAssistant = lipgloss.NewStyle().Foreground(lipgloss.Color(cFg))
	styleDimText   = lipgloss.NewStyle().Foreground(lipgloss.Color(cFgDim))
	styleFaintText = lipgloss.NewStyle().Foreground(lipgloss.Color(cFaint))
	styleThink     = lipgloss.NewStyle().Foreground(lipgloss.Color(cGhost)).Italic(true)

	styleToolName  = lipgloss.NewStyle().Foreground(lipgloss.Color(cBlue)).Bold(true)
	styleToolTitle = lipgloss.NewStyle().Foreground(lipgloss.Color(cFgDim))
	styleToolMeta  = lipgloss.NewStyle().Foreground(lipgloss.Color(cFaint))
	styleRail      = lipgloss.NewStyle().Foreground(lipgloss.Color(cBorder2))
	styleCode      = lipgloss.NewStyle().Foreground(lipgloss.Color(cFgDim))
	styleCodeBg    = lipgloss.NewStyle().Background(lipgloss.Color(cCodeBg))

	styleOK      = lipgloss.NewStyle().Foreground(lipgloss.Color(cGreen))
	styleWarn    = lipgloss.NewStyle().Foreground(lipgloss.Color(cYellow))
	styleErr     = lipgloss.NewStyle().Foreground(lipgloss.Color(cRed))
	styleAccent  = lipgloss.NewStyle().Foreground(lipgloss.Color(cClay))
	styleLink    = lipgloss.NewStyle().Foreground(lipgloss.Color(cBlue)).Underline(true)
	styleBold    = lipgloss.NewStyle().Foreground(lipgloss.Color(cFg)).Bold(true)
	styleItalic  = lipgloss.NewStyle().Italic(true).Foreground(lipgloss.Color(cFgDim))
	styleInline  = lipgloss.NewStyle().Foreground(lipgloss.Color(cClayHi)).Background(lipgloss.Color("#242018"))
	styleDiffAdd = lipgloss.NewStyle().Foreground(lipgloss.Color("#86e3ac")).Background(lipgloss.Color("#16251c"))
	styleDiffDel = lipgloss.NewStyle().Foreground(lipgloss.Color("#f39393")).Background(lipgloss.Color("#291818"))
	styleDiffHdr = lipgloss.NewStyle().Foreground(lipgloss.Color(cFaint))

	styleInputBorder = lipgloss.NewStyle().
				Border(lipgloss.RoundedBorder()).
				BorderForeground(lipgloss.Color(cBorder2))
	styleInputBorderFocus = lipgloss.NewStyle().
				Border(lipgloss.RoundedBorder()).
				BorderForeground(lipgloss.Color(cClay))
	styleInputPrompt = lipgloss.NewStyle().Foreground(lipgloss.Color(cClay)).Bold(true)
	stylePlaceholder = lipgloss.NewStyle().Foreground(lipgloss.Color(cGhost))

	styleFooterKey  = lipgloss.NewStyle().Foreground(lipgloss.Color(cFgDim))
	styleFooterDesc = lipgloss.NewStyle().Foreground(lipgloss.Color(cGhost))
	stylePill       = lipgloss.NewStyle().Foreground(lipgloss.Color(cFgDim)).Background(lipgloss.Color(cSurf3)).Padding(0, 1)
	stylePillAccent = lipgloss.NewStyle().Foreground(lipgloss.Color("#1a1206")).Background(lipgloss.Color(cClay)).Padding(0, 1).Bold(true)
	stylePillOK     = lipgloss.NewStyle().Foreground(lipgloss.Color("#08130c")).Background(lipgloss.Color(cGreen)).Padding(0, 1).Bold(true)
	stylePillWarn   = lipgloss.NewStyle().Foreground(lipgloss.Color("#1a1405")).Background(lipgloss.Color(cYellow)).Padding(0, 1).Bold(true)
)

// headerLine renders the one-line top bar.
func headerLine(width int, model, provider, cwd string, usage string) string {
	left := styleHeaderBrand.Render("◆ ") +
		lipgloss.NewStyle().Foreground(lipgloss.Color(cFg)).Bold(true).Render("basalt") +
		styleHeaderDim.Render("  terminal coding agent")

	build := func(cwdText string, withUsage bool) string {
		right := styleHeaderVal.Render(provider + "/" + model)
		if withUsage && usage != "" {
			right += styleHeaderDim.Render("  ·  ") + styleHeaderVal.Render(usage)
		}
		right += styleHeaderDim.Render("  ·  ") + styleHeaderVal.Render(cwdText)
		return right
	}

	right := build(cwd, true)
	if lipgloss.Width(left)+lipgloss.Width(right) > width {
		right = build(cwd, false)
	}
	if lipgloss.Width(left)+lipgloss.Width(right) > width {
		room := width - lipgloss.Width(left) - lipgloss.Width(provider+"/"+model) - 12
		right = build(truncateWidth(cwd, max(8, room)), false)
	}
	gap := width - lipgloss.Width(left) - lipgloss.Width(right)
	if gap < 1 {
		gap = 1
	}
	return left + spaces(gap) + right
}
