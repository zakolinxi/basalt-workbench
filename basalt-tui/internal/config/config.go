// Package config resolves the provider registry shared with the Basalt
// desktop workbench.
//
// Provider definitions live in the workbench SQLite database, API keys live in
// the Windows Credential Manager, and both apps read the same rows. A JSON file
// can still override individual fields or point at a database elsewhere:
//
//	$BASALT_TUI_CONFIG
//	<workspace>/.basalt/config.json
//	<UserConfigDir>/basalt-tui/config.json
//
// When no workbench database exists the built-in presets keep the TUI usable on
// its own.
package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"github.com/juang/basalt-tui/internal/basaltdb"
	"github.com/juang/basalt-tui/internal/secrets"
)

// ProviderConfig is the runtime view of one provider.
type ProviderConfig struct {
	ID          string            `json:"id,omitempty"`
	Name        string            `json:"name,omitempty"`
	Kind        string            `json:"kind"` // openai_chat | openai_responses | anthropic | gemini | ollama
	Model       string            `json:"model"`
	BaseURL     string            `json:"baseUrl,omitempty"`
	APIKey      string            `json:"apiKey,omitempty"`
	MaxTokens   int               `json:"maxTokens,omitempty"`
	Temperature float64           `json:"temperature,omitempty"`
	Reasoning   string            `json:"reasoning,omitempty"`
	Headers     map[string]string `json:"headers,omitempty"`
	TimeoutSecs int               `json:"timeoutSecs,omitempty"`
	// Source is "basalt", "config" or "builtin" and is shown in /model.
	Source string `json:"-"`
	// Models lists the models known by the desktop provider.
	Models []string `json:"-"`
}

// Config is the merged runtime configuration.
type Config struct {
	Default      string                    `json:"default"`
	Providers    map[string]ProviderConfig `json:"providers"`
	Theme        string                    `json:"theme,omitempty"`
	AutoApprove  bool                      `json:"autoApproveRead"`
	CompactAfter int                       `json:"compactAfter,omitempty"`
	// Source reports where the provider list came from.
	Source string `json:"-"`
	DBPath string `json:"-"`
}

// Loaded carries the effective config and the file it came from.
type Loaded struct {
	Config Config
	Path   string
}

var envByProvider = map[string]string{
	"openai":     "OPENAI_API_KEY",
	"anthropic":  "ANTHROPIC_API_KEY",
	"deepseek":   "DEEPSEEK_API_KEY",
	"moonshot":   "MOONSHOT_API_KEY",
	"zhipu":      "ZHIPU_API_KEY",
	"openrouter": "OPENROUTER_API_KEY",
	"groq":       "GROQ_API_KEY",
	"gemini":     "GEMINI_API_KEY",
}

func builtinPresets() Config {
	return Config{
		Default: "deepseek",
		Providers: map[string]ProviderConfig{
			"deepseek": {
				Name: "DeepSeek", Kind: "openai_chat", BaseURL: "https://api.deepseek.com/v1",
				Model: "deepseek-chat", MaxTokens: 8192, Source: "builtin",
			},
			"openai": {
				Name: "OpenAI", Kind: "openai_chat", BaseURL: "https://api.openai.com/v1",
				Model: "gpt-4.1", MaxTokens: 8192, Source: "builtin",
			},
			"anthropic": {
				Name: "Anthropic", Kind: "anthropic", BaseURL: "https://api.anthropic.com",
				Model: "claude-sonnet-4-5", MaxTokens: 8192, Source: "builtin",
			},
			"moonshot": {
				Name: "Moonshot", Kind: "openai_chat", BaseURL: "https://api.moonshot.cn/v1",
				Model: "kimi-k2-0905-preview", MaxTokens: 8192, Source: "builtin",
			},
			"zhipu": {
				Name: "Zhipu", Kind: "openai_chat", BaseURL: "https://open.bigmodel.cn/api/paas/v4",
				Model: "glm-4.6", MaxTokens: 8192, Source: "builtin",
			},
			"ollama": {
				Name: "Ollama", Kind: "ollama", BaseURL: "http://localhost:11434",
				Model: "qwen2.5-coder:7b", MaxTokens: 8192, Source: "builtin",
			},
			"lmstudio": {
				Name: "LM Studio", Kind: "openai_chat", BaseURL: "http://localhost:1234/v1",
				Model: "local-model", MaxTokens: 8192, Source: "builtin",
			},
		},
		Theme:        "basalt",
		AutoApprove:  true,
		CompactAfter: 48,
		Source:       "builtin",
	}
}

// Load merges the workbench database with an optional JSON override file.
func Load(workspace string) Loaded {
	cfg := Config{Providers: map[string]ProviderConfig{}, AutoApprove: true, CompactAfter: 48}

	// 1. Shared provider registry from the desktop workbench.
	if snap, err := basaltdb.Load(); err == nil && snap != nil {
		cfg.DBPath = snap.Path
		cfg.Theme = snap.Theme
		cfg.Source = "basalt"
		for _, p := range snap.ChatProviders() {
			cfg.Providers[p.ID] = fromDB(p)
		}
		if snap.ChatProviderID != "" {
			cfg.Default = snap.ChatProviderID
		}
	}

	// 2. JSON overrides.
	path, file := readConfigFile(workspace)
	if file != nil {
		mergeFile(&cfg, *file)
		if cfg.Source == "" {
			cfg.Source = "config"
		} else {
			cfg.Source += "+config"
		}
	}

	// 3. Built-in presets when nothing else is available.
	if len(cfg.Providers) == 0 {
		cfg = builtinPresets()
	}

	// 4. Resolve the default provider.
	cfg.Default = resolveDefault(cfg)
	return Loaded{Config: cfg, Path: path}
}

func fromDB(p basaltdb.Provider) ProviderConfig {
	model := p.DefaultChatModel
	if model == "" && len(p.Models) > 0 {
		model = p.Models[0].ID
	}
	models := make([]string, 0, len(p.Models))
	for _, m := range p.Models {
		models = append(models, m.ID)
	}
	key, _ := secrets.ReadProviderKey(p.ID)
	return ProviderConfig{
		ID: p.ID, Name: p.Name, Kind: normalizeKind(p.Protocol), Model: model,
		BaseURL: p.BaseURL, APIKey: key, Headers: p.Headers,
		TimeoutSecs: p.TimeoutSecs, Source: "basalt", Models: models,
	}
}

func normalizeKind(kind string) string {
	switch strings.ToLower(strings.TrimSpace(kind)) {
	case "", "openai", "openai_chat", "openai-chat", "chat":
		return "openai_chat"
	case "openai_responses", "openai-responses", "responses":
		return "openai_responses"
	case "anthropic":
		return "anthropic"
	case "gemini", "google":
		return "gemini"
	case "ollama":
		return "ollama"
	default:
		return strings.ToLower(kind)
	}
}

func readConfigFile(workspace string) (string, *fileConfig) {
	var candidates []string
	if p := os.Getenv("BASALT_TUI_CONFIG"); p != "" {
		candidates = append(candidates, p)
	}
	if workspace != "" {
		candidates = append(candidates, filepath.Join(workspace, ".basalt", "config.json"))
	}
	if dir, err := os.UserConfigDir(); err == nil {
		candidates = append(candidates, filepath.Join(dir, "basalt-tui", "config.json"))
	}
	for _, p := range candidates {
		raw, err := os.ReadFile(p)
		if err != nil {
			continue
		}
		var cfg fileConfig
		if err := json.Unmarshal(raw, &cfg); err != nil {
			continue
		}
		return p, &cfg
	}
	return "", nil
}

// fileConfig distinguishes "field absent" from a deliberate false.
type fileConfig struct {
	Default      string                    `json:"default"`
	Providers    map[string]ProviderConfig `json:"providers"`
	Theme        string                    `json:"theme,omitempty"`
	AutoApprove  *bool                     `json:"autoApproveRead"`
	CompactAfter int                       `json:"compactAfter,omitempty"`
}

// mergeFile applies JSON overrides onto the shared registry. A JSON provider
// key may be a provider id, a provider name, or a new alias.
func mergeFile(cfg *Config, file fileConfig) {
	for name, incoming := range file.Providers {
		key := matchProviderKey(*cfg, name)
		if key == "" {
			key = name
		}
		cur, exists := cfg.Providers[key]
		if !exists {
			cur = ProviderConfig{Name: name, Source: "config"}
		}
		if incoming.Name != "" {
			cur.Name = incoming.Name
		}
		if incoming.Kind != "" {
			cur.Kind = normalizeKind(incoming.Kind)
		}
		if incoming.Model != "" {
			cur.Model = incoming.Model
		}
		if incoming.BaseURL != "" {
			cur.BaseURL = incoming.BaseURL
		}
		if incoming.APIKey != "" {
			cur.APIKey = expand(incoming.APIKey)
		}
		if incoming.MaxTokens > 0 {
			cur.MaxTokens = incoming.MaxTokens
		}
		if incoming.Temperature > 0 {
			cur.Temperature = incoming.Temperature
		}
		if incoming.Reasoning != "" {
			cur.Reasoning = incoming.Reasoning
		}
		if len(incoming.Headers) > 0 {
			if cur.Headers == nil {
				cur.Headers = map[string]string{}
			}
			for k, v := range incoming.Headers {
				cur.Headers[k] = v
			}
		}
		if incoming.TimeoutSecs > 0 {
			cur.TimeoutSecs = incoming.TimeoutSecs
		}
		if cur.Source == "" || cur.Source == "basalt" {
			cur.Source = "basalt+config"
		}
		cfg.Providers[key] = cur
	}
	if file.Default != "" {
		cfg.Default = file.Default
	}
	if file.Theme != "" {
		cfg.Theme = file.Theme
	}
	if file.CompactAfter > 0 {
		cfg.CompactAfter = file.CompactAfter
	}
	if file.AutoApprove != nil {
		cfg.AutoApprove = *file.AutoApprove
	}
}

func matchProviderKey(cfg Config, nameOrID string) string {
	if nameOrID == "" {
		return ""
	}
	if _, ok := cfg.Providers[nameOrID]; ok {
		return nameOrID
	}
	for key, p := range cfg.Providers {
		if strings.EqualFold(key, nameOrID) || strings.EqualFold(p.Name, nameOrID) {
			return key
		}
	}
	return ""
}

func resolveDefault(cfg Config) string {
	if key := matchProviderKey(cfg, cfg.Default); key != "" {
		return key
	}
	// Prefer a provider with a usable key, then fall back to the first one.
	keys := make([]string, 0, len(cfg.Providers))
	for k := range cfg.Providers {
		keys = append(keys, k)
	}
	sort.Slice(keys, func(i, j int) bool {
		return strings.ToLower(cfg.Providers[keys[i]].Name) < strings.ToLower(cfg.Providers[keys[j]].Name)
	})
	for _, k := range keys {
		if cfg.Providers[k].APIKey != "" {
			return k
		}
	}
	if len(keys) > 0 {
		return keys[0]
	}
	return ""
}

// Find resolves a provider by id, map key or display name.
func (c Config) Find(nameOrID string) (string, ProviderConfig, bool) {
	if nameOrID == "" {
		nameOrID = c.Default
	}
	if p, ok := c.Providers[nameOrID]; ok {
		return nameOrID, p, true
	}
	for key, p := range c.Providers {
		if strings.EqualFold(p.Name, nameOrID) {
			return key, p, true
		}
	}
	return "", ProviderConfig{}, false
}

// Resolve returns the provider with env expansion and defaults applied.
func (c Config) Resolve(name string) (ProviderConfig, error) {
	key, p, ok := c.Find(name)
	if !ok {
		return ProviderConfig{}, fmt.Errorf("unknown provider %q", name)
	}
	_ = key
	p.Kind = normalizeKind(p.Kind)
	p.APIKey = expand(p.APIKey)
	if p.APIKey == "" {
		if env := envByProvider[strings.ToLower(strings.TrimSpace(p.Name))]; env != "" {
			p.APIKey = os.Getenv(env)
		}
	}
	if p.BaseURL == "" {
		switch p.Kind {
		case "anthropic":
			p.BaseURL = "https://api.anthropic.com"
		case "ollama":
			p.BaseURL = "http://localhost:11434"
		case "gemini":
			p.BaseURL = "https://generativelanguage.googleapis.com/v1beta"
		default:
			p.BaseURL = "https://api.openai.com/v1"
		}
	}
	p.BaseURL = normalizeBaseURL(p.BaseURL, p.Kind)
	if p.MaxTokens <= 0 {
		p.MaxTokens = 8192
	}
	if p.Name == "" {
		p.Name = key
	}
	if p.ID == "" {
		p.ID = key
	}
	return p, nil
}

func normalizeBaseURL(raw, kind string) string {
	raw = strings.TrimRight(strings.TrimSpace(raw), "/")
	suffixes := []string{"/chat/completions", "/completions", "/responses", "/messages", "/models"}
	for _, s := range suffixes {
		if strings.HasSuffix(raw, s) {
			raw = strings.TrimSuffix(raw, s)
		}
	}
	if kind == "ollama" {
		raw = strings.TrimSuffix(raw, "/api")
	}
	return raw
}

// List returns providers sorted by display name.
func (c Config) List() []ProviderConfig {
	out := make([]ProviderConfig, 0, len(c.Providers))
	for key, p := range c.Providers {
		if p.Name == "" {
			p.Name = key
		}
		if p.ID == "" {
			p.ID = key
		}
		out = append(out, p)
	}
	sort.Slice(out, func(i, j int) bool {
		return strings.ToLower(out[i].Name) < strings.ToLower(out[j].Name)
	})
	return out
}

// ProviderNames lists provider keys in stable order.
func (c Config) ProviderNames() []string {
	names := make([]string, 0, len(c.Providers))
	for name := range c.Providers {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// SaveDefault remembers the last selected provider for the TUI.
func SaveDefault(providerID string) error {
	dir, err := os.UserConfigDir()
	if err != nil {
		return err
	}
	dir = filepath.Join(dir, "basalt-tui")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	path := filepath.Join(dir, "config.json")
	doc := map[string]any{}
	if raw, err := os.ReadFile(path); err == nil {
		_ = json.Unmarshal(raw, &doc)
	}
	doc["default"] = providerID
	raw, err := json.MarshalIndent(doc, "", "  ")
	if err != nil {
		return err
	}
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, raw, 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

// Snippet renders a short redacted description for the UI.
func (p ProviderConfig) Snippet() string {
	key := "无 key"
	if p.APIKey != "" {
		key = "key " + p.APIKey[:min(4, len(p.APIKey))] + "…"
	}
	return fmt.Sprintf("%s · %s · %s", p.Name, p.Model, key)
}

// ParseTemp accepts the loose values typed in slash commands.
func ParseTemp(s string) (float64, bool) {
	v, err := strconv.ParseFloat(strings.TrimSpace(s), 64)
	if err != nil {
		return 0, false
	}
	return v, true
}

func expand(v string) string {
	v = strings.TrimSpace(v)
	if strings.HasPrefix(v, "${") && strings.HasSuffix(v, "}") {
		return os.Getenv(v[2 : len(v)-1])
	}
	if strings.HasPrefix(v, "$") {
		return os.Getenv(v[1:])
	}
	return v
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
