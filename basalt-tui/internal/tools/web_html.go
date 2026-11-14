package tools

import (
	"bytes"
	"fmt"
	"net/url"
	"strings"

	readability "github.com/go-shiori/go-readability"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// extractArticle pulls the readable body out of an HTML document. It falls back
// to a full-document conversion when readability cannot find enough text.
func extractArticle(raw []byte, pageURL *url.URL) (title, markdown string) {
	raw = bytes.TrimSpace(raw)
	if len(raw) == 0 {
		return "", ""
	}
	defer func() {
		if recover() != nil {
			title, markdown = "", htmlToMarkdown(raw, pageURL)
		}
	}()
	article, err := readability.FromReader(bytes.NewReader(raw), pageURL)
	if err == nil && strings.TrimSpace(article.TextContent) != "" {
		md := htmlToMarkdown([]byte(article.Content), pageURL)
		if strings.TrimSpace(md) != "" {
			return strings.TrimSpace(article.Title), md
		}
	}
	return "", htmlToMarkdown(raw, pageURL)
}

// plainText strips markup, keeping paragraph breaks.
func plainText(raw []byte) string {
	doc, err := html.Parse(bytes.NewReader(raw))
	if err != nil {
		return strings.TrimSpace(string(raw))
	}
	var b strings.Builder
	walkHTML(doc, &b, nil, false)
	return normalizeMarkdown(b.String())
}

// htmlToMarkdown converts a document into agent-friendly Markdown.
func htmlToMarkdown(raw []byte, pageURL *url.URL) string {
	doc, err := html.Parse(bytes.NewReader(raw))
	if err != nil {
		return strings.TrimSpace(string(raw))
	}
	var b strings.Builder
	walkHTML(doc, &b, pageURL, false)
	return normalizeMarkdown(b.String())
}

var skipTags = map[atom.Atom]bool{
	atom.Script: true, atom.Style: true, atom.Noscript: true, atom.Svg: true,
	atom.Iframe: true, atom.Form: true, atom.Nav: true, atom.Aside: true,
	atom.Button: true, atom.Select: true, atom.Textarea: true, atom.Canvas: true,
	atom.Video: true, atom.Audio: true, atom.Object: true, atom.Embed: true,
}

func walkHTML(n *html.Node, b *strings.Builder, base *url.URL, inPre bool) {
	switch n.Type {
	case html.DocumentNode:
		walkChildren(n, b, base, inPre)
		return
	case html.TextNode:
		text := n.Data
		if inPre {
			b.WriteString(text)
			return
		}
		text = strings.Join(strings.Fields(text), " ")
		if text == "" {
			return
		}
		b.WriteString(text)
		return
	case html.ElementNode:
	default:
		return
	}

	tag := n.DataAtom
	if skipTags[tag] {
		return
	}

	switch tag {
	case atom.Br:
		b.WriteString("\n")
	case atom.Hr:
		b.WriteString("\n\n---\n\n")
	case atom.H1, atom.H2, atom.H3, atom.H4, atom.H5, atom.H6:
		// atom constants for H1..H6 are not contiguous, so map explicitly.
		level := 1
		switch tag {
		case atom.H2:
			level = 2
		case atom.H3:
			level = 3
		case atom.H4:
			level = 4
		case atom.H5:
			level = 5
		case atom.H6:
			level = 6
		}
		b.WriteString("\n\n" + strings.Repeat("#", level) + " ")
		walkChildren(n, b, base, inPre)
		b.WriteString("\n\n")
	case atom.P, atom.Div, atom.Section, atom.Article, atom.Main, atom.Header, atom.Footer, atom.Figure:
		b.WriteString("\n\n")
		walkChildren(n, b, base, inPre)
		b.WriteString("\n\n")
	case atom.Ul:
		b.WriteString("\n")
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			if c.Type == html.ElementNode && c.DataAtom == atom.Li {
				b.WriteString("\n- ")
				walkChildren(c, b, base, inPre)
			} else {
				walkHTML(c, b, base, inPre)
			}
		}
		b.WriteString("\n")
	case atom.Ol:
		b.WriteString("\n")
		idx := 1
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			if c.Type == html.ElementNode && c.DataAtom == atom.Li {
				fmt.Fprintf(b, "\n%d. ", idx)
				idx++
				walkChildren(c, b, base, inPre)
			} else {
				walkHTML(c, b, base, inPre)
			}
		}
		b.WriteString("\n")
	case atom.Li:
		b.WriteString("\n- ")
		walkChildren(n, b, base, inPre)
	case atom.Pre:
		b.WriteString("\n\n```\n")
		walkChildren(n, b, base, true)
		b.WriteString("\n```\n\n")
	case atom.Code:
		if inPre {
			walkChildren(n, b, base, true)
		} else {
			b.WriteString("`")
			walkChildren(n, b, base, false)
			b.WriteString("`")
		}
	case atom.Blockquote:
		var inner strings.Builder
		walkChildren(n, &inner, base, inPre)
		text := strings.TrimSpace(inner.String())
		if text != "" {
			b.WriteString("\n\n> ")
			b.WriteString(strings.ReplaceAll(text, "\n", "\n> "))
			b.WriteString("\n\n")
		}
	case atom.A:
		href := resolveHref(base, attr(n, "href"))
		label := textOf(n)
		switch {
		case href == "":
			b.WriteString(label)
		case label == "" || label == href:
			b.WriteString(href)
		default:
			b.WriteString("[" + label + "](" + href + ")")
		}
	case atom.Img:
		if inPre {
			return
		}
		src := resolveHref(base, attr(n, "src"))
		alt := strings.TrimSpace(attr(n, "alt"))
		if src != "" {
			b.WriteString("![" + alt + "](" + src + ")")
		}
	case atom.Strong, atom.B:
		b.WriteString("**")
		walkChildren(n, b, base, inPre)
		b.WriteString("**")
	case atom.Em, atom.I:
		b.WriteString("*")
		walkChildren(n, b, base, inPre)
		b.WriteString("*")
	default:
		walkChildren(n, b, base, inPre)
	}
}

func walkChildren(n *html.Node, b *strings.Builder, base *url.URL, inPre bool) {
	for c := n.FirstChild; c != nil; c = c.NextSibling {
		walkHTML(c, b, base, inPre)
	}
}

func attr(n *html.Node, key string) string {
	for _, a := range n.Attr {
		if a.Key == key {
			return a.Val
		}
	}
	return ""
}

func textOf(n *html.Node) string {
	var b strings.Builder
	collectText(n, &b)
	return strings.TrimSpace(b.String())
}

// collectText gathers text without re-entering link formatting, which would
// recurse for <a> nodes.
func collectText(n *html.Node, b *strings.Builder) {
	switch n.Type {
	case html.TextNode:
		b.WriteString(n.Data)
		return
	case html.ElementNode:
		if skipTags[n.DataAtom] {
			return
		}
	}
	for c := n.FirstChild; c != nil; c = c.NextSibling {
		collectText(c, b)
	}
}

func resolveHref(base *url.URL, href string) string {
	href = strings.TrimSpace(href)
	if href == "" || strings.HasPrefix(href, "javascript:") || strings.HasPrefix(href, "#") {
		return ""
	}
	if base == nil {
		return href
	}
	ref, err := url.Parse(href)
	if err != nil {
		return href
	}
	return base.ResolveReference(ref).String()
}

func normalizeMarkdown(s string) string {
	s = strings.ReplaceAll(s, "\r\n", "\n")
	s = strings.ReplaceAll(s, "\u00a0", " ")
	lines := strings.Split(s, "\n")
	for i, line := range lines {
		lines[i] = strings.TrimRight(line, " \t")
	}
	s = strings.Join(lines, "\n")
	for strings.Contains(s, "\n\n\n") {
		s = strings.ReplaceAll(s, "\n\n\n", "\n\n")
	}
	return strings.TrimSpace(s)
}
