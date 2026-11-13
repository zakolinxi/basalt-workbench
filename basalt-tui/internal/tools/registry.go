package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	"github.com/juang/basalt-tui/internal/llm"
)

// Result is what a tool returns to the agent and the UI.
type Result struct {
	// Title is the one-line summary shown on the tool card.
	Title string
	// Output is the text sent back to the model.
	Output string
	// Display is optional richer text for the UI (diff, tree, …).
	Display string
	// Diff is a unified diff for mutating tools.
	Diff string
	// Meta carries structured counts used by the status bar.
	Meta map[string]any
}

// Tool is one callable capability.
type Tool struct {
	Name        string
	Description string
	Schema      json.RawMessage
	ReadOnly    bool
	Run         func(ctx context.Context, args json.RawMessage, env *Env) (Result, error)
}

// Registry is the ordered tool set exposed to the model.
type Registry struct {
	tools map[string]*Tool
	order []string
}

// NewRegistry builds the default Basalt tool set.
func NewRegistry() *Registry {
	r := &Registry{tools: map[string]*Tool{}}
	for _, t := range []*Tool{
		toolRead(), toolWrite(), toolEdit(), toolList(), toolGlob(),
		toolGrep(), toolBash(), toolTodo(), toolWebFetch(),
	} {
		r.Add(t)
	}
	return r
}

// Add registers a tool.
func (r *Registry) Add(t *Tool) {
	if _, exists := r.tools[t.Name]; !exists {
		r.order = append(r.order, t.Name)
	}
	r.tools[t.Name] = t
}

// Get looks a tool up by name.
func (r *Registry) Get(name string) (*Tool, bool) {
	t, ok := r.tools[name]
	return t, ok
}

// Names lists registered tools in insertion order.
func (r *Registry) Names() []string {
	out := make([]string, len(r.order))
	copy(out, r.order)
	return out
}

// Schemas converts the registry for the LLM request.
func (r *Registry) Schemas() []llm.Tool {
	out := make([]llm.Tool, 0, len(r.order))
	for _, name := range r.order {
		t := r.tools[name]
		out = append(out, llm.Tool{Name: t.Name, Description: t.Description, Parameters: t.Schema})
	}
	return out
}

// Execute runs a tool and converts panics into errors so one bad tool cannot
// take the whole TUI down.
func (r *Registry) Execute(ctx context.Context, name string, args json.RawMessage, env *Env) (res Result, err error) {
	t, ok := r.tools[name]
	if !ok {
		return Result{}, fmt.Errorf("未知工具 %q", name)
	}
	if len(args) == 0 {
		args = json.RawMessage("{}")
	}
	if !json.Valid(args) {
		return Result{}, fmt.Errorf("工具 %s 的参数不是合法 JSON", name)
	}
	defer func() {
		if p := recover(); p != nil {
			err = fmt.Errorf("工具 %s 执行崩溃：%v", name, p)
		}
	}()
	return t.Run(ctx, args, env)
}

// ---- argument helpers -----------------------------------------------------

func argString(args json.RawMessage, keys ...string) string {
	var m map[string]any
	if err := json.Unmarshal(args, &m); err != nil {
		return ""
	}
	for _, k := range keys {
		if v, ok := m[k]; ok {
			switch t := v.(type) {
			case string:
				return t
			case float64:
				return fmt.Sprintf("%.0f", t)
			case bool:
				return fmt.Sprintf("%v", t)
			}
		}
	}
	return ""
}

func argInt(args json.RawMessage, def int, keys ...string) int {
	s := argString(args, keys...)
	if s == "" {
		return def
	}
	var n int
	if _, err := fmt.Sscanf(s, "%d", &n); err != nil {
		return def
	}
	return n
}

func argBool(args json.RawMessage, keys ...string) bool {
	s := strings.ToLower(argString(args, keys...))
	return s == "true" || s == "1" || s == "yes"
}

// truncate keeps the head and tail of long tool output so the model sees both
// the start and the failure at the end.
func truncate(s string, limit int) (string, bool) {
	if len(s) <= limit {
		return s, false
	}
	head := limit * 2 / 3
	tail := limit - head
	return s[:head] + "\n…[输出已截断]…\n" + s[len(s)-tail:], true
}

// sortedKeys is a small helper used by directory listings.
func sortedKeys[V any](m map[string]V) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}
