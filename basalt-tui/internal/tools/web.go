package tools

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"strings"
	"sync"
	"time"

	"golang.org/x/net/html/charset"
)

const (
	defaultWebUserAgent = "basalt-tui/0.2 (+https://github.com/zakolinxi/basalt-workbench)"
	maxFetchBytes       = 4 << 20
	maxBatchURLs        = 5
	defaultMaxChars     = 40000
	hardMaxChars        = 120000
)

// WebOptions configures search and fetch behaviour.
type WebOptions struct {
	Provider   string
	TavilyKey  string
	BraveKey   string
	SearXNGURL string
	TavilyBase string
	BraveBase  string
	DDGBase    string
	UserAgent  string
	Timeout    time.Duration
	CacheTTL   time.Duration
	// AllowPrivate is only set by tests; production fetches reject private IPs.
	AllowPrivate bool
}

func (o WebOptions) withDefaults() WebOptions {
	if o.UserAgent == "" {
		o.UserAgent = defaultWebUserAgent
	}
	if o.Timeout <= 0 {
		o.Timeout = 25 * time.Second
	}
	if o.CacheTTL <= 0 {
		o.CacheTTL = 10 * time.Minute
	}
	return o
}

// webOptionsFromEnv picks up the usual search credentials.
func webOptionsFromEnv() WebOptions {
	return WebOptions{
		Provider:   strings.TrimSpace(os.Getenv("BASALT_WEB_PROVIDER")),
		TavilyKey:  strings.TrimSpace(os.Getenv("TAVILY_API_KEY")),
		BraveKey:   firstEnv("BRAVE_API_KEY", "BRAVE_SEARCH_API_KEY"),
		SearXNGURL: strings.TrimSpace(os.Getenv("SEARXNG_URL")),
		UserAgent:  strings.TrimSpace(os.Getenv("BASALT_WEB_USER_AGENT")),
	}
}

func firstEnv(names ...string) string {
	for _, n := range names {
		if v := strings.TrimSpace(os.Getenv(n)); v != "" {
			return v
		}
	}
	return ""
}

// ---- cache ---------------------------------------------------------------

type webCacheEntry struct {
	title string
	body  string
	at    time.Time
}

type webCache struct {
	mu  sync.Mutex
	m   map[string]webCacheEntry
	ttl time.Duration
}

func newWebCache(ttl time.Duration) *webCache {
	if ttl <= 0 {
		ttl = 10 * time.Minute
	}
	return &webCache{m: map[string]webCacheEntry{}, ttl: ttl}
}

func (c *webCache) get(key string) (webCacheEntry, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	e, ok := c.m[key]
	if !ok {
		return webCacheEntry{}, false
	}
	if time.Since(e.at) > c.ttl {
		delete(c.m, key)
		return webCacheEntry{}, false
	}
	return e, true
}

func (c *webCache) put(key string, e webCacheEntry) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if len(c.m) >= 64 {
		var oldestKey string
		var oldest time.Time
		for k, v := range c.m {
			if oldestKey == "" || v.at.Before(oldest) {
				oldestKey, oldest = k, v.at
			}
		}
		delete(c.m, oldestKey)
	}
	e.at = time.Now()
	c.m[key] = e
}

// ---- network safety ------------------------------------------------------

var webClient = &http.Client{
	Transport: &http.Transport{
		Proxy:                 http.ProxyFromEnvironment,
		DialContext:           safeDialContext,
		MaxIdleConns:          32,
		MaxIdleConnsPerHost:   4,
		IdleConnTimeout:       60 * time.Second,
		TLSHandshakeTimeout:   15 * time.Second,
		ResponseHeaderTimeout: 30 * time.Second,
	},
	CheckRedirect: func(req *http.Request, via []*http.Request) error {
		if len(via) >= 8 {
			return fmt.Errorf("重定向次数过多")
		}
		return checkPublicURL(req.Context(), req.URL)
	},
}

// clientFor returns the guarded client, or an unguarded one for tests.
func clientFor(web WebOptions) *http.Client {
	if web.AllowPrivate {
		return &http.Client{Transport: &http.Transport{Proxy: http.ProxyFromEnvironment}, Timeout: 30 * time.Second}
	}
	return webClient
}

func safeDialContext(ctx context.Context, network, addr string) (net.Conn, error) {
	host, port, err := net.SplitHostPort(addr)
	if err != nil {
		return nil, err
	}
	ips, err := net.DefaultResolver.LookupIPAddr(ctx, host)
	if err != nil {
		return nil, err
	}
	dialer := &net.Dialer{Timeout: 12 * time.Second}
	var lastErr error
	for _, ip := range ips {
		if blockedIP(ip.IP) {
			lastErr = fmt.Errorf("目标地址不允许访问")
			continue
		}
		conn, err := dialer.DialContext(ctx, network, net.JoinHostPort(ip.IP.String(), port))
		if err == nil {
			return conn, nil
		}
		lastErr = err
	}
	if lastErr == nil {
		lastErr = fmt.Errorf("无法解析目标地址")
	}
	return nil, lastErr
}

// blockedIP rejects loopback, private, link-local and other reserved ranges.
func blockedIP(ip net.IP) bool {
	if ip == nil {
		return true
	}
	if ip.IsLoopback() || ip.IsPrivate() || ip.IsUnspecified() || ip.IsMulticast() ||
		ip.IsLinkLocalUnicast() || ip.IsLinkLocalMulticast() || ip.IsInterfaceLocalMulticast() {
		return true
	}
	if v4 := ip.To4(); v4 != nil {
		switch {
		case v4[0] == 100 && v4[1] >= 64 && v4[1] <= 127: // 100.64/10 CGNAT
			return true
		case v4[0] == 192 && v4[1] == 0 && v4[2] == 0: // 192.0.0.0/24
			return true
		case v4[0] == 198 && (v4[1] == 18 || v4[1] == 19): // 198.18/15
			return true
		case v4[0] >= 240: // 240/4 reserved
			return true
		}
	}
	return false
}

func checkPublicURL(ctx context.Context, u *url.URL) error {
	if u == nil {
		return fmt.Errorf("无效的 URL")
	}
	if u.Scheme != "http" && u.Scheme != "https" {
		return fmt.Errorf("只支持 http(s) URL")
	}
	if u.Host == "" {
		return fmt.Errorf("URL 缺少主机名")
	}
	if u.User != nil {
		return fmt.Errorf("不允许带凭据的 URL")
	}
	host := u.Hostname()
	if ip := net.ParseIP(host); ip != nil {
		if blockedIP(ip) {
			return fmt.Errorf("目标地址不允许访问（内网或保留地址）")
		}
		return nil
	}
	ips, err := net.DefaultResolver.LookupIPAddr(ctx, host)
	if err != nil {
		return fmt.Errorf("无法解析 %s：%w", host, err)
	}
	if len(ips) == 0 {
		return fmt.Errorf("无法解析 %s", host)
	}
	for _, ip := range ips {
		if !blockedIP(ip.IP) {
			return nil
		}
	}
	return fmt.Errorf("目标地址不允许访问（内网或保留地址）")
}

// ---- fetch ---------------------------------------------------------------

type fetchOutcome struct {
	title  string
	body   string
	cached bool
	err    error
}

// fetchPage downloads and converts one page.
func (e *Env) fetchPage(ctx context.Context, rawURL, format string, maxChars int) (title, body string, cached bool, err error) {
	opts := e.Web.withDefaults()
	u, err := url.Parse(strings.TrimSpace(rawURL))
	if err != nil {
		return "", "", false, fmt.Errorf("URL 无效：%w", err)
	}
	if !opts.AllowPrivate {
		if err := checkPublicURL(ctx, u); err != nil {
			return "", "", false, err
		}
	}
	if format == "" {
		format = "markdown"
	}
	if maxChars <= 0 || maxChars > hardMaxChars {
		maxChars = defaultMaxChars
	}
	key := u.String() + "|" + format
	if entry, ok := e.cache.get(key); ok {
		return entry.title, entry.body, true, nil
	}

	reqCtx, cancel := context.WithTimeout(ctx, opts.Timeout)
	defer cancel()
	req, err := http.NewRequestWithContext(reqCtx, http.MethodGet, u.String(), nil)
	if err != nil {
		return "", "", false, err
	}
	req.Header.Set("User-Agent", opts.UserAgent)
	req.Header.Set("Accept", "text/html,application/xhtml+xml,application/json;q=0.9,text/plain;q=0.8,*/*;q=0.5")
	req.Header.Set("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
	resp, err := clientFor(opts).Do(req)
	if err != nil {
		return "", "", false, err
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode > 299 {
		return "", "", false, fmt.Errorf("HTTP %d %s", resp.StatusCode, resp.Status)
	}

	raw, err := io.ReadAll(io.LimitReader(resp.Body, maxFetchBytes+1))
	if err != nil {
		return "", "", false, err
	}
	if len(raw) > maxFetchBytes {
		return "", "", false, fmt.Errorf("页面超过 %d MB 限制", maxFetchBytes>>20)
	}
	contentType := resp.Header.Get("Content-Type")
	mediaType := strings.ToLower(strings.TrimSpace(strings.Split(contentType, ";")[0]))

	// Convert to UTF-8 early so both readability and text mode see sane input.
	utf8Raw := raw
	if reader, err := charset.NewReader(bytes.NewReader(raw), contentType); err == nil {
		if converted, err := io.ReadAll(reader); err == nil {
			utf8Raw = converted
		}
	}

	switch {
	case format == "html":
		body = string(utf8Raw)
	case mediaType == "text/html" || mediaType == "application/xhtml+xml" || mediaType == "":
		t, md := extractArticle(utf8Raw, u)
		title = t
		if format == "text" {
			body = plainText(utf8Raw)
		} else {
			body = md
		}
	case strings.HasPrefix(mediaType, "text/") ||
		mediaType == "application/json" || strings.Contains(mediaType, "xml") ||
		mediaType == "application/javascript":
		body = string(utf8Raw)
	default:
		return "", "", false, fmt.Errorf("不支持的内容类型：%s", mediaType)
	}

	body = strings.TrimSpace(body)
	if body == "" {
		return "", "", false, fmt.Errorf("页面没有可提取的正文")
	}
	body, _ = truncate(body, maxChars)
	e.cache.put(key, webCacheEntry{title: title, body: body})
	return title, body, false, nil
}

func toolWebFetch() *Tool {
	return &Tool{
		Name:        "webfetch",
		ReadOnly:    false,
		Description: "抓取网页并提取正文，自动转成 Markdown（标题、列表、代码、链接）。支持一次抓取多个 URL，结果会缓存 10 分钟。内网地址会被拒绝。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"url":{"type":"string","description":"单个 http(s) URL"},
				"urls":{"type":"array","items":{"type":"string"},"description":"批量 URL，最多 5 个，并发抓取"},
				"format":{"type":"string","enum":["markdown","text","html"],"description":"默认 markdown；text 只要纯文本，html 返回原始 HTML"},
				"maxChars":{"type":"integer","description":"每个页面的最大字符数，默认 40000，最大 120000"}
			}
		}`),
		Run: runWebFetch,
	}
}

func runWebFetch(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	var targets []string
	if one := strings.TrimSpace(argString(args, "url")); one != "" {
		targets = append(targets, one)
	}
	// Decode the urls array defensively (models occasionally send a string).
	var raw struct {
		URLs json.RawMessage `json:"urls"`
	}
	_ = json.Unmarshal(args, &raw)
	if len(raw.URLs) > 0 {
		var arr []string
		if json.Unmarshal(raw.URLs, &arr) == nil {
			targets = append(targets, arr...)
		} else {
			var single string
			if json.Unmarshal(raw.URLs, &single) == nil && single != "" {
				targets = append(targets, single)
			}
		}
	}
	if len(targets) == 0 {
		return Result{}, fmt.Errorf("需要 url 或 urls 参数")
	}
	if len(targets) > maxBatchURLs {
		targets = targets[:maxBatchURLs]
	}
	format := strings.ToLower(strings.TrimSpace(argString(args, "format")))
	switch format {
	case "", "markdown", "text", "html":
	default:
		format = "markdown"
	}
	maxChars := argInt(args, defaultMaxChars, "maxChars", "max_chars", "maxCharsPerPage")

	outcomes := make([]fetchOutcome, len(targets))
	sem := make(chan struct{}, 3)
	var wg sync.WaitGroup
	for i, target := range targets {
		wg.Add(1)
		go func(i int, target string) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()
			title, body, cached, err := env.fetchPage(ctx, target, format, maxChars)
			outcomes[i] = fetchOutcome{title: title, body: body, cached: cached, err: err}
		}(i, target)
	}
	wg.Wait()

	var b strings.Builder
	okCount, cachedCount := 0, 0
	for i, target := range targets {
		out := outcomes[i]
		if i > 0 {
			b.WriteString("\n\n---\n\n")
		}
		if out.err != nil {
			fmt.Fprintf(&b, "抓取失败：%s\n%s", target, out.err)
			continue
		}
		okCount++
		if out.cached {
			cachedCount++
		}
		if out.title != "" {
			fmt.Fprintf(&b, "# %s\n\n", out.title)
		}
		fmt.Fprintf(&b, "URL: %s\n\n%s", target, out.body)
	}
	if okCount == 0 {
		return Result{Title: "webfetch 全部失败", Output: b.String()}, nil
	}
	title := fmt.Sprintf("webfetch %d 个页面", okCount)
	if cachedCount > 0 {
		title += fmt.Sprintf("（%d 个来自缓存）", cachedCount)
	}
	body := b.String()
	display := body
	if len(display) > 6000 {
		display = display[:6000] + "\n…"
	}
	return Result{
		Title:   title,
		Output:  body,
		Display: display,
		Meta:    map[string]any{"pages": okCount, "cached": cachedCount},
	}, nil
}
