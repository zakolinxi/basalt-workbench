package tui

import (
	"fmt"
	"time"

	"github.com/juang/basalt-tui/internal/agent"
	"github.com/juang/basalt-tui/internal/config"
	"github.com/juang/basalt-tui/internal/session"
)

// Scene is one screenshot-ready frame.
type Scene struct {
	Name  string
	Title string
	Model *Model
}

func demoTime(h, m int) time.Time {
	return time.Date(2026, 11, 13, h, m, 0, 0, time.Local)
}

const demoUserPrompt = "帮我给登录接口加上失败次数限制：连续 5 次失败后锁定 15 分钟，并在 tests 里补一个用例。"

var demoReadOutput = `    1│ import { db } from "../db";
    2│ import { verifyPassword } from "./crypto";
    3│
    4│ export async function login(email: string, password: string) {
    5│   const user = await db.user.findUnique({ where: { email } });
    6│   if (!user || !(await verifyPassword(password, user.hash))) {
    7│     return { ok: false, error: "invalid_credentials" };
    8│   }
    9│   return { ok: true, token: signToken(user) };
   10│ }`

var demoAuthDiff = `--- a/src/server/auth.ts
+++ b/src/server/auth.ts
@@ -1,4 +1,14 @@
 import { db } from "../db";
 import { verifyPassword } from "./crypto";
 
+const MAX_ATTEMPTS = 5;
+const LOCK_MS = 15 * 60 * 1000;
+const failures = new Map<string, { count: number; until: number }>();
+
+function recordFailure(email: string) {
+  const entry = failures.get(email) ?? { count: 0, until: 0 };
+  entry.count += 1;
+  if (entry.count >= MAX_ATTEMPTS) entry.until = Date.now() + LOCK_MS;
+  failures.set(email, entry);
+  return entry;
+}
+
 export async function login(email: string, password: string) {`

var demoTestDiff = `--- a/tests/auth.test.ts
+++ b/tests/auth.test.ts
@@ -18,6 +18,24 @@ describe("login", () => {
   it("rejects a wrong password", async () => {
     expect(await login("ada@example.com", "nope")).toEqual({ ok: false, error: "invalid_credentials" });
   });
+
+  it("locks the account after five failures", async () => {
+    for (let i = 0; i < 5; i++) await login("ada@example.com", "nope");
+    expect(await login("ada@example.com", "secret")).toEqual({ ok: false, error: "locked_out" });
+  });
+
+  it("unlocks after the lock window", async () => {
+    vi.advanceTimersByTime(15 * 60 * 1000);
+    expect((await login("ada@example.com", "secret")).ok).toBe(true);
+  });
 });`

var demoTestOutput = ` ✓ tests/auth.test.ts (4 tests) 214ms
   ✓ login accepts valid credentials
   ✓ rejects a wrong password
   ✓ locks the account after five failures
   ✓ unlocks after the lock window

 Test Files  1 passed (1)
      Tests  4 passed (4)
   Duration  1.21s`

const demoSummary = "改完了，两个文件：\n\n" +
	"**`src/server/auth.ts`** — 新增 `MAX_ATTEMPTS` / `LOCK_MS`，失败 5 次后返回 `locked_out`，15 分钟后自动解锁。\n" +
	"**`tests/auth.test.ts`** — 新增 3 个用例：第 5 次失败锁定、锁定期间拒绝正确密码、过期后恢复。\n\n" +
	"验证：`pnpm vitest run tests/auth.test.ts` → **4 passed**。\n\n" +
	"> 计数器目前是进程内 Map，多实例部署时需要换成 Redis。已留 TODO，不在此次范围内。\n\n" +
	"要我把锁定时长改成可配置项吗？"

// DemoScenes builds the frames used by --snapshot.
func DemoScenes(width, height int, workspace string) ([]Scene, error) {
	loaded := config.Load(workspace)
	scenes := []struct {
		name  string
		build func() (*Model, error)
	}{
		{"01-welcome", func() (*Model, error) {
			m, err := New(workspace, loaded, true)
			if err != nil {
				return nil, err
			}
			useDemoProvider(m)
			m.prepare(width, height)
			return m, nil
		}},
		{"02-conversation", func() (*Model, error) {
			m, err := New(workspace, loaded, true)
			if err != nil {
				return nil, err
			}
			useDemoProvider(m)
			m.sess.Usage = session.Usage{Input: 8420, Output: 864, Requests: 2}
			m.ctxTokens = 8420
			m.cacheTokens = 2048
			m.lastTPS = 48.6
			m.items = []Item{
				{Kind: ItemUser, Text: demoUserPrompt, At: demoTime(14, 31)},
				{Kind: ItemThinking, At: demoTime(14, 31), Text: "先看登录实现和现有的测试结构，再决定限流放在路由层还是服务层。\n现有 login 只校验密码，没有失败计数，适合在服务层加计数与锁定窗口。"},
				{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 31), Text: "先看登录实现和现有的测试结构。\n\n登录逻辑现在只校验密码，没有失败计数。计划如下：\n\n1. 在 `auth.ts` 中加入失败计数器与锁定窗口\n2. 抽出 `recordFailure` 便于测试\n3. 在 `tests/auth.test.ts` 覆盖第 5 次失败与 15 分钟后解锁\n\n先读取文件确认当前实现。"},
				{Kind: ItemTool, At: demoTime(14, 31), Tool: &agent.ToolEvent{
					ID: "c1", Name: "read", Title: "read src/server/auth.ts  L1-10", ReadOnly: true,
					Elapsed: 32 * time.Millisecond, Output: demoReadOutput,
				}},
				{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 32), Text: "现状确认：`login` 没有任何限流。现在开始改 `auth.ts`，然后补测试。"},
			}
			m.prepare(width, height)
			return m, nil
		}},
		{"03-approval", func() (*Model, error) {
			m, err := New(workspace, loaded, true)
			if err != nil {
				return nil, err
			}
			useDemoProvider(m)
			m.sess.Usage = session.Usage{Input: 8420, Output: 1204, Requests: 3}
			m.ctxTokens = 8420
			m.cacheTokens = 1204
			m.lastTPS = 52.1
			m.items = []Item{
				{Kind: ItemUser, Text: demoUserPrompt, At: demoTime(14, 31)},
				{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 31), Text: "先看登录实现和现有的测试结构。登录逻辑现在只校验密码，没有失败计数，我准备在 `src/server/auth.ts` 中加计数与锁定窗口。"},
				{Kind: ItemTool, At: demoTime(14, 31), Tool: &agent.ToolEvent{
					ID: "c1", Name: "read", Title: "read src/server/auth.ts  L1-10", ReadOnly: true,
					Elapsed: 32 * time.Millisecond, Output: demoReadOutput,
				}},
				{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 32), Text: "已确认现状。下面修改 `auth.ts`：加入 `MAX_ATTEMPTS` 和 15 分钟锁定窗口。"},
			}
			m.prepare(width, height)
			m.approval = &approvalState{
				req: agent.Approval{
					ID: "c2", Tool: "edit", Title: "src/server/auth.ts  +10",
					Preview: demoAuthDiff,
				},
				reply: make(chan agent.Decision, 1),
			}
			m.refreshViewport(false)
			return m, nil
		}},
		{"04-done", func() (*Model, error) {
			m, err := New(workspace, loaded, true)
			if err != nil {
				return nil, err
			}
			useDemoProvider(m)
			m.sess.Usage = session.Usage{Input: 12840, Output: 1836, Requests: 4, ToolCalls: 4}
			m.ctxTokens = 12840
			m.cacheTokens = 6144
			m.lastTPS = 36.4
			m.items = demoDoneItems()
			m.prepare(width, height)
			return m, nil
		}},
		{"05-commands", func() (*Model, error) {
			m, err := New(workspace, loaded, true)
			if err != nil {
				return nil, err
			}
			useDemoProvider(m)
			m.sess.Usage = session.Usage{Input: 8420, Output: 864, Requests: 2}
			m.ctxTokens = 8420
			m.cacheTokens = 2048
			m.lastTPS = 48.6
			m.items = []Item{
				{Kind: ItemUser, Text: "把 auth 的失败计数改成可配置的", At: demoTime(14, 41)},
				{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 42), Text: "可以。`MAX_ATTEMPTS` 和 `LOCK_MS` 目前是常量，我建议抽到 `src/config.ts`，用环境变量覆盖，保持现有默认值不变。\n\n需要我先看一下 `config.ts` 的现有结构吗？"},
			}
			m.prepare(width, height)
			m.input.SetValue("/")
			m.input.CursorEnd()
			m.slash.open = true
			m.slash.query = ""
			m.filterSlash()
			m.layout()
			m.refreshViewport(false)
			return m, nil
		}},
		{"06-models", func() (*Model, error) {
			m, err := New(workspace, loaded, true)
			if err != nil {
				return nil, err
			}
			useDemoProvider(m)
			m.cfg.Providers = demoProviders()
			m.cfg.Default = "demo-deepseek"
			m.cfg.Source = "basalt"
			m.sess.Usage = session.Usage{Input: 8420, Output: 864, Requests: 2}
			m.ctxTokens = 8420
			m.cacheTokens = 2048
			m.lastTPS = 48.6
			m.items = []Item{
				{Kind: ItemUser, Text: "这个任务换个便宜点的模型试试", At: demoTime(14, 46)},
				{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 46), Text: "好，按 `/model` 打开选择器，方向键选、Enter 切换。"},
			}
			m.prepare(width, height)
			m.picker = newModelPicker(m.cfg, "demo-deepseek", "deepseek-chat")
			m.refreshViewport(false)
			return m, nil
		}},
		{"07-web", func() (*Model, error) {
			m, err := New(workspace, loaded, true)
			if err != nil {
				return nil, err
			}
			useDemoProvider(m)
			m.sess.Usage = session.Usage{Input: 6240, Output: 512, Requests: 2, ToolCalls: 1}
			m.ctxTokens = 6240
			m.cacheTokens = 1870
			m.lastTPS = 41.2
			m.items = []Item{
				{Kind: ItemUser, Text: "查一下 opencode 现在的维护状态，顺便说下它的终端交互有什么值得借鉴的", At: demoTime(14, 52)},
				{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 52), Text: "我搜一下最新信息，再抓取第一条正文确认。"},
				{Kind: ItemTool, At: demoTime(14, 52), Tool: &agent.ToolEvent{
					ID: "w1", Name: "websearch", ReadOnly: true,
					Elapsed: 920 * time.Millisecond,
					Title:   "websearch \"opencode terminal agent\" · 3 个结果 · tavily",
					Output:  demoSearchOutput,
				}},
				{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 53), Text: "查到三条结果。`opencode-ai/opencode` 主仓库已归档，项目迁移到了 Crush；它的 TUI 交互仍然值得借鉴：\n\n- `@` 引用文件、`/` 命令面板、Tab 补全\n- 模型与 provider 快速切换\n- 工具调用前展示结果和差异\n\n搜索走 Tavily，正文抓取会自动提取主内容并转成 Markdown。"},
			}
			m.prepare(width, height)
			return m, nil
		}},
	}

	out := make([]Scene, 0, len(scenes))
	for _, s := range scenes {
		model, err := s.build()
		if err != nil {
			return nil, err
		}
		title := fmt.Sprintf("basalt — %s — %d×%d", model.relativeCwd(), width, height)
		out = append(out, Scene{Name: s.name, Title: title, Model: model})
	}
	return out, nil
}

func (m *Model) prepare(width, height int) {
	m.width = width
	m.height = height
	m.layout()
	m.ready = true
	m.refreshViewport(false)
}

// RenderFrame returns the plain ANSI frame for a scene.
func (s Scene) RenderFrame() string {
	return s.Model.View()
}

// useDemoProvider pins the demo scenes to a stable provider so screenshots do
// not depend on what is configured in the workbench database.
func useDemoProvider(m *Model) {
	const id = "demo-deepseek"
	m.providerKey = id
	m.preset = config.ProviderConfig{
		ID: id, Name: "deepseek", Kind: "openai_chat", Model: "deepseek-chat",
		BaseURL: "https://api.deepseek.com/v1", MaxTokens: 8192,
		ContextLength: 131072, Source: "builtin",
	}
	m.sess.Provider = id
	m.sess.Model = "deepseek-chat"
	m.cfg.Source = "builtin"
	m.cfgPath = ""
}

var demoSearchOutput = `搜索：opencode terminal agent（tavily，3 个结果）

1. GitHub - opencode-ai/opencode: A powerful AI coding agent. Built for the terminal.
   https://github.com/opencode-ai/opencode
   Archived: this repository has moved and is kept for provenance.

2. OpenCode: Revolutionizing Terminal Coding Agent - Webkul Blog
   https://webkul.com/blog/opencode-terminal-coding-agent
   OpenCode can act on its own but stays safe with checks.

3. opencode-ai/opencode releases
   https://github.com/opencode-ai/opencode/releases
   Latest release notes and version history.`

func demoProviders() map[string]config.ProviderConfig {
	return map[string]config.ProviderConfig{
		"demo-deepseek": {
			ID: "demo-deepseek", Name: "DeepSeek", Kind: "openai_chat",
			Model: "deepseek-chat", Models: []string{"deepseek-chat", "deepseek-reasoner"},
			APIKey: "demo", Source: "basalt",
		},
		"demo-anthropic": {
			ID: "demo-anthropic", Name: "Anthropic Claude", Kind: "anthropic",
			Model: "claude-sonnet-4-5", Models: []string{"claude-sonnet-4-5", "claude-opus-4-5"},
			APIKey: "demo", Source: "basalt",
		},
		"demo-openai": {
			ID: "demo-openai", Name: "OpenAI", Kind: "openai_responses",
			Model: "gpt-4.1", Models: []string{"gpt-4.1", "o3"},
			APIKey: "demo", Source: "basalt",
		},
		"demo-gemini": {
			ID: "demo-gemini", Name: "Gemini", Kind: "gemini",
			Model: "gemini-2.5-pro", Models: []string{"gemini-2.5-pro", "gemini-2.5-flash"},
			APIKey: "demo", Source: "basalt",
		},
		"demo-ollama": {
			ID: "demo-ollama", Name: "Ollama", Kind: "ollama",
			Model: "qwen2.5-coder:7b", Models: []string{"qwen2.5-coder:7b"},
			Source: "basalt",
		},
	}
}

func demoDoneItems() []Item {
	return []Item{
		{Kind: ItemUser, Text: demoUserPrompt, At: demoTime(14, 31)},
		{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 32), Text: "已确认现状，开始改造。先加计数与锁定窗口，再补测试。"},
		{Kind: ItemTool, At: demoTime(14, 32), Tool: &agent.ToolEvent{
			ID: "c1", Name: "edit", Title: "edit src/server/auth.ts  +10 -0",
			ReadOnly: false, Elapsed: 21 * time.Millisecond, Added: 10, Diff: demoAuthDiff,
		}},
		{Kind: ItemTool, At: demoTime(14, 33), Tool: &agent.ToolEvent{
			ID: "c2", Name: "edit", Title: "edit tests/auth.test.ts  +18 -0",
			ReadOnly: false, Elapsed: 18 * time.Millisecond, Added: 18, Diff: demoTestDiff,
		}},
		{Kind: ItemTool, At: demoTime(14, 33), Tool: &agent.ToolEvent{
			ID: "c3", Name: "bash", Title: "✓ bash pnpm vitest run tests/auth.test.ts",
			ReadOnly: false, Elapsed: 1214 * time.Millisecond, Output: demoTestOutput,
		}},
		{Kind: ItemAssistant, Model: "deepseek-chat", At: demoTime(14, 34), Text: demoSummary},
	}
}

// SeedDemo loads the completed demo conversation so `basalt --demo` can be
// explored without an API key.
func (m *Model) SeedDemo() {
	useDemoProvider(m)
	m.sess.Usage = session.Usage{Input: 12840, Output: 1836, Requests: 4, ToolCalls: 4}
	m.usage = m.sess.Usage
	m.items = demoDoneItems()
	m.items = append(m.items, Item{Kind: ItemNotice, Text: "演示模式：以上为预置对话。可以直接提问（需配置 API Key），或 /new 开始空白会话。"})
	m.refreshViewport(false)
}
