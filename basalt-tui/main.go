package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/juang/basalt-tui/internal/config"
	"github.com/juang/basalt-tui/internal/snapshot"
	"github.com/juang/basalt-tui/internal/tui"
	"github.com/muesli/termenv"
)

func main() {
	var (
		workspace  = flag.String("C", "", "工作区目录（默认当前目录）")
		provider   = flag.String("provider", "", "使用的 provider 名称")
		modelFlag  = flag.String("model", "", "覆盖模型名称")
		demo       = flag.Bool("demo", false, "以演示会话启动（不需要 API Key）")
		snap       = flag.String("snapshot", "", "渲染效果图到指定目录（PNG）")
		snapWidth  = flag.Int("width", 120, "效果图终端列数")
		snapHeight = flag.Int("height", 38, "效果图终端行数")
		version    = flag.Bool("version", false, "打印版本")
	)
	flag.Parse()

	if *version {
		fmt.Printf("basalt %s\n", tui.Version)
		return
	}

	// Truecolor is required both for the TUI and for deterministic screenshots.
	lipgloss.SetColorProfile(termenv.TrueColor)

	workspaceDir := *workspace
	if workspaceDir == "" {
		workspaceDir, _ = os.Getwd()
	}
	abs, err := filepath.Abs(workspaceDir)
	if err != nil {
		fail(err)
	}
	if st, err := os.Stat(abs); err != nil || !st.IsDir() {
		fail(fmt.Errorf("工作区不存在：%s", abs))
	}

	loaded := config.Load(abs)
	if *provider != "" {
		loaded.Config.Default = *provider
	}
	if *modelFlag != "" {
		p := loaded.Config.Providers[loaded.Config.Default]
		p.Model = *modelFlag
		loaded.Config.Providers[loaded.Config.Default] = p
	}

	if *snap != "" {
		if err := renderSnapshots(*snap, abs, *snapWidth, *snapHeight); err != nil {
			fail(err)
		}
		return
	}

	model, err := tui.New(abs, loaded, *demo)
	if err != nil {
		fail(err)
	}
	if *demo {
		model.SeedDemo()
	}

	p := tea.NewProgram(model,
		tea.WithAltScreen(),
		tea.WithMouseCellMotion(),
	)
	model.SetProgram(p)
	if _, err := p.Run(); err != nil {
		fail(err)
	}
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, "basalt: "+err.Error())
	os.Exit(1)
}

func renderSnapshots(dir, workspace string, cols, rows int) error {
	absDir, err := filepath.Abs(dir)
	if err == nil {
		dir = absDir
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	browsers := snapshot.Browsers()
	if len(browsers) == 0 {
		return fmt.Errorf("未找到 Chrome 或 Edge，无法生成 PNG 效果图")
	}
	scenes, err := tui.DemoScenes(cols, rows, workspace)
	if err != nil {
		return err
	}
	winW := int(float64(cols)*8.3) + 150
	winH := int(float64(rows)*19.2) + 170
	for _, scene := range scenes {
		frame := scene.RenderFrame()
		htmlPath := filepath.Join(dir, scene.Name+".html")
		pngPath := filepath.Join(dir, scene.Name+".png")
		if err := os.WriteFile(htmlPath, []byte(snapshot.Page(frame, scene.Title)), 0o644); err != nil {
			return err
		}
		if err := snapshot.Capture(browsers[0], htmlPath, pngPath, winW, winH); err != nil {
			return fmt.Errorf("截图失败 %s：%w", scene.Name, err)
		}
		fmt.Printf("  ✓ %s\n", strings.TrimPrefix(pngPath, workspace+string(filepath.Separator)))
	}
	return nil
}
