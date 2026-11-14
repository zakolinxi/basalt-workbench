package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

type searchHit struct {
	Title     string
	URL       string
	Snippet   string
	Published string
}

type searchOptions struct {
	Max       int
	Site      string
	Freshness string
}

// searcher is one web search backend.
type searcher interface {
	Name() string
	Search(ctx context.Context, query string, opts searchOptions, web WebOptions) ([]searchHit, error)
}

func (o WebOptions) withSearchDefaults() WebOptions {
	o = o.withDefaults()
	if o.TavilyBase == "" {
		o.TavilyBase = "https://api.tavily.com"
	}
	if o.BraveBase == "" {
		o.BraveBase = "https://api.search.brave.com"
	}
	if o.DDGBase == "" {
		o.DDGBase = "https://html.duckduckgo.com"
	}
	return o
}

// pickSearcher honours an explicit provider, otherwise picks the first
// configured backend and falls back to DuckDuckGo.
func pickSearcher(web WebOptions) (searcher, error) {
	web = web.withSearchDefaults()
	switch strings.ToLower(strings.TrimSpace(web.Provider)) {
	case "tavily":
		if web.TavilyKey == "" {
			return nil, fmt.Errorf("TAVILY_API_KEY 未配置")
		}
		return tavilySearcher{}, nil
	case "brave":
		if web.BraveKey == "" {
			return nil, fmt.Errorf("BRAVE_API_KEY 未配置")
		}
		return braveSearcher{}, nil
	case "searxng", "searx":
		if web.SearXNGURL == "" {
			return nil, fmt.Errorf("SEARXNG_URL 未配置")
		}
		return searxngSearcher{}, nil
	case "duckduckgo", "ddg":
		return ddgSearcher{}, nil
	case "":
		if web.TavilyKey != "" {
			return tavilySearcher{}, nil
		}
		if web.BraveKey != "" {
			return braveSearcher{}, nil
		}
		if web.SearXNGURL != "" {
			return searxngSearcher{}, nil
		}
		return ddgSearcher{}, nil
	default:
		return nil, fmt.Errorf("未知搜索后端 %q", web.Provider)
	}
}

func webJSON(ctx context.Context, req *http.Request, out any, web WebOptions) error {
	reqCtx, cancel := context.WithTimeout(ctx, web.withDefaults().Timeout)
	defer cancel()
	req = req.WithContext(reqCtx)
	req.Header.Set("Accept", "application/json")
	resp, err := clientFor(web).Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode > 299 {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, 600))
		return fmt.Errorf("HTTP %d：%s", resp.StatusCode, strings.TrimSpace(string(body)))
	}
	return json.NewDecoder(io.LimitReader(resp.Body, 8<<20)).Decode(out)
}

func clampResults(n int) int {
	if n <= 0 {
		return 5
	}
	if n > 10 {
		return 10
	}
	return n
}

func clipRunes(s string, limit int) string {
	r := []rune(strings.TrimSpace(s))
	if len(r) <= limit {
		return string(r)
	}
	return string(r[:limit]) + "…"
}

// ---- Tavily --------------------------------------------------------------

type tavilySearcher struct{}

func (tavilySearcher) Name() string { return "tavily" }

func (tavilySearcher) Search(ctx context.Context, query string, opts searchOptions, web WebOptions) ([]searchHit, error) {
	web = web.withSearchDefaults()
	body := map[string]any{
		"query":          withSite(query, opts.Site),
		"max_results":    clampResults(opts.Max),
		"search_depth":   "basic",
		"include_answer": false,
	}
	if opts.Freshness != "" {
		body["time_range"] = opts.Freshness
	}
	raw, _ := json.Marshal(body)
	req, err := http.NewRequest(http.MethodPost, strings.TrimRight(web.TavilyBase, "/")+"/search", strings.NewReader(string(raw)))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	if web.TavilyKey != "" {
		req.Header.Set("Authorization", "Bearer "+web.TavilyKey)
	}
	var resp struct {
		Results []struct {
			Title         string  `json:"title"`
			URL           string  `json:"url"`
			Content       string  `json:"content"`
			Score         float64 `json:"score"`
			PublishedDate string  `json:"published_date"`
		} `json:"results"`
	}
	if err := webJSON(ctx, req, &resp, web); err != nil {
		return nil, err
	}
	hits := make([]searchHit, 0, len(resp.Results))
	for _, r := range resp.Results {
		hits = append(hits, searchHit{Title: r.Title, URL: r.URL, Snippet: r.Content, Published: r.PublishedDate})
	}
	return hits, nil
}

// ---- Brave ---------------------------------------------------------------

type braveSearcher struct{}

func (braveSearcher) Name() string { return "brave" }

func (braveSearcher) Search(ctx context.Context, query string, opts searchOptions, web WebOptions) ([]searchHit, error) {
	web = web.withSearchDefaults()
	q := url.Values{}
	q.Set("q", withSite(query, opts.Site))
	q.Set("count", strconv.Itoa(clampResults(opts.Max)))
	if f := braveFreshness(opts.Freshness); f != "" {
		q.Set("freshness", f)
	}
	req, err := http.NewRequest(http.MethodGet, strings.TrimRight(web.BraveBase, "/")+"/res/v1/web/search?"+q.Encode(), nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("X-Subscription-Token", web.BraveKey)
	var resp struct {
		Web struct {
			Results []struct {
				Title       string `json:"title"`
				URL         string `json:"url"`
				Description string `json:"description"`
				Age         string `json:"age"`
			} `json:"results"`
		} `json:"web"`
	}
	if err := webJSON(ctx, req, &resp, web); err != nil {
		return nil, err
	}
	hits := make([]searchHit, 0, len(resp.Web.Results))
	for _, r := range resp.Web.Results {
		hits = append(hits, searchHit{Title: r.Title, URL: r.URL, Snippet: r.Description, Published: r.Age})
	}
	return hits, nil
}

func braveFreshness(f string) string {
	switch strings.ToLower(f) {
	case "day", "d":
		return "pd"
	case "week", "w":
		return "pw"
	case "month", "m":
		return "pm"
	case "year", "y":
		return "py"
	}
	return ""
}

// ---- SearXNG -------------------------------------------------------------

type searxngSearcher struct{}

func (searxngSearcher) Name() string { return "searxng" }

func (searxngSearcher) Search(ctx context.Context, query string, opts searchOptions, web WebOptions) ([]searchHit, error) {
	web = web.withSearchDefaults()
	q := url.Values{}
	q.Set("q", withSite(query, opts.Site))
	q.Set("format", "json")
	q.Set("safesearch", "1")
	q.Set("language", "zh-CN")
	req, err := http.NewRequest(http.MethodGet, strings.TrimRight(web.SearXNGURL, "/")+"/search?"+q.Encode(), nil)
	if err != nil {
		return nil, err
	}
	var resp struct {
		Results []struct {
			Title         string `json:"title"`
			URL           string `json:"url"`
			Content       string `json:"content"`
			PublishedDate string `json:"publishedDate"`
		} `json:"results"`
	}
	if err := webJSON(ctx, req, &resp, web); err != nil {
		return nil, err
	}
	hits := make([]searchHit, 0, len(resp.Results))
	for i, r := range resp.Results {
		if i >= clampResults(opts.Max) {
			break
		}
		hits = append(hits, searchHit{Title: r.Title, URL: r.URL, Snippet: r.Content, Published: r.PublishedDate})
	}
	return hits, nil
}

// ---- DuckDuckGo ----------------------------------------------------------

type ddgSearcher struct{}

func (ddgSearcher) Name() string { return "duckduckgo" }

func (ddgSearcher) Search(ctx context.Context, query string, opts searchOptions, web WebOptions) ([]searchHit, error) {
	web = web.withSearchDefaults()
	form := url.Values{}
	form.Set("q", withSite(query, opts.Site))
	req, err := http.NewRequest(http.MethodPost,
		strings.TrimRight(web.DDGBase, "/")+"/html/", strings.NewReader(form.Encode()))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("User-Agent", web.UserAgent)
	reqCtx, cancel := context.WithTimeout(ctx, web.Timeout)
	defer cancel()
	resp, err := clientFor(web).Do(req.WithContext(reqCtx))
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode > 299 {
		return nil, fmt.Errorf("HTTP %d", resp.StatusCode)
	}
	raw, err := io.ReadAll(io.LimitReader(resp.Body, 4<<20))
	if err != nil {
		return nil, err
	}
	doc, err := html.Parse(strings.NewReader(string(raw)))
	if err != nil {
		return nil, err
	}
	var hits []searchHit
	limit := clampResults(opts.Max)
	walkSearch(doc, func(n *html.Node) {
		if len(hits) >= limit {
			return
		}
		if n.Type != html.ElementNode || !hasClass(n, "result") {
			return
		}
		// Skip nested result containers.
		for p := n.Parent; p != nil; p = p.Parent {
			if p.Type == html.ElementNode && hasClass(p, "result") {
				return
			}
		}
		var href, title, snippet string
		walkSearch(n, func(c *html.Node) {
			if c.Type != html.ElementNode {
				return
			}
			if c.DataAtom == atom.A && hasClass(c, "result__a") && href == "" {
				href = decodeDDG(attr(c, "href"))
				title = strings.TrimSpace(textOf(c))
			}
			if hasClass(c, "result__snippet") && snippet == "" {
				snippet = strings.Join(strings.Fields(textOf(c)), " ")
			}
		})
		if href != "" && title != "" {
			hits = append(hits, searchHit{Title: title, URL: href, Snippet: snippet})
		}
	})
	if len(hits) == 0 {
		return nil, fmt.Errorf("DuckDuckGo 没有返回结果（可能触发了验证，建议配置 TAVILY_API_KEY）")
	}
	return hits, nil
}

func walkSearch(n *html.Node, fn func(*html.Node)) {
	fn(n)
	for c := n.FirstChild; c != nil; c = c.NextSibling {
		walkSearch(c, fn)
	}
}

func hasClass(n *html.Node, class string) bool {
	for _, a := range n.Attr {
		if a.Key == "class" {
			for _, f := range strings.Fields(a.Val) {
				if f == class {
					return true
				}
			}
		}
	}
	return false
}

func decodeDDG(href string) string {
	href = strings.TrimSpace(href)
	if href == "" {
		return ""
	}
	if strings.HasPrefix(href, "//") {
		href = "https:" + href
	}
	u, err := url.Parse(href)
	if err != nil {
		return href
	}
	if target := u.Query().Get("uddg"); target != "" {
		return target
	}
	return href
}

func findSnippet(doc *html.Node) string {
	var snippet string
	walkSearch(doc, func(n *html.Node) {
		if snippet != "" || n.Type != html.ElementNode {
			return
		}
		if n.DataAtom != atom.Div && n.DataAtom != atom.A && n.DataAtom != atom.Td {
			return
		}
		if !hasClass(n, "result__snippet") {
			return
		}
		snippet = strings.Join(strings.Fields(textOf(n)), " ")
	})
	return snippet
}

func withSite(query, site string) string {
	site = strings.TrimSpace(site)
	if site == "" {
		return query
	}
	return query + " site:" + site
}

// ---- tool ----------------------------------------------------------------

func toolWebSearch() *Tool {
	return &Tool{
		Name:     "websearch",
		ReadOnly: true,
		Description: "搜索公开网页，返回标题、URL 和摘要。默认用 Tavily（也可 Brave / SearXNG / DuckDuckGo）。" +
			"涉及版本、文档、新闻等事实时应先搜索再回答，并引用真实 URL。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"query":{"type":"string","description":"搜索关键词"},
				"maxResults":{"type":"integer","description":"结果数，默认 5，最大 10"},
				"fetchTop":{"type":"integer","description":"自动抓取前 N 条正文，0-3，默认 0"},
				"site":{"type":"string","description":"限定站点，例如 github.com"},
				"freshness":{"type":"string","enum":["day","week","month","year"],"description":"时间范围（Tavily/Brave）"}
			},
			"required":["query"]
		}`),
		Run: runWebSearch,
	}
}

func runWebSearch(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	query := strings.TrimSpace(argString(args, "query", "q"))
	if query == "" {
		return Result{}, fmt.Errorf("缺少 query")
	}
	opts := searchOptions{
		Max:       clampResults(argInt(args, 5, "maxResults", "max_results", "limit")),
		Site:      strings.TrimSpace(argString(args, "site")),
		Freshness: strings.ToLower(strings.TrimSpace(argString(args, "freshness"))),
	}
	engine, err := pickSearcher(env.Web)
	if err != nil {
		return Result{}, fmt.Errorf("%w（可设置 TAVILY_API_KEY、BRAVE_API_KEY 或 SEARXNG_URL）", err)
	}
	start := time.Now()
	hits, err := engine.Search(ctx, query, opts, env.Web)
	if err != nil {
		return Result{}, fmt.Errorf("%s 搜索失败：%w", engine.Name(), err)
	}
	if len(hits) == 0 {
		return Result{Title: "websearch · 0 个结果", Output: "没有找到结果", Meta: map[string]any{"engine": engine.Name()}}, nil
	}

	var b strings.Builder
	fmt.Fprintf(&b, "搜索：%s（%s，%d 个结果）\n", query, engine.Name(), len(hits))
	for i, hit := range hits {
		fmt.Fprintf(&b, "\n%d. %s\n   %s\n", i+1, strings.Join(strings.Fields(hit.Title), " "), hit.URL)
		if snippet := strings.Join(strings.Fields(hit.Snippet), " "); snippet != "" {
			fmt.Fprintf(&b, "   %s\n", clipRunes(snippet, 320))
		}
		if hit.Published != "" {
			fmt.Fprintf(&b, "   %s\n", hit.Published)
		}
	}

	fetchTop := argInt(args, 0, "fetchTop", "fetch_top")
	if fetchTop > 3 {
		fetchTop = 3
	}
	if fetchTop > 0 {
		if fetchTop > len(hits) {
			fetchTop = len(hits)
		}
		for i := 0; i < fetchTop; i++ {
			hit := hits[i]
			title, body, _, err := env.fetchPage(ctx, hit.URL, "markdown", 3000)
			b.WriteString("\n\n---\n")
			if err != nil {
				fmt.Fprintf(&b, "\n### 正文抓取失败：%s\n%s\n", hit.URL, err)
				continue
			}
			if title == "" {
				title = hit.Title
			}
			fmt.Fprintf(&b, "\n### %s\n%s\n\n%s\n", title, hit.URL, body)
		}
	}

	body := strings.TrimSpace(b.String())
	display := body
	if len(display) > 6000 {
		display = display[:6000] + "\n…"
	}
	title := fmt.Sprintf("websearch %q · %d 个结果 · %s", clipRunes(query, 30), len(hits), engine.Name())
	if elapsed := time.Since(start); elapsed > time.Second {
		title += fmt.Sprintf(" · %.1fs", elapsed.Seconds())
	}
	return Result{
		Title:   title,
		Output:  body,
		Display: display,
		Meta:    map[string]any{"engine": engine.Name(), "results": len(hits)},
	}, nil
}
