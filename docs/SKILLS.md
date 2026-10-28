# 开发环境、Skills 与资源检查记录

## 环境检查结果

| 项目 | 结果 |
| --- | --- |
| 操作系统 | Windows 10.0.26300 x64 |
| 工作目录 | `C:\Users\juang\Desktop\ex` |
| Node.js | v26.1.0 |
| npm | 11.13.0 |
| Git | 2.54.0.windows.1 |
| WebView2 Runtime | 155.0.4283.62 |
| Rust | 初始未安装 → rustup + stable 1.99.1 (x86_64-pc-windows-msvc) |
| MSVC Build Tools | 初始未安装 → Visual Studio Build Tools 2022 / MSVC 14.44.35207 |
| 桌面构建环境 | Tauri 2 + NSIS（GitHub 下载失败后使用镜像补齐官方 NSIS 工具与插件；安装包构建与安装验证成功） |

工具链安装过程中遇到：

- 官方 `static.rust-lang.org` 通道下载极慢并中断，导致 toolchain 缺少 `rustc` manifest。
- 最终使用 rsproxy 镜像重新安装 stable 工具链；MSVC 使用 winget 静默安装（需要 UAC）。
- 以上均为本机环境操作，不改变项目运行时的网络行为。

## Skills 安装尝试

按任务要求尝试了以下资源。**当前网络环境无法克隆 GitHub 仓库**，因此全部安装失败；
没有把组件库或 MCP 误称为 Skill。

| 资源 | 尝试命令 / 方式 | 结果 |
| --- | --- | --- |
| Anthropic Frontend Design | `npx skills add anthropics/skills --skill frontend-design` | 失败：`Failed to clone https://github.com/anthropics/skills.git`，连接 443 超时 |
| UI/UX Pro Max | `npx skills add nextlevelbuilder/ui-ux-pro-max-skill` | 失败：GitHub clone 不可用 |
| Vercel React Best Practices | `npx skills add vercel-labs/agent-skills --skill vercel-react-best-practices` | 失败：GitHub clone 不可用 |
| Vercel Composition Patterns | `npx skills add vercel-labs/agent-skills --skill composition-patterns` | 失败：GitHub clone 不可用 |

`skills` CLI 本身可用（v1.7.4），失败原因是外部 Git 服务连接，而不是命令格式。

开发结束前 GitHub 连接短暂恢复（`git ls-remote` 成功），再次尝试安装；`git clone --depth 1`
仍在数分钟内无法完成并超时，故上述 Skills 最终仍未安装。没有伪造安装成功记录，
也没有把未安装的 Skill 内容冒充为已使用。

## 实际采用的资源

| 资源 | 类型 | 用法 |
| --- | --- | --- |
| Lucide Icons | 图标库 | 统一线性图标体系 |
| Monaco Editor | 编辑器组件 | 本地打包，配置本地 worker，不依赖 CDN |
| xterm.js + Fit / Search Addon | 终端渲染 | ConPTY 输出、尺寸适配、输出搜索 |
| jsdiff（`diff`） | 前端 Diff 计算 | 统一 / 并排视图的行级比较 |
| react-markdown + remark-gfm + rehype-highlight | Markdown | 对话渲染、代码高亮 |
| Tauri 2 + plugin-dialog | 桌面运行时 | 窗口、命令、原生文件选择 |
| Rust：reqwest / rusqlite / keyring / portable-pty / tokio | 后端 | 流式 HTTP、SQLite、Windows 凭据、PTY、异步任务 |
| shadcn/ui | 组件模式参考 | **未直接引入**。项目使用无 Tailwind 的独立 Token/组件层，避免与高密度黑白设计、React 19 + Vite 8 + Tauri 2 组合产生额外运行时依赖；组件 API 与组成方式保持可控、可复用 |
| Playwright MCP | 浏览器测试 | 本机未配置 Playwright MCP；UI 验证通过 Tauri 桌面窗口与构建完成验证（见 TESTING.md） |

## 指导落地情况

- 由于外部 Skills 无法安装，设计和工程实践按任务书本身的要求实现：
  先建立 Token 与组件层，再统一页面；避免模板化 AI 聊天页；所有按钮均有实际功能；
  不伪造任务进度；展示真实 Provider 结果与错误。
- 外部资源安装失败未被当作停止开发的理由；核心功能继续完成。
