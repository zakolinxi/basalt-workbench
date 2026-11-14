package config

import (
	"database/sql"
	"os"
	"path/filepath"
	"testing"
)

const testSchema = `
CREATE TABLE providers (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, category TEXT NOT NULL,
  protocol TEXT NOT NULL, base_url TEXT NOT NULL DEFAULT '',
  models_json TEXT NOT NULL DEFAULT '[]',
  default_chat_model TEXT, default_vision_model TEXT, default_image_model TEXT,
  headers_json TEXT NOT NULL DEFAULT '{}', params_json TEXT NOT NULL DEFAULT '{}',
  timeout_secs INTEGER NOT NULL DEFAULT 120, stream INTEGER NOT NULL DEFAULT 1,
  enabled INTEGER NOT NULL DEFAULT 1, is_template INTEGER NOT NULL DEFAULT 0,
  last_status TEXT, last_error TEXT, last_tested_at TEXT,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
`

func setupDB(t *testing.T) string {
	t.Helper()
	p := filepath.Join(t.TempDir(), "basalt.db")
	db, err := sql.Open("sqlite", p)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if _, err := db.Exec(testSchema); err != nil {
		t.Fatal(err)
	}
	_, err = db.Exec(`INSERT INTO providers
		(id,name,category,protocol,base_url,models_json,default_chat_model,headers_json,enabled,is_template,created_at,updated_at)
		VALUES ('p1','DeepSeek','chat','openai_chat','https://api.deepseek.com/v1',
		 '[{"id":"deepseek-chat"},{"id":"deepseek-reasoner"}]','deepseek-chat','{}',1,0,'x','x')`)
	if err != nil {
		t.Fatal(err)
	}
	_, err = db.Exec(`INSERT INTO settings(key,value) VALUES('app', '{"routing":{"chat":{"providerId":"p1","model":"deepseek-reasoner"}}}')`)
	if err != nil {
		t.Fatal(err)
	}
	return p
}

func TestLoadSharedRegistry(t *testing.T) {
	t.Setenv("BASALT_DB", setupDB(t))
	t.Setenv("BASALT_TUI_CONFIG", filepath.Join(t.TempDir(), "missing.json"))
	loaded := Load("")
	if loaded.Config.Source != "basalt" {
		t.Fatalf("unexpected source %q", loaded.Config.Source)
	}
	if len(loaded.Config.Providers) != 1 {
		t.Fatalf("expected one provider, got %d", len(loaded.Config.Providers))
	}
	p, err := loaded.Config.Resolve("DeepSeek")
	if err != nil {
		t.Fatal(err)
	}
	if p.Kind != "openai_chat" || p.Model != "deepseek-chat" || p.BaseURL == "" {
		t.Fatalf("unexpected provider %+v", p)
	}
}

func TestJSONOverridesSharedProvider(t *testing.T) {
	dbPath := setupDB(t)
	t.Setenv("BASALT_DB", dbPath)
	override := filepath.Join(t.TempDir(), "config.json")
	if err := os.WriteFile(override, []byte(`{
		"default": "DeepSeek",
		"autoApproveRead": false,
		"providers": {"DeepSeek": {"model": "deepseek-reasoner", "maxTokens": 4096}}
	}`), 0o644); err != nil {
		t.Fatal(err)
	}
	t.Setenv("BASALT_TUI_CONFIG", override)
	loaded := Load("")
	if loaded.Config.Default != "p1" {
		t.Fatalf("default should resolve to the shared id, got %q", loaded.Config.Default)
	}
	p, _ := loaded.Config.Resolve("p1")
	if p.Model != "deepseek-reasoner" || p.MaxTokens != 4096 {
		t.Fatalf("override not applied: %+v", p)
	}
	if p.Name != "DeepSeek" || p.BaseURL == "" {
		t.Fatalf("shared fields should be preserved: %+v", p)
	}
	if loaded.Config.AutoApprove {
		t.Fatal("explicit false should disable read auto-approval")
	}
}

func TestBuiltinFallback(t *testing.T) {
	t.Setenv("BASALT_DB", filepath.Join(t.TempDir(), "missing.db"))
	t.Setenv("BASALT_TUI_CONFIG", filepath.Join(t.TempDir(), "missing.json"))
	loaded := Load("")
	if loaded.Config.Source != "builtin" {
		t.Fatalf("unexpected source %q", loaded.Config.Source)
	}
	if len(loaded.Config.Providers) < 5 {
		t.Fatalf("expected builtin presets, got %d", len(loaded.Config.Providers))
	}
}

func TestNormalizeKindsAndURLs(t *testing.T) {
	cases := map[string]string{
		"openai":           "openai_chat",
		"openai_chat":      "openai_chat",
		"openai_responses": "openai_responses",
		"gemini":           "gemini",
		"ollama":           "ollama",
	}
	for in, want := range cases {
		if got := normalizeKind(in); got != want {
			t.Fatalf("normalizeKind(%q)=%q, want %q", in, got, want)
		}
	}
	if got := normalizeBaseURL("https://x/v1/chat/completions", "openai_chat"); got != "https://x/v1" {
		t.Fatalf("endpoint suffix not trimmed: %s", got)
	}
	if got := normalizeBaseURL("http://localhost:11434/api", "ollama"); got != "http://localhost:11434" {
		t.Fatalf("ollama suffix not trimmed: %s", got)
	}
}
