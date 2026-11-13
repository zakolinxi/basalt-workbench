# Basalt TUI

在终端里完成整个开发闭环的编程代理 —— 读代码、改文件、跑测试，改动前先看 diff 再批准。

这是 [Basalt](../README.md) 的终端形态（`basalt-tui`）：独立的 Go 单文件可执行程序，不依赖桌面工作台，也不需要 Tauri。视觉与 Basalt 桌面版共用一套配色和交互约定。

![欢迎页](docs/screenshots/01-welcome.png)

## 效果

| 对话与工具调用 | 改动审批 |
| --- | --- |
| ![对话](docs/screenshots/02-conversation.png) | ![审批](docs/screenshots/03-approval.png) |

![完成](docs/screenshots/04-done.png)

命令面板与斜杠补全：

![命令](docs/screenshots/05-commands.png)

## 特性

- **真流式对话**：OpenAI Chat Completions、Anthropic Messages、Ollama 原生协议，逐字输出，工具调用增量聚合。
- **真实文件操作**：`read` / `write` / `edit` / `list` / `glob` / `grep` / `bash` / `webfetch` / `update_plan`，全部限制在工作区内。
- **改动可见、可撤销**：写入前生成行级 diff；每次修改保存快照，`/undo` 一键还原。
- **审批式执行**：写文件、执行命令、抓取网页前弹出确认卡片，支持 `y` 允许一次、`a` 本会话总是允许、`n` 拒绝。
- **多 provider 热切换**：`/model <name>` 立即切换，配置来自内置预设、项目级 `.basalt/config.json` 或用户级配置。
- **会话持久化**：自动保存到用户配置目录，`/sessions` / `/resume` 恢复；`/export` 导出 Markdown。
- **终端质感**：真彩渐变、Markdown 渲染、语法高亮、行内代码、diff 着色、自适应输入框与状态栏。
- **上下文管理**：超长会话自动压缩，`/compact` 手动触发。
- **零依赖演示**：`--demo` 直接进入预置会话，不需要 API Key。

## 快速开始

需要 Go 1.24+。

```bash
cd basalt-tui
go build -o bin/basalt.exe .

# 设置任意一个 provider 的环境变量
export DEEPSEEK_API_KEY=sk-...
# 或 OPENAI_API_KEY / ANTHROPIC_API_KEY / MOONSHOT_API_KEY / ZHIPU_API_KEY

./bin/basalt.exe
```

Windows PowerShell：

```powershell
$env:DEEPSEEK_API_KEY = "sk-..."
.\bin\basalt.exe -C C:\path\to\project
```

没有 Key 也可以先看界面：

```bash
./bin/basalt.exe --demo
```

## 配置

配置文件按以下顺序查找，命中第一个即停止：

1. `$BASALT_TUI_CONFIG`
2. `<工作区>/.basalt/config.json`
3. `<UserConfigDir>/basalt-tui/config.json`（Windows：`%APPDATA%\basalt-tui\config.json`）
4. 内置默认值

最小示例（也见 [config.example.json](config.example.json)）：

```json
{
  "default": "deepseek",
  "providers": {
    "deepseek": {
      "kind": "openai",
      "baseUrl": "https://api.deepseek.com/v1",
      "model": "deepseek-chat",
      "apiKey": "${DEEPSEEK_API_KEY}",
      "maxTokens": 8192
    },
    "local": {
      "kind": "ollama",
      "baseUrl": "http://localhost:11434",
      "model": "qwen2.5-coder:7b"
    }
  }
}
```

`kind` 支持 `openai`（任意 OpenAI 兼容端点）、`anthropic`、`ollama`。API Key 为空时会自动读取对应环境变量。

## 命令

| 命令 | 作用 |
| --- | --- |
| `/help` | 帮助与快捷键 |
| `/model [name]` | 列出或切换 provider |
| `/new` | 新会话 |
| `/sessions` | 列出历史会话 |
| `/resume <id>` | 恢复会话 |
| `/compact` | 压缩上下文 |
| `/undo` | 撤销最近一次文件改动 |
| `/clear` | 清屏 |
| `/tools` | 查看工具 |
| `/cost` | token 用量 |
| `/init` | 生成 `BASALT.md` 项目说明 |
| `/export` | 导出会话 Markdown |
| `/quit` | 退出 |

快捷键：`Enter` 发送 · `Ctrl+J` 换行 · `Esc` 中断生成 · `Ctrl+L` 清屏 · `Ctrl+P` 命令面板 · `↑/↓` 历史输入 · `PgUp/PgDn` 滚动。

## 工作方式

1. 启动时读取工作区、Git 状态、浅层文件树，以及 `AGENTS.md` / `BASALT.md` / `CLAUDE.md` 作为系统提示。
2. 模型流式返回文本与工具调用；工具按顺序执行，结果回填后继续下一轮，最多 24 轮。
3. 只读工具自动执行；写操作与命令先算 diff、再请求批准。拒绝会作为工具结果返回给模型。
4. 每轮结束保存会话与用量；上下文过长时压缩较早的消息。

## 目录结构

```
basalt-tui/
  main.go                  CLI 入口、快照模式
  internal/
    agent/                 对话循环、审批、上下文压缩、系统提示
    config/                配置加载与 provider 解析
    diff/                  行级 LCS diff
    llm/                   OpenAI / Anthropic / Ollama 流式客户端
    session/               会话持久化
    snapshot/              ANSI → HTML → PNG 截图
    tools/                 文件、搜索、shell、计划、网页工具
    tui/                   Bubbletea 界面、主题、Markdown、演示场景
  docs/screenshots/        效果图
```

## 开发

```bash
go test ./...                                   # 单元 + 端到端测试（含 SSE 解析、工具执行）
go run . --demo                                 # 交互演示
go run . --snapshot docs/screenshots -C ..      # 重新生成效果图（需要 Chrome/Edge）
```

## 安全边界

- 所有路径解析后必须位于工作区内，`../` 越界会被拒绝。
- `bash` 子进程会移除常见 API Key 环境变量，避免误泄露。
- 网页抓取仅支持 `http(s)` GET，响应限制 2MB。
- 每次写操作前都能看到完整 diff，拒绝即不落盘。

## 与 opencode 的差异

这是面向 Basalt 工作流的精简实现，不是 opencode 的复刻：

- 没有 MCP、LSP、多 agent 并行、插件系统与 Web 客户端。
- 命令与工具集更小，聚焦「读 → 改 → 验证」主链路。
- 配置格式与 Basalt 桌面版不完全兼容，但配色、审批语义与提示词规则保持一致。

## License

MIT，与仓库根目录一致。
