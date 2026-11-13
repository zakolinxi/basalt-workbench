// Package snapshot turns rendered terminal frames into PNG screenshots.
package snapshot

import (
	"fmt"
	"html"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/mattn/go-runewidth"
)

type sgrState struct {
	fg        string
	bg        string
	bold      bool
	dim       bool
	italic    bool
	underline bool
}

func (s sgrState) css() string {
	var parts []string
	if s.fg != "" {
		parts = append(parts, "color:"+s.fg)
	}
	if s.bg != "" {
		parts = append(parts, "background-color:"+s.bg)
	}
	if s.bold {
		parts = append(parts, "font-weight:700")
	}
	if s.italic {
		parts = append(parts, "font-style:italic")
	}
	if s.underline {
		parts = append(parts, "text-decoration:underline")
	}
	if s.dim {
		parts = append(parts, "opacity:.62")
	}
	return strings.Join(parts, ";")
}

// ANSIToHTML converts a lipgloss-rendered frame into HTML lines.
func ANSIToHTML(frame string) string {
	var b strings.Builder
	for _, line := range strings.Split(frame, "\n") {
		cls := ""
		if strings.Count(line, "█") >= 8 {
			cls = ` class="logo"`
		}
		b.WriteString("<div" + cls + ">")
		st := sgrState{}
		i := 0
		for i < len(line) {
			if line[i] == 0x1b {
				j := i + 1
				if j < len(line) && line[j] == '[' {
					j++
					start := j
					for j < len(line) && (line[j] >= '0' && line[j] <= '9' || line[j] == ';') {
						j++
					}
					if j < len(line) {
						params := line[start:j]
						j++
						applySGR(&st, params)
					}
				}
				i = j
				continue
			}
			start := i
			for i < len(line) && line[i] != 0x1b {
				i++
			}
			text := expandWide(line[start:i])
			if css := st.css(); css != "" {
				fmt.Fprintf(&b, `<span style="%s">%s</span>`, css, text)
			} else {
				b.WriteString(text)
			}
		}
		b.WriteString("</div>")
	}
	return b.String()
}

// expandWide wraps double-width runes in a fixed 2ch box so CJK text occupies
// exactly two terminal columns, matching the real TUI layout.
func expandWide(text string) string {
	var b strings.Builder
	for _, r := range text {
		esc := html.EscapeString(string(r))
		if runewidth.RuneWidth(r) == 2 {
			b.WriteString(`<span class="w2">`)
			b.WriteString(esc)
			b.WriteString(`</span>`)
			continue
		}
		b.WriteString(esc)
	}
	return b.String()
}

func applySGR(st *sgrState, params string) {
	if params == "" {
		*st = sgrState{}
		return
	}
	nums := strings.Split(params, ";")
	for i := 0; i < len(nums); i++ {
		n, _ := strconv.Atoi(nums[i])
		switch {
		case n == 0:
			*st = sgrState{}
		case n == 1:
			st.bold = true
		case n == 2:
			st.dim = true
		case n == 3:
			st.italic = true
		case n == 4:
			st.underline = true
		case n == 22:
			st.bold, st.dim = false, false
		case n == 23:
			st.italic = false
		case n == 24:
			st.underline = false
		case n == 39:
			st.fg = ""
		case n == 49:
			st.bg = ""
		case n >= 30 && n <= 37:
			st.fg = ansi16[n-30]
		case n >= 90 && n <= 97:
			st.fg = ansi16[8+n-90]
		case n >= 40 && n <= 47:
			st.bg = ansi16[n-40]
		case n >= 100 && n <= 107:
			st.bg = ansi16[8+n-100]
		case n == 38 || n == 48:
			if i+1 < len(nums) && nums[i+1] == "2" && i+4 < len(nums) {
				r, _ := strconv.Atoi(nums[i+2])
				g, _ := strconv.Atoi(nums[i+3])
				b, _ := strconv.Atoi(nums[i+4])
				c := fmt.Sprintf("#%02x%02x%02x", r, g, b)
				if n == 38 {
					st.fg = c
				} else {
					st.bg = c
				}
				i += 4
			} else if i+1 < len(nums) && nums[i+1] == "5" && i+2 < len(nums) {
				idx, _ := strconv.Atoi(nums[i+2])
				c := ansi256[idx]
				if n == 38 {
					st.fg = c
				} else {
					st.bg = c
				}
				i += 2
			}
		}
	}
}

var ansi16 = []string{
	"#1c1c22", "#f07171", "#5fd896", "#e9bd4f", "#8db0f7", "#c9a2f2", "#7fd8c8", "#e6e6ec",
	"#5b5b66", "#f79a9a", "#86e3ac", "#f2d37e", "#a8c4ff", "#d9bdf8", "#a4e8de", "#ffffff",
}

var ansi256 []string

func init() {
	ansi256 = make([]string, 256)
	copy(ansi256, ansi16)
	base := []string{"#000000", "#800000", "#008000", "#808000", "#000080", "#800080", "#008080", "#c0c0c0",
		"#808080", "#ff0000", "#00ff00", "#ffff00", "#0000ff", "#ff00ff", "#00ffff", "#ffffff"}
	for i := 16; i < 232; i++ {
		n := i - 16
		r := n / 36
		g := (n % 36) / 6
		b := n % 6
		scale := func(v int) int {
			if v == 0 {
				return 0
			}
			return 55 + v*40
		}
		ansi256[i] = fmt.Sprintf("#%02x%02x%02x", scale(r), scale(g), scale(b))
	}
	for i := 232; i < 256; i++ {
		v := 8 + (i-232)*10
		ansi256[i] = fmt.Sprintf("#%02x%02x%02x", v, v, v)
	}
	_ = base
}

// Page wraps a frame in a terminal-window chrome.
func Page(frame, title string) string {
	body := ANSIToHTML(frame)
	return `<!doctype html>
<html><head><meta charset="utf-8">
<style>
  * { box-sizing: border-box; }
  html, body { margin:0; background:#08080a; }
  .stage { padding:34px; display:inline-block; }
  .win { border-radius:14px; overflow:hidden; background:#0f0f11;
         border:1px solid #2b2b32; box-shadow:0 34px 90px rgba(0,0,0,.65), 0 2px 0 rgba(255,255,255,.04) inset; }
  .bar { height:40px; background:linear-gradient(#1b1b20,#17171b); border-bottom:1px solid #26262c;
         display:flex; align-items:center; padding:0 15px; }
  .dot { width:11px; height:11px; border-radius:50%; margin-right:8px; }
  .dot.r { background:#f0716f; } .dot.y { background:#e9bd4f; } .dot.g { background:#54d68a; }
  .title { margin-left:12px; color:#8c8c97; font:12px 'Cascadia Mono',Consolas,'Microsoft YaHei UI',monospace; letter-spacing:.2px; }
  .term { padding:15px 20px 17px; color:#ececef; white-space:pre; overflow:hidden;
          font:13.5px/19.2px 'Cascadia Mono','Cascadia Code',Consolas,'Microsoft YaHei UI',monospace;
          font-variant-ligatures:none; -webkit-font-smoothing:antialiased; text-rendering:geometricPrecision; }
  .term div { white-space:pre; }
  .w2 { display:inline-block; width:2ch; text-align:center; }
  .term span[style*="background-color"] { padding:3px 0; }
</style></head>
<body><div class="stage"><div class="win">
  <div class="bar"><span class="dot r"></span><span class="dot y"></span><span class="dot g"></span><span class="title">` +
		html.EscapeString(title) + `</span></div>
  <div class="term">` + body + `</div>
</div></div></body></html>`
}

// Browsers returns installed Chromium browsers in preference order.
func Browsers() []string {
	candidates := []string{
		`C:\Program Files\Google\Chrome\Application\chrome.exe`,
		`C:\Program Files (x86)\Google\Chrome\Application\chrome.exe`,
		`C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`,
		`C:\Program Files\Microsoft\Edge\Application\msedge.exe`,
	}
	var out []string
	for _, p := range candidates {
		if st, err := os.Stat(p); err == nil && !st.IsDir() {
			out = append(out, p)
		}
	}
	return out
}

// Capture renders HTML to a PNG using headless Chromium.
func Capture(browser, htmlPath, pngPath string, width, height int) error {
	url := "file:///" + strings.ReplaceAll(filepath.ToSlash(htmlPath), " ", "%20")
	scale := 2
	cmd := exec.Command(browser,
		"--headless=new",
		"--disable-gpu",
		"--hide-scrollbars",
		"--no-first-run",
		"--no-default-browser-check",
		"--force-device-scale-factor="+strconv.Itoa(scale),
		fmt.Sprintf("--window-size=%d,%d", width, height),
		"--screenshot="+pngPath,
		url,
	)
	cmd.Stdout = nil
	cmd.Stderr = nil
	return cmd.Run()
}
