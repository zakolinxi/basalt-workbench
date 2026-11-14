package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"sync/atomic"
	"testing"

	"golang.org/x/text/encoding/simplifiedchinese"
)

func TestHTMLToMarkdown(t *testing.T) {
	page, _ := url.Parse("https://example.com/docs/")
	raw := []byte(`<html><body>
		<h1>Getting started</h1>
		<p>Install with <code>go install</code> and see <a href="/guide">the guide</a>.</p>
		<ul><li>first</li><li>second</li></ul>
		<pre><code>go test ./...</code></pre>
		<script>var x = 1;</script>
	</body></html>`)
	md := htmlToMarkdown(raw, page)
	for _, want := range []string{"# Getting started", "`go install`", "[the guide](https://example.com/guide)", "- first", "```", "go test"} {
		if !strings.Contains(md, want) {
			t.Fatalf("markdown missing %q:\n%s", want, md)
		}
	}
	if strings.Contains(md, "var x = 1") {
		t.Fatal("script content should be dropped")
	}
}

func TestExtractArticlePrefersMainContent(t *testing.T) {
	page, _ := url.Parse("https://example.com/post")
	raw := []byte(`<html><body>
		<nav>menu menu menu</nav>
		<article>
			<h2>Real title</h2>
			<p>` + strings.Repeat("This is the actual article body. ", 30) + `</p>
		</article>
		<footer>copyright</footer>
	</body></html>`)
	title, md := extractArticle(raw, page)
	if !strings.Contains(md, "actual article body") {
		t.Fatalf("article body missing:\n%s", md)
	}
	if strings.Contains(md, "copyright") {
		t.Fatal("footer should be excluded")
	}
	_ = title
}

func TestSSRFBlocksPrivateAddresses(t *testing.T) {
	cases := []string{
		"http://127.0.0.1/",
		"http://10.0.0.1/admin",
		"http://192.168.1.1/",
		"http://169.254.169.254/latest/meta-data",
		"http://[::1]/",
		"http://100.64.0.1/",
		"file:///etc/passwd",
	}
	for _, raw := range cases {
		u, err := url.Parse(raw)
		if err != nil {
			t.Fatal(err)
		}
		if err := checkPublicURL(context.Background(), u); err == nil {
			t.Fatalf("%s should be blocked", raw)
		}
	}
}

func TestFetchCachesPages(t *testing.T) {
	var hits int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&hits, 1)
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		fmt.Fprint(w, `<html><body><article><h1>Cache test</h1><p>`+
			strings.Repeat("A page worth caching. ", 20)+`</p></article></body></html>`)
	}))
	defer srv.Close()

	env := NewEnv(t.TempDir(), "web", t.TempDir())
	env.Web = WebOptions{AllowPrivate: true}
	title, body, cached, err := env.fetchPage(context.Background(), srv.URL, "markdown", 5000)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(body, "Cache test") {
		t.Fatalf("unexpected fetch result: %q %q", title, body)
	}
	if cached {
		t.Fatal("first fetch should not be cached")
	}
	_, _, cached, err = env.fetchPage(context.Background(), srv.URL, "markdown", 5000)
	if err != nil || !cached {
		t.Fatalf("second fetch should hit the cache: %v cached=%v", err, cached)
	}
	if got := atomic.LoadInt32(&hits); got != 1 {
		t.Fatalf("expected a single request, got %d", got)
	}
}

func TestFetchConvertsGBK(t *testing.T) {
	page := `<html><body><article><h1>中文页面</h1><p>` +
		strings.Repeat("这是一段用于测试编码转换的正文内容。", 12) + `</p></article></body></html>`
	gbk, err := simplifiedchinese.GBK.NewEncoder().Bytes([]byte(page))
	if err != nil {
		t.Fatal(err)
	}
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=gbk")
		_, _ = w.Write(gbk)
	}))
	defer srv.Close()

	env := NewEnv(t.TempDir(), "web", t.TempDir())
	env.Web = WebOptions{AllowPrivate: true}
	_, body, _, err := env.fetchPage(context.Background(), srv.URL, "markdown", 5000)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(body, "中文页面") || !strings.Contains(body, "编码转换") {
		t.Fatalf("GBK page was not decoded:\n%s", body)
	}
}

func TestFetchRejectsBinary(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/pdf")
		_, _ = w.Write([]byte("%PDF-1.7"))
	}))
	defer srv.Close()
	env := NewEnv(t.TempDir(), "web", t.TempDir())
	env.Web = WebOptions{AllowPrivate: true}
	if _, _, _, err := env.fetchPage(context.Background(), srv.URL, "markdown", 1000); err == nil {
		t.Fatal("binary content should be rejected")
	}
}

func TestTavilySearch(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Authorization") != "Bearer test-key" {
			t.Errorf("missing tavily auth")
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{
				{"title": "Docs", "url": "https://example.com/docs", "content": "Official docs", "published_date": "2026-01-02"},
			},
		})
	}))
	defer srv.Close()

	hits, err := (tavilySearcher{}).Search(context.Background(), "basalt", searchOptions{Max: 3}, WebOptions{
		TavilyKey: "test-key", TavilyBase: srv.URL, AllowPrivate: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(hits) != 1 || hits[0].Title != "Docs" || hits[0].URL != "https://example.com/docs" {
		t.Fatalf("unexpected hits %+v", hits)
	}
}

func TestBraveSearch(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("X-Subscription-Token") != "brave-key" {
			t.Errorf("missing brave token")
		}
		if got := r.URL.Query().Get("q"); !strings.Contains(got, "site:github.com") {
			t.Errorf("site filter missing: %q", got)
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"web": map[string]any{"results": []map[string]any{
				{"title": "Repo", "url": "https://github.com/x/y", "description": "code"},
			}},
		})
	}))
	defer srv.Close()

	hits, err := (braveSearcher{}).Search(context.Background(), "basalt", searchOptions{Max: 5, Site: "github.com"}, WebOptions{
		BraveKey: "brave-key", BraveBase: srv.URL, AllowPrivate: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(hits) != 1 || hits[0].URL != "https://github.com/x/y" {
		t.Fatalf("unexpected hits %+v", hits)
	}
}

func TestSearXNGSearch(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Query().Get("format") != "json" {
			t.Errorf("expected json format")
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{
				{"title": "Result", "url": "https://example.org", "content": "snippet"},
			},
		})
	}))
	defer srv.Close()

	hits, err := (searxngSearcher{}).Search(context.Background(), "q", searchOptions{Max: 3}, WebOptions{
		SearXNGURL: srv.URL, AllowPrivate: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(hits) != 1 || hits[0].Title != "Result" {
		t.Fatalf("unexpected hits %+v", hits)
	}
}

func TestDuckDuckGoSearch(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/html/" || r.Method != http.MethodPost {
			t.Errorf("unexpected %s %s", r.Method, r.URL.Path)
		}
		fmt.Fprint(w, `<html><body>
			<div class="result">
				<a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa&amp;rut=1">Doc A</a>
				<div class="result__snippet">Snippet A</div>
			</div>
			<div class="result">
				<a class="result__a" href="https://example.com/b">Doc B</a>
				<div class="result__snippet">Snippet B</div>
			</div>
		</body></html>`)
	}))
	defer srv.Close()

	hits, err := (ddgSearcher{}).Search(context.Background(), "q", searchOptions{Max: 5}, WebOptions{
		DDGBase: srv.URL, AllowPrivate: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(hits) != 2 {
		t.Fatalf("expected 2 hits, got %d", len(hits))
	}
	if hits[0].URL != "https://example.com/a" {
		t.Fatalf("redirect URL not decoded: %s", hits[0].URL)
	}
	if hits[1].Snippet != "Snippet B" {
		t.Fatalf("snippet not parsed: %+v", hits[1])
	}
}

func TestPickSearcher(t *testing.T) {
	if _, err := pickSearcher(WebOptions{}); err != nil {
		t.Fatalf("empty config should fall back to duckduckgo: %v", err)
	}
	s, _ := pickSearcher(WebOptions{TavilyKey: "k"})
	if s.Name() != "tavily" {
		t.Fatalf("expected tavily, got %s", s.Name())
	}
	if _, err := pickSearcher(WebOptions{Provider: "brave"}); err == nil {
		t.Fatal("missing brave key should error")
	}
}

func TestWebFetchToolMultipleURLs(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		fmt.Fprintf(w, `<html><body><article><h1>Page %s</h1><p>%s</p></article></body></html>`,
			r.URL.Path, strings.Repeat("Body text. ", 20))
	}))
	defer srv.Close()

	env := NewEnv(t.TempDir(), "web", t.TempDir())
	env.Web = WebOptions{AllowPrivate: true}
	args, _ := json.Marshal(map[string]any{"urls": []string{srv.URL + "/a", srv.URL + "/b"}})
	res, err := NewRegistry().Execute(context.Background(), "webfetch", args, env)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(res.Output, "Page /a") || !strings.Contains(res.Output, "Page /b") {
		t.Fatalf("batched fetch missing pages:\n%s", res.Output)
	}
}
