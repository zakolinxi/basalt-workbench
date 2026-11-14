package tui

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textarea"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/juang/basalt-tui/internal/agent"
	"github.com/juang/basalt-tui/internal/config"
	"github.com/juang/basalt-tui/internal/llm"
	"github.com/juang/basalt-tui/internal/session"
	"github.com/juang/basalt-tui/internal/tools"
)

// Version is stamped at build time via -ldflags when packaging.
var Version = "0.2.0"

// Model is the bubbletea application state.
type Model struct {
	cfg         config.Config
	cfgPath     string
	providerKey string
	preset      config.ProviderConfig
	client      llm.Client
	ag          *agent.Agent
	sess        *session.Session
	store       *session.Store
	env         *tools.Env
	reg         *tools.Registry
	program     *tea.Program

	width  int
	height int
	ready  bool
	cwd    string
	home   string

	viewport viewport.Model
	input    textarea.Model
	spinner  spinner.Model

	items       []Item
	events      chan agent.Event
	streamIdx   int
	thinkingIdx int
	plan        []tools.PlanStep
	history     []string
	historyIdx  int
	draft       string
	autoScroll  bool
	streaming   bool
	cancel      context.CancelFunc
	runStarted  time.Time
	lastEventAt time.Time
	streamChars int

	approval *approvalState
	picker   *pickerState
	slash    *slashState
	notice   string
	noticeAt time.Time

	usage    session.Usage
	contextN int

	showHelp bool
	quitting bool
	demo     bool
}

type approvalState struct {
	req   agent.Approval
	reply chan agent.Decision
}

type slashState struct {
	open   bool
	query  string
	index  int
	items  []slashItem
	filter []slashItem
}

type slashItem struct {
	cmd  string
	desc string
}

// agentEventMsg wraps one agent event for the bubbletea loop.
type agentEventMsg agent.Event

// approvalMsg asks the UI to show a confirmation card.
type approvalMsg struct {
	req   agent.Approval
	reply chan agent.Decision
}

// runDoneMsg signals the agent goroutine finished.
type runDoneMsg struct {
	err error
}

// savedMsg reports an async session save.
type savedMsg struct{ err error }

// New builds the application model.
func New(workspace string, loaded config.Loaded, demo bool) (*Model, error) {
	cwd, _ := filepath.Abs(workspace)
	home, _ := os.UserHomeDir()
	dataDir := filepath.Join(dataRoot(), "basalt-tui")
	_ = os.MkdirAll(dataDir, 0o755)
	store, err := session.NewStore(filepath.Join(dataDir, "sessions"))
	if err != nil {
		return nil, err
	}

	providerKey := loaded.Config.Default
	preset, err := loaded.Config.Resolve(providerKey)
	if err != nil {
		return nil, err
	}
	client, err := llm.New(preset)
	if err != nil {
		return nil, err
	}
	sessionID := session.New(cwd, providerKey, preset.Model)
	env := tools.NewEnv(cwd, sessionID.ID, dataDir)
	reg := tools.NewRegistry()
	ag := agent.New(client, reg, env, sessionID)
	ag.MaxTokens = preset.MaxTokens
	ag.Temp = preset.Temperature
	ag.Reasoning = preset.Reasoning
	ag.AutoApproveRead = loaded.Config.AutoApprove
	if loaded.Config.CompactAfter > 0 {
		ag.CompactAfter = loaded.Config.CompactAfter
	}
	ag.System = agent.SystemPrompt(cwd, reg)

	ta := textarea.New()
	ta.Placeholder = "输入消息，Enter 发送，Ctrl+J 换行"
	ta.Prompt = "› "
	ta.FocusedStyle.Prompt = styleInputPrompt
	ta.BlurredStyle.Prompt = styleInputPrompt
	ta.FocusedStyle.Placeholder = stylePlaceholder
	ta.BlurredStyle.Placeholder = stylePlaceholder
	ta.CharLimit = 0
	ta.ShowLineNumbers = false
	ta.KeyMap.InsertNewline.SetKeys("ctrl+j")
	ta.Focus()

	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(lipgloss.Color(cClay))

	m := &Model{
		cfg:         loaded.Config,
		cfgPath:     loaded.Path,
		providerKey: providerKey,
		preset:      preset,
		client:      client,
		ag:          ag,
		sess:        sessionID,
		store:       store,
		env:         env,
		reg:         reg,
		cwd:         cwd,
		home:        home,
		viewport:    viewport.New(80, 20),
		input:       ta,
		spinner:     sp,
		autoScroll:  true,
		streamIdx:   -1,
		thinkingIdx: -1,
		demo:        demo,
		slash:       newSlashState(),
	}
	m.usage = sessionID.Usage
	return m, nil
}

// SetProgram is required by the approver to send UI messages.
func (m *Model) SetProgram(p *tea.Program) { m.program = p }

func dataRoot() string {
	if dir, err := os.UserConfigDir(); err == nil {
		return dir
	}
	if dir, err := os.UserHomeDir(); err == nil {
		return filepath.Join(dir, ".config")
	}
	return "."
}

// Approve implements agent.Approver.
func (m *Model) Approve(ctx context.Context, req agent.Approval) agent.Decision {
	if m.program == nil {
		return agent.DecisionOnce
	}
	reply := make(chan agent.Decision, 1)
	m.program.Send(approvalMsg{req: req, reply: reply})
	select {
	case d := <-reply:
		return d
	case <-ctx.Done():
		return agent.DecisionDeny
	}
}

// Init implements tea.Model.
func (m *Model) Init() tea.Cmd {
	return tea.Batch(textarea.Blink, m.spinner.Tick)
}

// Update implements tea.Model.
func (m *Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width, m.height = msg.Width, msg.Height
		m.layout()
		m.ready = true
		m.refreshViewport(false)
		return m, nil

	case tea.KeyMsg:
		return m.handleKey(msg)

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		if m.streaming {
			return m, cmd
		}
		return m, nil

	case approvalMsg:
		m.approval = &approvalState{req: msg.req, reply: msg.reply}
		m.refreshViewport(false)
		return m, nil

	case agentEventMsg:
		return m, m.handleAgentEvent(agent.Event(msg))

	case runDoneMsg:
		m.streaming = false
		m.cancel = nil
		m.streamIdx = -1
		m.thinkingIdx = -1
		m.usage = m.sess.Usage
		if msg.err != nil && msg.err != context.Canceled {
			m.items = append(m.items, Item{Kind: ItemError, Text: msg.err.Error(), At: time.Now()})
		}
		m.refreshViewport(false)
		cmd := m.saveCmd()
		return m, cmd

	case savedMsg:
		return m, nil
	}

	var cmd tea.Cmd
	m.input, cmd = m.input.Update(msg)
	return m, cmd
}

func (m *Model) handleKey(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	// Approval card takes over the keyboard.
	if m.approval != nil {
		switch strings.ToLower(msg.String()) {
		case "y", "enter":
			m.approval.reply <- agent.DecisionOnce
			m.approval = nil
			m.notice = "已允许本次操作"
		case "a":
			tool := m.approval.req.Tool
			m.approval.reply <- agent.DecisionAlways
			m.ag.AutoApprove(tool)
			m.approval = nil
			m.notice = fmt.Sprintf("本会话自动允许 %s", tool)
		case "n", "esc", "ctrl+c":
			m.notice = "已拒绝该操作"
			m.approval.reply <- agent.DecisionDeny
			m.approval = nil
		}
		if m.approval == nil {
			m.noticeAt = time.Now()
			m.refreshViewport(false)
		}
		return m, nil
	}

	if m.slash.open {
		if handled, cmd := m.handleSlashKey(msg); handled {
			return m, cmd
		}
	}

	if m.picker != nil {
		return m.handlePickerKey(msg)
	}

	switch msg.String() {
	case "ctrl+c":
		if m.streaming && m.cancel != nil {
			m.cancel()
			m.notice = "正在中断…"
			m.noticeAt = time.Now()
			return m, nil
		}
		m.quitting = true
		return m, tea.Quit

	case "esc":
		if m.streaming && m.cancel != nil {
			m.cancel()
			m.notice = "正在中断…"
			m.noticeAt = time.Now()
			return m, nil
		}
		if m.showHelp {
			m.showHelp = false
			m.refreshViewport(false)
			return m, nil
		}
		return m, nil

	case "ctrl+l":
		m.items = nil
		m.streamIdx = -1
		m.refreshViewport(false)
		return m, nil

	case "ctrl+p":
		m.slash.open = true
		m.slash.query = ""
		m.filterSlash()
		m.refreshViewport(false)
		return m, nil

	case "enter":
		text := strings.TrimSpace(m.input.Value())
		if text == "" {
			return m, nil
		}
		if m.streaming {
			m.notice = "正在生成，按 esc 先中断"
			m.noticeAt = time.Now()
			return m, nil
		}
		m.input.Reset()
		m.layout()
		return m, m.submit(text)

	case "pgup", "pgdown", "ctrl+u", "ctrl+d":
		var cmd tea.Cmd
		m.viewport, cmd = m.viewport.Update(msg)
		m.autoScroll = m.viewport.AtBottom()
		return m, cmd

	case "up", "down":
		// History recall when the composer is empty; otherwise let the
		// textarea handle cursor movement.
		if m.input.Value() == "" && !m.streaming {
			m.recallHistory(msg.String())
			return m, nil
		}
	}

	if msg.String() == "ctrl+j" || msg.String() == "alt+enter" {
		var cmd tea.Cmd
		m.input, cmd = m.input.Update(msg)
		m.layout()
		m.refreshViewport(false)
		return m, cmd
	}

	var cmd tea.Cmd
	m.input, cmd = m.input.Update(msg)
	m.layout()
	if strings.HasPrefix(m.input.Value(), "/") && !strings.Contains(m.input.Value(), " ") {
		m.slash.open = true
		m.slash.query = strings.TrimPrefix(m.input.Value(), "/")
		m.filterSlash()
		m.refreshViewport(false)
	} else if m.slash.open && !strings.HasPrefix(m.input.Value(), "/") {
		m.slash.open = false
		m.refreshViewport(false)
	}
	return m, cmd
}

func (m *Model) recallHistory(dir string) {
	if len(m.history) == 0 {
		return
	}
	if m.historyIdx == 0 {
		m.draft = m.input.Value()
	}
	switch dir {
	case "up":
		if m.historyIdx < len(m.history) {
			m.historyIdx++
		}
	case "down":
		if m.historyIdx > 0 {
			m.historyIdx--
		}
	}
	if m.historyIdx == 0 {
		m.input.SetValue(m.draft)
	} else {
		m.input.SetValue(m.history[len(m.history)-m.historyIdx])
	}
	m.layout()
}

// submit routes slash commands or starts an agent run.
func (m *Model) submit(text string) tea.Cmd {
	m.history = append(m.history, text)
	m.historyIdx = 0
	m.draft = ""

	if strings.HasPrefix(text, "/") {
		return m.runSlash(text)
	}

	m.items = append(m.items, Item{Kind: ItemUser, Text: text, At: time.Now()})
	m.streamIdx = -1
	m.thinkingIdx = -1
	m.items = append(m.items, Item{Kind: ItemAssistant, Text: "", At: time.Now(), Streaming: true, Model: m.preset.Model})
	m.streamIdx = len(m.items) - 1
	m.autoScroll = true
	m.streaming = true
	m.runStarted = time.Now()
	m.streamChars = 0
	m.refreshViewport(true)

	ch := make(chan agent.Event, 256)
	m.events = ch
	ctx, cancel := context.WithCancel(context.Background())
	m.cancel = cancel
	m.ag.Emit = func(ev agent.Event) {
		select {
		case ch <- ev:
		default:
		}
	}
	go func() {
		err := m.ag.Run(ctx, text)
		close(ch)
		if m.program != nil {
			m.program.Send(runDoneMsg{err: err})
		}
	}()
	return tea.Batch(m.waitForEvent(ch), m.spinner.Tick)
}

func (m *Model) waitForEvent(ch chan agent.Event) tea.Cmd {
	return func() tea.Msg {
		ev, ok := <-ch
		if !ok {
			return nil
		}
		return agentEventMsg(ev)
	}
}

func (m *Model) handleAgentEvent(ev agent.Event) tea.Cmd {
	m.lastEventAt = time.Now()
	switch ev.Kind {
	case agent.EvText:
		m.streamChars += len(ev.Text)
		if m.streamIdx < 0 || m.streamIdx >= len(m.items) {
			m.items = append(m.items, Item{Kind: ItemAssistant, At: time.Now(), Streaming: true, Model: m.preset.Model})
			m.streamIdx = len(m.items) - 1
		}
		m.items[m.streamIdx].Text += ev.Text
		m.items[m.streamIdx].Streaming = true
		m.refreshViewport(m.autoScroll)

	case agent.EvThinking:
		if m.thinkingIdx < 0 || m.thinkingIdx >= len(m.items) || m.items[m.thinkingIdx].Kind != ItemThinking || !m.items[m.thinkingIdx].Streaming {
			m.items = append(m.items, Item{Kind: ItemThinking, At: time.Now(), Streaming: true})
			m.thinkingIdx = len(m.items) - 1
		}
		m.items[m.thinkingIdx].Text += ev.Text
		m.refreshViewport(m.autoScroll)

	case agent.EvToolStart:
		if m.thinkingIdx >= 0 && m.thinkingIdx < len(m.items) {
			m.items[m.thinkingIdx].Streaming = false
		}
		if m.streamIdx >= 0 && m.streamIdx < len(m.items) {
			m.items[m.streamIdx].Streaming = false
		}
		m.streamIdx = -1
		m.thinkingIdx = -1
		m.items = append(m.items, Item{Kind: ItemTool, Tool: ev.Tool, At: time.Now()})
		m.refreshViewport(m.autoScroll)

	case agent.EvToolEnd:
		for i := len(m.items) - 1; i >= 0; i-- {
			if m.items[i].Kind == ItemTool && m.items[i].Tool != nil && m.items[i].Tool.ID == ev.Tool.ID {
				m.items[i].Tool = ev.Tool
				break
			}
		}
		m.refreshViewport(m.autoScroll)

	case agent.EvUsage:
		m.usage = m.sess.Usage
		m.refreshViewport(false)

	case agent.EvNotice:
		m.items = append(m.items, Item{Kind: ItemNotice, Text: ev.Text, At: time.Now()})
		m.refreshViewport(m.autoScroll)

	case agent.EvError:
		if ev.Err != nil {
			m.items = append(m.items, Item{Kind: ItemError, Text: ev.Err.Error(), At: time.Now()})
		}
		m.refreshViewport(m.autoScroll)
	}
	if m.events != nil {
		return m.waitForEvent(m.events)
	}
	return nil
}

func (m *Model) saveCmd() tea.Cmd {
	sess := m.sess
	store := m.store
	return func() tea.Msg {
		return savedMsg{err: store.Save(sess)}
	}
}

// layout recomputes pane sizes from the terminal size.
func (m *Model) layout() {
	if m.width == 0 {
		return
	}
	value := m.input.Value()
	wrapWidth := m.width - 8
	if wrapWidth < 12 {
		wrapWidth = 12
	}
	lines := 1
	for _, seg := range strings.Split(value, "\n") {
		w := displayWidth(seg)
		lines += w / wrapWidth
	}
	if lines < 1 {
		lines = 1
	}
	if lines > 8 {
		lines = 8
	}
	m.input.SetHeight(lines)
	m.input.SetWidth(m.width - 2)

	bodyH := m.height - (lines + 5)
	if bodyH < 4 {
		bodyH = 4
	}
	m.viewport.Width = m.width
	m.viewport.Height = bodyH
}

// refreshViewport re-renders the conversation.
func (m *Model) refreshViewport(stick bool) {
	if m.width == 0 {
		return
	}
	m.viewport.SetContent(m.renderBody())
	if stick || m.autoScroll {
		m.viewport.GotoBottom()
	}
}

func (m *Model) relativeCwd() string {
	if m.home != "" && strings.HasPrefix(m.cwd, m.home) {
		rest := strings.TrimPrefix(m.cwd, m.home)
		if rest == "" {
			return "~"
		}
		return "~" + filepath.ToSlash(rest)
	}
	return filepath.ToSlash(m.cwd)
}
