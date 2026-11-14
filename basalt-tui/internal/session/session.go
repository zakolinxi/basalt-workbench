// Package session persists conversations as JSON documents.
package session

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/juang/basalt-tui/internal/llm"
)

// Usage accumulates token accounting for one session.
type Usage struct {
	Input     int `json:"input"`
	Output    int `json:"output"`
	Cached    int `json:"cached"`
	Requests  int `json:"requests"`
	ToolCalls int `json:"toolCalls"`
}

// Add merges a provider usage snapshot.
func (u *Usage) Add(in, out, cached int) {
	u.Input += in
	u.Output += out
	u.Cached += cached
}

// Session is a persisted conversation.
type Session struct {
	ID        string        `json:"id"`
	Title     string        `json:"title"`
	CreatedAt time.Time     `json:"createdAt"`
	UpdatedAt time.Time     `json:"updatedAt"`
	Provider  string        `json:"provider"`
	Model     string        `json:"model"`
	Workspace string        `json:"workspace"`
	Messages  []llm.Message `json:"messages"`
	Usage     Usage         `json:"usage"`
	// ContextTokens is the prompt size of the last request and CacheTokens is
	// how much of it was served from the provider cache.
	ContextTokens int `json:"contextTokens,omitempty"`
	CacheTokens   int `json:"cacheTokens,omitempty"`
}

// Store persists sessions under a directory.
type Store struct {
	Dir string
}

// NewStore creates the session directory.
func NewStore(dir string) (*Store, error) {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, err
	}
	return &Store{Dir: dir}, nil
}

// New creates an empty session with a sortable random id.
func New(workspace, provider, model string) *Session {
	now := time.Now()
	return &Session{
		ID:        now.Format("20060102-150405") + "-" + randHex(3),
		Title:     "新会话",
		CreatedAt: now,
		UpdatedAt: now,
		Provider:  provider,
		Model:     model,
		Workspace: workspace,
	}
}

// TitleFrom derives a session title from the first user message.
func TitleFrom(text string) string {
	text = strings.TrimSpace(strings.ReplaceAll(text, "\n", " "))
	if text == "" {
		return "新会话"
	}
	r := []rune(text)
	if len(r) > 36 {
		return string(r[:36]) + "…"
	}
	return text
}

// Save writes the session atomically.
func (s *Store) Save(sess *Session) error {
	sess.UpdatedAt = time.Now()
	raw, err := json.MarshalIndent(sess, "", "  ")
	if err != nil {
		return err
	}
	path := filepath.Join(s.Dir, sess.ID+".json")
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, raw, 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

// Load reads a session by id.
func (s *Store) Load(id string) (*Session, error) {
	raw, err := os.ReadFile(filepath.Join(s.Dir, id+".json"))
	if err != nil {
		return nil, err
	}
	var sess Session
	if err := json.Unmarshal(raw, &sess); err != nil {
		return nil, err
	}
	return &sess, nil
}

// Summary is a list entry.
type Summary struct {
	ID        string    `json:"id"`
	Title     string    `json:"title"`
	UpdatedAt time.Time `json:"updatedAt"`
	Model     string    `json:"model"`
	Messages  int       `json:"messages"`
}

// List returns sessions newest first.
func (s *Store) List() []Summary {
	entries, err := os.ReadDir(s.Dir)
	if err != nil {
		return nil
	}
	out := make([]Summary, 0, len(entries))
	for _, e := range entries {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".json") {
			continue
		}
		raw, err := os.ReadFile(filepath.Join(s.Dir, e.Name()))
		if err != nil {
			continue
		}
		var sess Session
		if json.Unmarshal(raw, &sess) != nil {
			continue
		}
		out = append(out, Summary{ID: sess.ID, Title: sess.Title, UpdatedAt: sess.UpdatedAt, Model: sess.Model, Messages: len(sess.Messages)})
	}
	sort.Slice(out, func(i, j int) bool { return out[i].UpdatedAt.After(out[j].UpdatedAt) })
	return out
}

// Delete removes a session file.
func (s *Store) Delete(id string) error {
	return os.Remove(filepath.Join(s.Dir, id+".json"))
}

// Transcript renders the conversation for /export.
func (sess *Session) Transcript() string {
	var b strings.Builder
	fmt.Fprintf(&b, "# %s\n\n模型：%s · 工作区：%s\n\n", sess.Title, sess.Model, sess.Workspace)
	for _, m := range sess.Messages {
		switch m.Role {
		case "user":
			fmt.Fprintf(&b, "## 用户\n\n%s\n\n", m.Content)
		case "assistant":
			if strings.TrimSpace(m.Content) != "" {
				fmt.Fprintf(&b, "## Basalt\n\n%s\n\n", m.Content)
			}
			for _, tc := range m.ToolCalls {
				fmt.Fprintf(&b, "> 工具调用：`%s` `%s`\n\n", tc.Name, compactArgs(tc.Arguments))
			}
		}
	}
	return b.String()
}

func compactArgs(args string) string {
	args = strings.ReplaceAll(args, "\n", " ")
	if len(args) > 100 {
		return args[:100] + "…"
	}
	return args
}

func randHex(n int) string {
	buf := make([]byte, n)
	if _, err := rand.Read(buf); err != nil {
		return fmt.Sprintf("%x", time.Now().UnixNano())
	}
	return hex.EncodeToString(buf)
}
