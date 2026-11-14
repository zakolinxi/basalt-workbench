// Package basaltdb reads the provider registry of the Basalt desktop
// workbench. The TUI and the desktop app share one SQLite database so a
// provider configured once is available in both places.
package basaltdb

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	_ "modernc.org/sqlite"
)

// Identifier must match the Tauri bundle identifier.
const Identifier = "com.basalt.workbench"

// Model is one chat model entry from models_json.
type Model struct {
	ID            string `json:"id"`
	Name          string `json:"name"`
	ContextLength *int64 `json:"contextLength,omitempty"`
}

// Provider is one row of the providers table.
type Provider struct {
	ID               string
	Name             string
	Category         string
	Protocol         string
	BaseURL          string
	Models           []Model
	DefaultChatModel string
	Headers          map[string]string
	TimeoutSecs      int
	Stream           bool
	Enabled          bool
	IsTemplate       bool
}

// Snapshot is the subset of the workbench database the TUI needs.
type Snapshot struct {
	Path           string
	Providers      []Provider
	ChatProviderID string
	ChatModel      string
	Theme          string
}

// DefaultDir resolves the application data directory used by Tauri v2.
func DefaultDir() string {
	if home := os.Getenv("BASALT_HOME"); home != "" {
		return home
	}
	switch runtime.GOOS {
	case "windows":
		if appData := os.Getenv("APPDATA"); appData != "" {
			return filepath.Join(appData, Identifier)
		}
	case "darwin":
		if home, err := os.UserHomeDir(); err == nil {
			return filepath.Join(home, "Library", "Application Support", Identifier)
		}
	default:
		if data := os.Getenv("XDG_DATA_HOME"); data != "" {
			return filepath.Join(data, Identifier)
		}
		if home, err := os.UserHomeDir(); err == nil {
			return filepath.Join(home, ".local", "share", Identifier)
		}
	}
	return ""
}

// DefaultPath returns the workbench database path.
func DefaultPath() string {
	dir := DefaultDir()
	if dir == "" {
		return ""
	}
	return filepath.Join(dir, "basalt.db")
}

// GetenvPath lets tests or power users pin an explicit database.
func path() string {
	if p := os.Getenv("BASALT_DB"); p != "" {
		return p
	}
	return DefaultPath()
}

// Load reads the default database. A missing database is not an error: the
// caller falls back to standalone configuration.
func Load() (*Snapshot, error) {
	p := path()
	if p == "" {
		return nil, nil
	}
	if _, err := os.Stat(p); err != nil {
		return nil, nil
	}
	return Open(p)
}

// Open reads a specific database file in read-only mode.
func Open(p string) (*Snapshot, error) {
	dsn := "file:" + filepath.ToSlash(p) + "?mode=ro&_pragma=busy_timeout(3000)"
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("打开 Basalt 数据库失败：%w", err)
	}
	defer db.Close()

	rows, err := db.Query(`SELECT id, name, category, protocol, base_url, models_json,
		COALESCE(default_chat_model, ''), headers_json, timeout_secs, stream, enabled, is_template
		FROM providers ORDER BY is_template ASC, category ASC, name ASC`)
	if err != nil {
		return nil, fmt.Errorf("读取 providers 失败：%w", err)
	}
	defer rows.Close()

	snap := &Snapshot{Path: p}
	for rows.Next() {
		var (
			p                     Provider
			models, headers       string
			timeout, stream, enab int
			isTemplate            int
		)
		if err := rows.Scan(&p.ID, &p.Name, &p.Category, &p.Protocol, &p.BaseURL,
			&models, &p.DefaultChatModel, &headers, &timeout, &stream, &enab, &isTemplate); err != nil {
			return nil, err
		}
		p.TimeoutSecs = timeout
		p.Stream = stream != 0
		p.Enabled = enab != 0
		p.IsTemplate = isTemplate != 0
		_ = json.Unmarshal([]byte(models), &p.Models)
		p.Headers = parseHeaders(headers)
		snap.Providers = append(snap.Providers, p)
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}

	var raw string
	if err := db.QueryRow(`SELECT value FROM settings WHERE key = 'app'`).Scan(&raw); err == nil {
		var app struct {
			Theme   string `json:"theme"`
			Routing map[string]struct {
				ProviderID string `json:"providerId"`
				Model      string `json:"model"`
			} `json:"routing"`
		}
		if json.Unmarshal([]byte(raw), &app) == nil {
			snap.Theme = app.Theme
			if r, ok := app.Routing["chat"]; ok {
				snap.ChatProviderID = r.ProviderID
				snap.ChatModel = r.Model
			}
		}
	}
	return snap, nil
}

// ChatProviders returns enabled, non-template chat providers.
func (s *Snapshot) ChatProviders() []Provider {
	if s == nil {
		return nil
	}
	out := make([]Provider, 0, len(s.Providers))
	for _, p := range s.Providers {
		if p.Category == "chat" && p.Enabled && !p.IsTemplate {
			out = append(out, p)
		}
	}
	return out
}

// Find returns a chat provider by id first, then by case-insensitive name.
func (s *Snapshot) Find(nameOrID string) (Provider, bool) {
	if s == nil {
		return Provider{}, false
	}
	for _, p := range s.Providers {
		if p.ID == nameOrID && p.Category == "chat" && !p.IsTemplate {
			return p, true
		}
	}
	for _, p := range s.Providers {
		if p.Category == "chat" && !p.IsTemplate && strings.EqualFold(p.Name, nameOrID) {
			return p, true
		}
	}
	return Provider{}, false
}

// DefaultChatModel picks the provider and model to start with.
func (s *Snapshot) DefaultChatModel() (Provider, string, bool) {
	if p, ok := s.Find(s.ChatProviderID); ok {
		model := s.ChatModel
		if model == "" {
			model = firstModel(p)
		}
		return p, model, true
	}
	for _, p := range s.ChatProviders() {
		return p, firstModel(p), true
	}
	return Provider{}, "", false
}

func firstModel(p Provider) string {
	if p.DefaultChatModel != "" {
		return p.DefaultChatModel
	}
	if len(p.Models) > 0 {
		return p.Models[0].ID
	}
	return ""
}

func parseHeaders(raw string) map[string]string {
	var anyMap map[string]any
	if json.Unmarshal([]byte(raw), &anyMap) != nil {
		return nil
	}
	out := make(map[string]string, len(anyMap))
	for k, v := range anyMap {
		if s, ok := v.(string); ok && s != "" {
			out[k] = s
		}
	}
	return out
}
