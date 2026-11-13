package tools

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
)

var heavyDirs = map[string]bool{
	".git": true, "node_modules": true, "target": true, "dist": true,
	"build": true, ".next": true, ".turbo": true, "vendor": true,
	".venv": true, "venv": true, "__pycache__": true, ".cache": true,
	".idea": true, ".gradle": true, ".DS_Store": true,
}

// walkWorkspace visits files under root, skipping heavy and binary files.
func walkWorkspace(root string, fn func(path string, d fs.DirEntry) error) error {
	return filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil // unreadable entries must not abort the walk
		}
		if d.IsDir() {
			name := d.Name()
			if path != root && (heavyDirs[name] || (strings.HasPrefix(name, ".") && name != ".github" && name != ".basalt")) {
				return filepath.SkipDir
			}
			return nil
		}
		return fn(path, d)
	})
}

func toolList() *Tool {
	return &Tool{
		Name:        "list",
		ReadOnly:    true,
		Description: "列出目录结构（树形，自动跳过 node_modules/target 等重型目录）。先用它了解项目布局。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"path":{"type":"string","description":"目录，默认工作区根"},
				"depth":{"type":"integer","description":"递归深度，默认 2，最大 4"}
			}
		}`),
		Run: runList,
	}
}

func runList(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	abs, err := env.Resolve(argString(args, "path", "dir", "directory"))
	if err != nil {
		return Result{}, err
	}
	depth := argInt(args, 2, "depth")
	if depth < 1 {
		depth = 1
	}
	if depth > 4 {
		depth = 4
	}
	var b strings.Builder
	count := 0
	var walk func(dir string, prefix string, level int)
	walk = func(dir string, prefix string, level int) {
		entries, err := os.ReadDir(dir)
		if err != nil {
			return
		}
		sort.Slice(entries, func(i, j int) bool {
			if entries[i].IsDir() != entries[j].IsDir() {
				return entries[i].IsDir()
			}
			return entries[i].Name() < entries[j].Name()
		})
		for _, e := range entries {
			if count > 320 {
				return
			}
			name := e.Name()
			if e.IsDir() {
				if heavyDirs[name] || (strings.HasPrefix(name, ".") && name != ".github" && name != ".basalt") {
					continue
				}
				fmt.Fprintf(&b, "%s%s/\n", prefix, name)
				count++
				if level < depth {
					walk(filepath.Join(dir, name), prefix+"  ", level+1)
				}
				continue
			}
			fmt.Fprintf(&b, "%s%s\n", prefix, name)
			count++
		}
	}
	walk(abs, "", 1)
	out := strings.TrimRight(b.String(), "\n")
	if out == "" {
		out = "（空目录）"
	}
	if count > 320 {
		out += "\n…（目录过大，已截断）"
	}
	title := "list " + env.Rel(abs)
	return Result{Title: title, Output: out, Display: out, Meta: map[string]any{"entries": count}}, nil
}

func toolGlob() *Tool {
	return &Tool{
		Name:        "glob",
		ReadOnly:    true,
		Description: "按通配符查找文件，例如 **/*.ts、src/**/*.rs、*.md。结果按路径排序。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"pattern":{"type":"string","description":"glob 模式，支持 **"},
				"path":{"type":"string","description":"搜索起点，默认工作区根"}
			},
			"required":["pattern"]
		}`),
		Run: runGlob,
	}
}

func runGlob(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	pattern := strings.TrimSpace(argString(args, "pattern", "glob"))
	if pattern == "" {
		return Result{}, fmt.Errorf("缺少 pattern")
	}
	pattern = filepath.ToSlash(pattern)
	abs, err := env.Resolve(argString(args, "path", "dir"))
	if err != nil {
		return Result{}, err
	}
	re, err := globToRegex(pattern)
	if err != nil {
		return Result{}, err
	}
	var matches []string
	walkWorkspace(abs, func(path string, d fs.DirEntry) error {
		rel := filepath.ToSlash(env.Rel(path))
		base := d.Name()
		if re.MatchString(rel) || re.MatchString(base) {
			matches = append(matches, rel)
		}
		return nil
	})
	sort.Strings(matches)
	if len(matches) > 200 {
		matches = matches[:200]
	}
	out := strings.Join(matches, "\n")
	if out == "" {
		out = fmt.Sprintf("没有匹配 %s 的文件", pattern)
	}
	return Result{
		Title:  fmt.Sprintf("glob %s · %d 个文件", pattern, len(matches)),
		Output: out,
		Meta:   map[string]any{"count": len(matches)},
	}, nil
}

func toolGrep() *Tool {
	return &Tool{
		Name:        "grep",
		ReadOnly:    true,
		Description: "用正则搜索文件内容，返回 路径:行号: 内容。适合定位符号、配置与报错来源。",
		Schema: json.RawMessage(`{
			"type":"object",
			"properties":{
				"pattern":{"type":"string","description":"Go 正则表达式"},
				"include":{"type":"string","description":"文件名过滤，例如 *.go 或 *.ts"},
				"path":{"type":"string","description":"搜索目录，默认工作区根"},
				"max":{"type":"integer","description":"最多返回条数，默认 60"}
			},
			"required":["pattern"]
		}`),
		Run: runGrep,
	}
}

func runGrep(ctx context.Context, args json.RawMessage, env *Env) (Result, error) {
	pattern := argString(args, "pattern", "query")
	re, err := regexp.Compile(pattern)
	if err != nil {
		return Result{}, fmt.Errorf("正则无效：%v", err)
	}
	abs, err := env.Resolve(argString(args, "path", "dir"))
	if err != nil {
		return Result{}, err
	}
	include := strings.TrimSpace(argString(args, "include", "glob"))
	var includeRe *regexp.Regexp
	if include != "" {
		includeRe, _ = globToRegex(filepath.ToSlash(include))
	}
	limit := argInt(args, 60, "max", "limit")
	if limit <= 0 || limit > 500 {
		limit = 60
	}

	var results []string
	walkWorkspace(abs, func(path string, d fs.DirEntry) error {
		if len(results) >= limit {
			return filepath.SkipAll
		}
		info, err := d.Info()
		if err != nil || info.Size() > 2<<20 {
			return nil
		}
		name := filepath.ToSlash(d.Name())
		if includeRe != nil && !includeRe.MatchString(name) {
			return nil
		}
		f, err := os.Open(path)
		if err != nil {
			return nil
		}
		defer f.Close()
		rel := env.Rel(path)
		sc := bufio.NewScanner(f)
		sc.Buffer(make([]byte, 0, 64*1024), 512*1024)
		line := 0
		for sc.Scan() {
			line++
			text := sc.Text()
			if strings.IndexByte(text, 0) >= 0 {
				return nil
			}
			if re.MatchString(text) {
				if len(text) > 240 {
					text = text[:240] + "…"
				}
				results = append(results, fmt.Sprintf("%s:%d: %s", rel, line, strings.TrimSpace(text)))
				if len(results) >= limit {
					return filepath.SkipAll
				}
			}
		}
		return nil
	})

	out := strings.Join(results, "\n")
	if out == "" {
		out = "没有匹配结果"
	}
	return Result{
		Title:  fmt.Sprintf("grep %q · %d 处匹配", pattern, len(results)),
		Output: out,
		Meta:   map[string]any{"matches": len(results)},
	}, nil
}

// globToRegex converts a slash-separated glob into an anchored regexp.
func globToRegex(pattern string) (*regexp.Regexp, error) {
	var b strings.Builder
	b.WriteString("^")
	for i := 0; i < len(pattern); i++ {
		c := pattern[i]
		switch c {
		case '*':
			if i+1 < len(pattern) && pattern[i+1] == '*' {
				i++
				if i+1 < len(pattern) && pattern[i+1] == '/' {
					i++
					b.WriteString("(?:.*/)?")
				} else {
					b.WriteString(".*")
				}
			} else {
				b.WriteString("[^/]*")
			}
		case '?':
			b.WriteString("[^/]")
		case '.', '+', '(', ')', '|', '^', '$', '{', '}', '[', ']', '\\':
			b.WriteByte('\\')
			b.WriteByte(c)
		default:
			b.WriteByte(c)
		}
	}
	b.WriteString("$")
	return regexp.Compile(b.String())
}
