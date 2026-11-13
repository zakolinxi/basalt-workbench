package tui

import (
	"regexp"
	"strings"
	"unicode/utf8"

	"github.com/charmbracelet/lipgloss"
)

var (
	hlKeyword = lipgloss.NewStyle().Foreground(lipgloss.Color(cPurple))
	hlString  = lipgloss.NewStyle().Foreground(lipgloss.Color("#a8d48a"))
	hlComment = lipgloss.NewStyle().Foreground(lipgloss.Color("#5d5d68")).Italic(true)
	hlNumber  = lipgloss.NewStyle().Foreground(lipgloss.Color(cYellow))
	hlFunc    = lipgloss.NewStyle().Foreground(lipgloss.Color(cBlue))
	hlType    = lipgloss.NewStyle().Foreground(lipgloss.Color("#7fd8c8"))
	hlPlain   = lipgloss.NewStyle().Foreground(lipgloss.Color("#cfcfd6"))
)

var keywordSet = map[string]bool{}
var typeSet = map[string]bool{}

func init() {
	for _, w := range strings.Fields(`func fn def class struct enum impl trait interface type const let var mut pub use mod crate
	return if else match for while loop in of break continue where async await yield import export from package
	new delete this self super static public private protected final abstract extends implements throws
	switch case default do try catch finally throw raise with as lambda pass global nonlocal assert
	true false nil null none True False None unsafe extern volatile sizeof namespace template typename
	go defer chan select range map make append len cap copy close panic recover
	val fun object companion data sealed override open internal inline suspend lateinit
	echo cd export source then fi done esac function local readonly declare`) {
		keywordSet[w] = true
	}
	for _, w := range strings.Fields(`string int int8 int16 int32 int64 uint uint8 uint16 uint32 uint64 float32 float64
	bool byte rune error any vec Vec String Option Result Box HashMap HashSet Arc Rc Mutex RwLock
	array slice list dict tuple set object Promise Array Map Set Record Partial Omit Pick
	i8 i16 i32 i64 u8 u16 u32 u64 usize isize f32 f64 char str void never unknown number boolean
	usize String[] usize`) {
		typeSet[w] = true
	}
}

func isIdentStart(b byte) bool {
	return b == '_' || b >= 'a' && b <= 'z' || b >= 'A' && b <= 'Z'
}

func isIdent(b byte) bool { return isIdentStart(b) || b >= '0' && b <= '9' }

func isDigit(b byte) bool { return b >= '0' && b <= '9' }

// highlightLine colorises one line of source. It is intentionally small: the
// goal is scannability inside a terminal, not a full parser.
func highlightLine(line, lang string) string {
	hashComment := lang == "py" || lang == "python" || lang == "sh" || lang == "bash" ||
		lang == "yaml" || lang == "yml" || lang == "toml" || lang == "rb"
	var b strings.Builder
	i := 0
	for i < len(line) {
		c := line[i]
		// Multi-byte runes (CJK comments, box drawing) pass through intact.
		if c >= 0x80 {
			r, size := utf8.DecodeRuneInString(line[i:])
			b.WriteString(hlPlain.Render(string(r)))
			i += size
			continue
		}
		// Line comments.
		if c == '/' && i+1 < len(line) && line[i+1] == '/' {
			b.WriteString(hlComment.Render(line[i:]))
			break
		}
		if hashComment && c == '#' {
			b.WriteString(hlComment.Render(line[i:]))
			break
		}
		// Strings (single line only).
		if c == '"' || c == '\'' || c == '`' {
			quote := c
			j := i + 1
			for j < len(line) {
				if line[j] == '\\' && quote != '`' {
					j += 2
					continue
				}
				if line[j] == quote {
					j++
					break
				}
				j++
			}
			if j > len(line) {
				j = len(line)
			}
			b.WriteString(hlString.Render(line[i:j]))
			i = j
			continue
		}
		// Numbers.
		if isDigit(c) && (i == 0 || !isIdent(line[i-1])) {
			j := i
			for j < len(line) && (isIdent(line[j]) || line[j] == '.' || line[j] == 'x') {
				j++
			}
			b.WriteString(hlNumber.Render(line[i:j]))
			i = j
			continue
		}
		// Identifiers.
		if isIdentStart(c) {
			j := i
			for j < len(line) && isIdent(line[j]) {
				j++
			}
			word := line[i:j]
			switch {
			case keywordSet[word]:
				b.WriteString(hlKeyword.Render(word))
			case typeSet[word]:
				b.WriteString(hlType.Render(word))
			default:
				// A word immediately followed by '(' reads as a call.
				k := j
				for k < len(line) && line[k] == ' ' {
					k++
				}
				if k < len(line) && line[k] == '(' {
					b.WriteString(hlFunc.Render(word))
				} else {
					b.WriteString(hlPlain.Render(word))
				}
			}
			i = j
			continue
		}
		b.WriteString(hlPlain.Render(string(c)))
		i++
	}
	return b.String()
}

var ansiRe = regexp.MustCompile(`\x1b\[[0-9;]*m`)

func stripANSI(s string) string { return ansiRe.ReplaceAllString(s, "") }
