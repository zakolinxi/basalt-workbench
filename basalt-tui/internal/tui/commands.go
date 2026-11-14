package tui

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/juang/basalt-tui/internal/agent"
	"github.com/juang/basalt-tui/internal/config"
	"github.com/juang/basalt-tui/internal/llm"
	"github.com/juang/basalt-tui/internal/session"
)

func newSlashState() *slashState {
	s := &slashState{items: allCommands()}
	s.filter = s.items
	return s
}

func allCommands() []slashItem {
	return []slashItem{
		{"/help", "显示帮助与快捷键"},
		{"/model", "查看或切换 provider / 模型"},
		{"/new", "开始新会话"},
		{"/sessions", "列出历史会话"},
		{"/resume", "恢复历史会话"},
		{"/compact", "压缩上下文"},
		{"/undo", "撤销最近一次文件改动"},
		{"/clear", "清空当前屏幕"},
		{"/tools", "查看可用工具"},
		{"/cost", "查看 token 用量"},
		{"/init", "生成 BASALT.md 项目说明"},
		{"/export", "导出当前会话为 Markdown"},
		{"/quit", "退出 basalt"},
	}
}

func (m *Model) filterSlash() {
	q := strings.ToLower(strings.TrimSpace(m.slash.query))
	var out []slashItem
	for _, item := range m.slash.items {
		if q == "" || strings.Contains(strings.TrimPrefix(item.cmd, "/"), q) {
			out = append(out, item)
		}
	}
	m.slash.filter = out
	if m.slash.index >= len(out) {
		m.slash.index = max(0, len(out)-1)
	}
}

func (m *Model) handleSlashKey(msg tea.KeyMsg) (bool, tea.Cmd) {
	switch msg.String() {
	case "esc":
		m.slash.open = false
		m.refreshViewport(false)
		return true, nil
	case "up", "ctrl+p":
		if len(m.slash.filter) > 0 {
			m.slash.index--
			if m.slash.index < 0 {
				m.slash.index = len(m.slash.filter) - 1
			}
		}
		m.refreshViewport(false)
		return true, nil
	case "down", "ctrl+n":
		if len(m.slash.filter) > 0 {
			m.slash.index = (m.slash.index + 1) % len(m.slash.filter)
		}
		m.refreshViewport(false)
		return true, nil
	case "tab":
		if len(m.slash.filter) > 0 {
			cmd := m.slash.filter[m.slash.index].cmd
			m.input.SetValue(cmd + " ")
			m.slash.open = false
			m.layout()
			m.refreshViewport(false)
		}
		return true, nil
	case "enter":
		if len(m.slash.filter) > 0 {
			cmd := m.slash.filter[m.slash.index].cmd
			m.input.SetValue(cmd)
		}
	}
	return false, nil
}

func (m *Model) runSlash(text string) tea.Cmd {
	fields := strings.Fields(text)
	cmd := fields[0]
	args := fields[1:]
	m.slash.open = false

	notice := func(format string, a ...any) tea.Cmd {
		m.items = append(m.items, Item{Kind: ItemNotice, Text: fmt.Sprintf(format, a...)})
		m.refreshViewport(true)
		return nil
	}

	switch cmd {
	case "/quit", "/exit", "/q":
		m.quitting = true
		return tea.Quit
	case "/help", "/?":
		m.showHelp = true
		m.refreshViewport(false)
		return nil
	case "/clear":
		m.items = nil
		m.refreshViewport(false)
		return nil
	case "/model", "/provider":
		if len(args) == 0 {
			m.openModelPicker()
			return nil
		}
		if len(args) >= 2 {
			return m.switchProvider(args[0], args[1])
		}
		return m.switchProvider(args[0])
	case "/new":
		if err := m.store.Save(m.sess); err != nil {
			return notice("保存旧会话失败：%v", err)
		}
		m.resetSession()
		return notice("已开始新会话")
	case "/sessions":
		list := m.store.List()
		if len(list) == 0 {
			return notice("还没有历史会话")
		}
		var b strings.Builder
		b.WriteString("最近的会话（/resume <id> 恢复）：\n")
		limit := min(10, len(list))
		for _, s := range list[:limit] {
			mark := "  "
			if s.ID == m.sess.ID {
				mark = "● "
			}
			fmt.Fprintf(&b, "%s%s  %-34s %2d 条  %s\n", mark, s.ID, truncateWidth(s.Title, 34), s.Messages, s.UpdatedAt.Format("01-02 15:04"))
		}
		return notice("%s", strings.TrimRight(b.String(), "\n"))
	case "/resume":
		if len(args) == 0 {
			return notice("用法：/resume <session-id>，先用 /sessions 查看")
		}
		return m.resume(args[0])
	case "/compact":
		if m.streaming {
			return notice("正在生成，稍后再压缩")
		}
		dropped := m.ag.Compact()
		if dropped == 0 {
			return notice("上下文还很短，无需压缩")
		}
		return notice("已压缩 %d 条旧消息", dropped)
	case "/undo":
		snap, ok := m.env.PopSnapshot()
		if !ok {
			return notice("没有可撤销的文件改动")
		}
		rel := m.env.Rel(snap.Path)
		if snap.Existed {
			if err := os.WriteFile(snap.Path, []byte(snap.Content), 0o644); err != nil {
				return notice("撤销失败：%v", err)
			}
		} else {
			_ = os.Remove(snap.Path)
		}
		return notice("已撤销 %s", rel)
	case "/tools":
		var b strings.Builder
		b.WriteString("可用工具：\n")
		for _, name := range m.reg.Names() {
			t, ok := m.reg.Get(name)
			if !ok {
				continue
			}
			kind := "需审批"
			if t.ReadOnly {
				kind = "只读"
			}
			fmt.Fprintf(&b, "  %-12s %-6s %s\n", name, kind, truncateWidth(t.Description, 72))
		}
		return notice("%s", strings.TrimRight(b.String(), "\n"))
	case "/cost":
		u := m.sess.Usage
		return notice("用量：请求 %d · 工具 %d · 输入 %s · 输出 %s · 缓存 %s",
			u.Requests, u.ToolCalls, compactNumber(u.Input), compactNumber(u.Output), compactNumber(u.Cached))
	case "/init":
		return m.initProject()
	case "/export":
		return m.exportSession()
	}
	return notice("未知命令 %s，输入 /help 查看全部命令", cmd)
}

func (m *Model) switchProvider(name string, model ...string) tea.Cmd {
	key, _, ok := m.cfg.Find(name)
	if !ok {
		m.items = append(m.items, Item{Kind: ItemError, Text: "找不到供应商 " + name + "，用 /model 查看列表"})
		m.refreshViewport(true)
		return nil
	}
	preset, err := m.cfg.Resolve(key)
	if err != nil {
		m.items = append(m.items, Item{Kind: ItemError, Text: err.Error()})
		m.refreshViewport(true)
		return nil
	}
	if len(model) > 0 && strings.TrimSpace(model[0]) != "" {
		preset.Model = strings.TrimSpace(model[0])
	}
	client, err := llm.New(preset)
	if err != nil {
		m.items = append(m.items, Item{Kind: ItemError, Text: err.Error()})
		m.refreshViewport(true)
		return nil
	}
	if m.sess.Usage.Requests > 0 {
		_ = m.store.Save(m.sess)
	}
	m.providerKey = key
	m.preset = preset
	m.client = client
	m.sess.Provider = key
	m.sess.Model = preset.Model
	m.ag.Client = client
	m.ag.MaxTokens = preset.MaxTokens
	m.ag.Temp = preset.Temperature
	m.ag.Reasoning = preset.Reasoning
	// Remember the choice for the next launch without copying the shared
	// provider registry into the TUI config file.
	if !m.demo {
		_ = config.SaveDefault(key)
	}
	source := preset.Source
	if source == "" {
		source = "builtin"
	}
	m.items = append(m.items, Item{Kind: ItemNotice,
		Text: fmt.Sprintf("已切换到 %s · %s（%s，%s）", preset.Name, preset.Model, preset.Kind, source)})
	m.refreshViewport(true)
	return nil
}

func (m *Model) resetSession() {
	sess := session.New(m.cwd, m.providerKey, m.preset.Model)
	m.sess = sess
	m.env.SessionID = sess.ID
	m.ag.Session = sess
	m.items = nil
	m.usage = session.Usage{}
	m.historyIdx = 0
	m.refreshViewport(false)
}

func (m *Model) resume(id string) tea.Cmd {
	sess, err := m.store.Load(id)
	if err != nil {
		m.items = append(m.items, Item{Kind: ItemError, Text: "无法恢复会话：" + err.Error()})
		m.refreshViewport(true)
		return nil
	}
	m.sess = sess
	m.ag.Session = sess
	m.env.SessionID = sess.ID
	m.usage = sess.Usage
	if preset, err := m.cfg.Resolve(sess.Provider); err == nil {
		if client, err := llm.New(preset); err == nil {
			key, _, _ := m.cfg.Find(sess.Provider)
			m.providerKey = key
			m.preset = preset
			m.client = client
			m.ag.Client = client
			m.ag.MaxTokens = preset.MaxTokens
			m.ag.Temp = preset.Temperature
		}
	}
	m.items = itemsFromSession(sess)
	m.refreshViewport(true)
	m.items = append(m.items, Item{Kind: ItemNotice, Text: "已恢复会话 " + sess.ID})
	m.refreshViewport(true)
	return nil
}

func itemsFromSession(sess *session.Session) []Item {
	var items []Item
	for _, msg := range sess.Messages {
		switch msg.Role {
		case "user":
			items = append(items, Item{Kind: ItemUser, Text: msg.Content})
		case "assistant":
			if strings.TrimSpace(msg.Content) != "" {
				items = append(items, Item{Kind: ItemAssistant, Text: msg.Content, Model: sess.Model})
			}
			for _, call := range msg.ToolCalls {
				items = append(items, Item{Kind: ItemTool, Tool: &agent.ToolEvent{
					ID: call.ID, Name: call.Name, Title: truncateWidth(call.Arguments, 64),
					ReadOnly: true,
				}})
			}
		}
	}
	return items
}

func (m *Model) initProject() tea.Cmd {
	path := filepath.Join(m.cwd, "BASALT.md")
	if _, err := os.Stat(path); err == nil {
		m.items = append(m.items, Item{Kind: ItemNotice, Text: "BASALT.md 已存在，未覆盖。"})
		m.refreshViewport(true)
		return nil
	}
	var dirs []string
	entries, _ := os.ReadDir(m.cwd)
	for _, e := range entries {
		if e.IsDir() && !strings.HasPrefix(e.Name(), ".") && e.Name() != "node_modules" && e.Name() != "target" {
			dirs = append(dirs, e.Name())
		}
	}
	sort.Strings(dirs)
	content := fmt.Sprintf(`# BASALT.md

本文件为 Basalt 终端代理提供项目上下文。每次会话会自动读取。

## 项目概览

- 工作区：%s
- 主要目录：%s

## 构建与测试

<!-- 补充真实命令，例如： -->
<!-- - 构建：go build ./... -->
<!-- - 测试：go test ./... -->

## 约定

- 修改代码前先阅读相关文件。
- 保持现有代码风格与命名。
- 完成后运行测试并说明结果。
`, m.relativeCwd(), strings.Join(dirs, "、"))
	m.env.Remember(path)
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		m.items = append(m.items, Item{Kind: ItemError, Text: "写入 BASALT.md 失败：" + err.Error()})
		m.refreshViewport(true)
		return nil
	}
	m.items = append(m.items, Item{Kind: ItemNotice, Text: "已生成 BASALT.md，请补充构建/测试命令。"})
	m.refreshViewport(true)
	// Refresh the system prompt so the new file is picked up immediately.
	m.ag.System = agent.SystemPrompt(m.cwd, m.reg)
	return nil
}

func (m *Model) exportSession() tea.Cmd {
	name := "basalt-" + m.sess.ID + ".md"
	path := filepath.Join(m.cwd, name)
	if err := os.WriteFile(path, []byte(m.sess.Transcript()), 0o644); err != nil {
		m.items = append(m.items, Item{Kind: ItemError, Text: "导出失败：" + err.Error()})
		m.refreshViewport(true)
		return nil
	}
	m.items = append(m.items, Item{Kind: ItemNotice, Text: "已导出到 " + name})
	m.refreshViewport(true)
	return nil
}
