package basaltdb

import (
	"database/sql"
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

func makeDB(t *testing.T) string {
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
		VALUES
		('p1','DeepSeek','chat','openai_chat','https://api.deepseek.com/v1',
		 '[{"id":"deepseek-chat","name":"DeepSeek Chat"},{"id":"deepseek-reasoner"}]',
		 'deepseek-chat','{"X-Trace":"1"}',1,0,'2026-01-01','2026-01-02'),
		('p2','Template','chat','openai_chat','', '[]', NULL,'{}',0,1,'2026-01-01','2026-01-02'),
		('p3','Image','image','openai_images','', '[]', NULL,'{}',1,0,'2026-01-01','2026-01-02')`)
	if err != nil {
		t.Fatal(err)
	}
	_, err = db.Exec(`INSERT INTO settings(key,value) VALUES('app', ?)`,
		`{"theme":"dark","routing":{"chat":{"providerId":"p1","model":"deepseek-reasoner"}}}`)
	if err != nil {
		t.Fatal(err)
	}
	return p
}

func TestOpenFiltersAndRoutes(t *testing.T) {
	snap, err := Open(makeDB(t))
	if err != nil {
		t.Fatal(err)
	}
	chat := snap.ChatProviders()
	if len(chat) != 1 || chat[0].Name != "DeepSeek" {
		t.Fatalf("unexpected chat providers %+v", chat)
	}
	if chat[0].Headers["X-Trace"] != "1" {
		t.Fatalf("headers not parsed: %+v", chat[0].Headers)
	}
	if snap.ChatProviderID != "p1" || snap.ChatModel != "deepseek-reasoner" {
		t.Fatalf("route not parsed: %s %s", snap.ChatProviderID, snap.ChatModel)
	}
	p, model, ok := snap.DefaultChatModel()
	if !ok || p.ID != "p1" || model != "deepseek-reasoner" {
		t.Fatalf("unexpected default %+v %s", p, model)
	}
	if found, ok := snap.Find("deepseek"); !ok || found.ID != "p1" {
		t.Fatalf("case-insensitive name lookup failed")
	}
}

func TestMissingDatabaseIsNotAnError(t *testing.T) {
	t.Setenv("BASALT_DB", filepath.Join(t.TempDir(), "nope.db"))
	snap, err := Load()
	if err != nil || snap != nil {
		t.Fatalf("expected nil,nil for missing db, got %v %v", snap, err)
	}
}
