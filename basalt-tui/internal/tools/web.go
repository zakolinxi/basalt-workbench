package tools

import (
	"context"
	"html"
	"io"
	"net/http"
	"regexp"
	"strings"
	"time"
)

var fetchClient = &http.Client{
	Timeout: 45 * time.Second,
	Transport: &http.Transport{
		Proxy:               http.ProxyFromEnvironment,
		MaxIdleConnsPerHost: 4,
		IdleConnTimeout:     30 * time.Second,
	},
}

func httpRequestWithContext(ctx context.Context, u string) (*http.Request, error) {
	return http.NewRequestWithContext(ctx, http.MethodGet, u, nil)
}

func readLimited(r io.Reader, limit int64) ([]byte, error) {
	return io.ReadAll(io.LimitReader(r, limit))
}

var (
	reScript1  = regexp.MustCompile(`(?is)<script[^>]*>.*?</script>`)
	reScript2  = regexp.MustCompile(`(?is)<style[^>]*>.*?</style>`)
	reScript3  = regexp.MustCompile(`(?is)<noscript[^>]*>.*?</noscript>`)
	reScript4  = regexp.MustCompile(`(?is)<svg[^>]*>.*?</svg>`)
	reBlockTag = regexp.MustCompile(`(?i)</?(p|div|section|article|header|footer|li|tr|h[1-6]|br|pre|table)[^>]*>`)
	reTag      = regexp.MustCompile(`(?s)<[^>]+>`)
	reSpaces   = regexp.MustCompile(`[ \t\f\v]+`)
	reBlank    = regexp.MustCompile(`\n{3,}`)
)

// stripHTML converts an HTML document into readable plain text.
func stripHTML(s string) string {
	s = reScript1.ReplaceAllString(s, " ")
	s = reScript2.ReplaceAllString(s, " ")
	s = reScript3.ReplaceAllString(s, " ")
	s = reScript4.ReplaceAllString(s, " ")
	s = reBlockTag.ReplaceAllString(s, "\n")
	s = reTag.ReplaceAllString(s, " ")
	s = html.UnescapeString(s)
	s = strings.ReplaceAll(s, "\r\n", "\n")
	s = reSpaces.ReplaceAllString(s, " ")
	lines := strings.Split(s, "\n")
	for i, line := range lines {
		lines[i] = strings.TrimSpace(line)
	}
	return strings.Join(lines, "\n")
}

func collapseBlank(s string) string {
	return reBlank.ReplaceAllString(s, "\n\n")
}
