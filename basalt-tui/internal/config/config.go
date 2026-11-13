// Package config loads and resolves Basalt TUI configuration.
//
// Configuration is searched in this order (first hit wins):
//
//	$BASALT_TUI_CONFIG
//	<workspace>/.basalt/config.json
//	<UserConfigDir>/basalt-tui/config.json
//
// Missing fields fall back to built-in provider presets. API keys keep the
// ${ENV_VAR} form supporting shell-style expansion, and empty keys are filled
// from well-known environment variables so `basalt` works with no config file.
package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
)

// ProviderConfig describes one chat-completions endpoint.
type ProviderConfig struct {
	Kind        string  `json:"kind"` // openai | anthropic | ollama
	Model       string  `json:"model"`
	BaseURL     string  `json:"baseUrl,omitempty"`
	APIKey      string  `json:"apiKey,omitempty"`
	MaxTokens   int     `json:"maxTokens,omitempty"`
	Temperature float64 `json:"temperature,omitempty"`
	Reasoning   string  `json:"reasoning,omitempty"` // low | medium | high (OpenAI reasoning models)
}

// Config is the on-disk configuration document.
type Config struct {
	Default   string                    `json:"default"`
	Providers map[string]ProviderConfig `json:"providers"`
	Theme     string                    `json:"theme,omitempty"`
	// AutoApprove applies to read-only tools. Mutating tools still ask.
	AutoApprove bool `json:"autoApproveRead"`
	// CompactAfter is the message-count ceiling before auto-compaction kicks in.
	CompactAfter int      `json:"compactAfter,omitempty"`
	EnvKeys      []string `json:"envKeys,omitempty"`
}

// Path is the config path that was actually used, empty for built-ins.
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
}

func defaults() Config {
	return Config{
		Default: "deepseek",
		Providers: map[string]ProviderConfig{
			"deepseek": {
				Kind: "openai", BaseURL: "https://api.deepseek.com/v1",
				Model: "deepseek-chat", MaxTokens: 8192,
			},
			"openai": {
				Kind: "openai", BaseURL: "https://api.openai.com/v1",
				Model: "gpt-4.1", MaxTokens: 8192,
			},
			"anthropic": {
				Kind: "anthropic", BaseURL: "https://api.anthropic.com",
				Model: "claude-sonnet-4-5", MaxTokens: 8192,
			},
			"moonshot": {
				Kind: "openai", BaseURL: "https://api.moonshot.cn/v1",
				Model: "kimi-k2-0905-preview", MaxTokens: 8192,
			},
			"zhipu": {
				Kind: "openai", BaseURL: "https://open.bigmodel.cn/api/paas/v4",
				Model: "glm-4.6", MaxTokens: 8192,
			},
			"ollama": {
				Kind: "ollama", BaseURL: "http://localhost:11434",
				Model: "qwen2.5-coder:7b", MaxTokens: 8192,
			},
			"lmstudio": {
				Kind: "openai", BaseURL: "http://localhost:1234/v1",
				Model: "local-model", MaxTokens: 8192,
			},
		},
		Theme:        "basalt",
		AutoApprove:  true,
		CompactAfter: 48,
	}
}

// Load reads the first config file found, or built-in defaults.
func Load(workspace string) Loaded {
	candidates := []string{}
	if p := os.Getenv("BASALT_TUI_CONFIG"); p != "" {
		candidates = append(candidates, p)
	}
	if workspace != "" {
		candidates = append(candidates, filepath.Join(workspace, ".basalt", "config.json"))
	}
	if dir, err := os.UserConfigDir(); err == nil {
		candidates = append(candidates, filepath.Join(dir, "basalt-tui", "config.json"))
	}
	cfg := defaults()
	for _, p := range candidates {
		raw, err := os.ReadFile(p)
		if err != nil {
			continue
		}
		// Unmarshal over defaults so partial files only override what they set.
		merged := cfg
		merged.Providers = map[string]ProviderConfig{}
		for k, v := range cfg.Providers {
			merged.Providers[k] = v
		}
		if err := json.Unmarshal(raw, &merged); err != nil {
			continue
		}
		if merged.Default == "" {
			merged.Default = cfg.Default
		}
		return Loaded{Config: merged, Path: p}
	}
	return Loaded{Config: cfg}
}

// Resolve returns the provider config with env expansion applied.
func (c Config) Resolve(name string) (ProviderConfig, error) {
	if name == "" {
		name = c.Default
	}
	p, ok := c.Providers[name]
	if !ok {
		if len(c.Providers) == 1 {
			for _, only := range c.Providers {
				p, ok = only, true
			}
		}
		if !ok {
			return ProviderConfig{}, fmt.Errorf("unknown provider %q", name)
		}
	}
	p.APIKey = expand(p.APIKey)
	if p.APIKey == "" {
		if env := envByProvider[name]; env != "" {
			p.APIKey = os.Getenv(env)
		}
	}
	if p.BaseURL == "" {
		switch p.Kind {
		case "anthropic":
			p.BaseURL = "https://api.anthropic.com"
		case "ollama":
			p.BaseURL = "http://localhost:11434"
		default:
			p.BaseURL = "https://api.openai.com/v1"
		}
	}
	p.BaseURL = strings.TrimRight(p.BaseURL, "/")
	if p.MaxTokens <= 0 {
		p.MaxTokens = 8192
	}
	return p, nil
}

// ProviderNames lists configured providers in stable order.
func (c Config) ProviderNames() []string {
	names := make([]string, 0, len(c.Providers))
	for name := range c.Providers {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// Save writes the config to <UserConfigDir>/basalt-tui/config.json.
func (c Config) Save() (string, error) {
	dir, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	dir = filepath.Join(dir, "basalt-tui")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	path := filepath.Join(dir, "config.json")
	raw, err := json.MarshalIndent(c, "", "  ")
	if err != nil {
		return "", err
	}
	return path, os.WriteFile(path, raw, 0o600)
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

// Snippet renders a short redacted description for the UI.
func (p ProviderConfig) Snippet() string {
	key := "no key"
	if p.APIKey != "" {
		key = "key " + p.APIKey[:min(4, len(p.APIKey))] + "…"
	}
	return fmt.Sprintf("%s · %s · %s", p.Kind, p.Model, key)
}

// ParseTemp accepts the loose values typed in slash commands.
func ParseTemp(s string) (float64, bool) {
	v, err := strconv.ParseFloat(strings.TrimSpace(s), 64)
	if err != nil {
		return 0, false
	}
	return v, true
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
