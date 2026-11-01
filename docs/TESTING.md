# 自动化测试与验证记录

验证日期：2026-10-27，更新：2026-11-01
环境：Windows 10.0.26300 x64 · Node 26.1.0 · Rust 1.99.1 (MSVC) · WebView2 155

> 2026-10-28 起内置 Mock 供应商与 Mock 演示沙盒已移除；本文中涉及 Mock 的
> 冒烟流程与截图仅作历史记录，不再可执行。

## 1. 静态检查与单元测试

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| TypeScript 类型检查 | `npx tsc --noEmit` | 通过，0 errors |
| 前端单元测试 | `npx vitest run` | **23 passed**（Diff / 权限 / Base URL / 工具格式化 / i18n） |
| 前端生产构建 | `npm run build` | 通过，生成 `dist/` |
| Rust 编译检查 | `cargo check` | 通过 |
| Rust 测试 | `cargo test` | **62 passed, 0 failed** |

前端测试覆盖 Diff、权限、Base URL 归一化、工具名称/目标格式化，
以及 i18n 精确匹配、数字插值、空翻译与回退。

Rust 测试覆盖：

- `files`：`..` 穿越、绝对路径、`.git` 写入、二进制检测、嵌套相对路径。
- `policy`：读取允许、写入默认审批、项目外绝对路径拒绝、危险命令拒绝、
  `git push` 审批、命令内 `..` 拒绝。
- `providers/sse`：跨 chunk 分行、`[DONE]`、SSE JSON。
- `providers/http`：URL 拼接、非法 Base URL。
- `providers/base_url`：补全 scheme 与 `/v1`、Gemini `/v1beta`、Ollama 根地址、
  已有版本路径、粘贴完整端点、本机 / 局域网 http 识别。
- `model_interfaces`：模型家族推荐协议、OpenAI Chat/Responses 候选、
  非 OpenAI 供应商候选限制。
- `agent/web`：HTML 清理与实体解码、DDG/Bing 结果解析、内网 URL 拒绝。
- `providers/*` 缓存用量：OpenAI `prompt_tokens_details.cached_tokens`、
  DeepSeek `prompt_cache_hit_tokens`、Anthropic 缓存读写归一化、
  `message_delta` 缺少输入字段时不清零、累计快照按字段取最大值；
  Anthropic `cache_control` 断点、无缓存回退与模型工具回传上限。
- `db`：模板种子、无 Mock、设置往返持久化、会话级缓存用量隔离、
  运行指标按任务聚合与最新聊天指标读取。
- `git`：真实临时仓库的 untracked 状态、HEAD ↔ 工作区 Diff、分支列表、切换与新建。
- `agent/tools`：真实 PowerShell 命令执行、stdout/stderr 捕获、退出码、
  取消后进程终止。
- `skills`：SKILL.md frontmatter（折叠/字面量/引号）、无 frontmatter、Skill 文件读取
  与目录越界拒绝。
- `mcp`：工具名命名空间与 64 字符限制、SSE JSON 帧解析、stdio/HTTP 输入校验。
- `plugins`：plugin.json 安装路径不得逃逸插件根目录。
- `agent/round`：自动重试次数上限与指数退避增长。
- `error`：429/5xx 判定为可重试，401/404 不重试；错误友好化、令牌脱敏、Keyring 服务名。
- `policy`：read_skill、spawn_subagent、spawn_agents 均允许，写入/命令仍按审批策略。
- `git`：真实临时仓库的 untracked 状态、HEAD ↔ 工作区 Diff、分支列表、切换与新建、
  按路径提交、还原已跟踪文件、拒绝未跟踪文件与 `..` 路径。
- `secrets`：Keyring 服务名稳定。

## 2. 桌面端到端验证

当前验证方式：使用临时 `identifier` 启动开发实例，避免读写真实 `%APPDATA%` 数据；
通过 WebView2 调试端口执行真实界面操作并检查请求体。已覆盖：

- 默认 Agent 模式、无 Mock 供应商、空模型路由。
- 供应商模板创建与模型目录。
- Base URL 自动补全（OpenAI `/v1`、Gemini `/v1beta`、Ollama 根地址）与保存持久化。
- 模型卡片编辑、上下文预设、默认模型切换、分组能力矩阵与批量标记。
- 工具调用摘要行：18 次调用默认折叠为一行、显示次数与失败数、展开紧凑步骤。
- Rail GitHub 入口：图标、悬浮提示与 opener 权限配置。
- 输入框底部项目 / Git 分支状态栏；点击项目名切换工作区、添加目录；点击分支切换或新建分支。
- 上下文占用条：系统提示 / 消息 / 工具 / 输入 / 剩余容量明细与实际 tokens 上限。
- 自动接口选择：本地假服务同时提供 Chat/Responses；首次调用走 Chat，后台探测记录
  Responses，第二次调用实际命中 `/responses`；手动「检测接口」与持久化已验证。
- 缓存统计：本地假服务的 Chat 流返回 `cached_tokens`，首轮显示 `152.3K tok ·
  缓存命中 98.3%`；后台切到 Responses 后第二轮累计为 `457K tok · 98.3%`、
  2 次请求；确认流式 Chat 请求携带 `stream_options.include_usage`。
- 缓存详情面板：点击缓存条展开 Token 用量，显示总量 `458,720 tok`、缓存命中
  `98.3%`、未缓存输入 `7,740 tok`、缓存读取 `449,280 tok`、输出 `1,700 tok`；
  点击外部与 Esc 均可关闭。
- 连续 Assistant 消息：构造 1 条用户消息 + 3 条连续回复，仅首条保留角色头，
  后两条头部为 `display: none`、间距收为 10px，内容连续显示。
- 任务布局重构：构造 1 个中断任务（3 轮文字、2 批工具、PASS/FAIL 测试、5 项计划、
  2 个文件变更、7.5s/90 输出 tokens 指标）；确认任务卡片移除，工具批次与测试内联，
  底部显示 `Cache Model · 7.5s · 12 tok/s` 与「继续」，右侧 TODO 高度为面板 15%
  （124px / 828px）、仅三项并可显示全部 5 项，代码变更位于 TODO 下方。
- 运行中状态：将任务置为 running 后确认统计条隐藏、底部显示「Agent 正在执行 / 停止」；
  清除运行指标后确认仍显示「继续」回退按钮。
- 自定义下拉：设置页 10 个、供应商页 2 个、能力矩阵 8 个全部替换，原生 `select` 数量为 0；
  语言、主题与能力状态下拉均可打开、选择与关闭，语言下拉切换后界面即时更新。
- 会话缓存隔离：为两个会话写入不同 run_metrics 后切换查看，
  `Cache usage test` 显示 `305.6K tok · 98.3%`，`再看看` 显示 `3.3K tok · 86.7%`，
  无指标的新会话不显示缓存条；切换回原会话数值保持。
- Anthropic 缓存请求：本地假服务验证 system 与最后消息均带 `cache_control`，
  usage 归一化后为输入 4700 / 缓存读取 3000；被网关拒绝时无缓存重试。
- 启动分包：首屏资源未加载 `CodeEditorView`、`TerminalPanel`、设置/供应商/图像视图；
  主入口 JS 从约 5.27 MB 降至约 817 KB，Monaco 与 xterm 独立懒加载；
  终端首次打开后正常挂载，确认不再占启动成本。
- 终端布局：实测终端面板宽度 1183px = 主区域宽度，xterm 1167px 填满，
  面板高度 140px（低于 32vh 上限），上方聊天区不再被大块空白压缩。
- 对话视觉：用户消息右对齐气泡；Assistant 仅显示一次轻量角色头（头像 + Basalt +
  模型胶囊），时间/复制悬停出现；输入框状态行与操作栏处于同一圆角面板内。
- 四语言切换：在设置页依次切换 简体中文 → English → 日本語 → 한국어 → 简体中文，
  确认标题、工具摘要、测试结果、缓存命中与模型选择同步更新并正确还原；
  `Tools run: 3, 1 failed`、`ツール実行 3 回、1 件失敗`、
  `도구 실행 3회, 1회 실패`。English 模式下设置、供应商、图像主视图无残留中文 UI；
  用户内容（会话标题、模型输出、计划文本）保持原文。

### 扩展、MCP 与故障恢复（2026-11-01）

- 扩展页四个标签均可打开：Skills、MCP、插件包、工具注册表；界面、权限提示与统计数字正常。
- 使用临时 Skill 文件夹验证安装、列表、启用状态、删除与工具注册表联动；
  安装后 `read_skill` 自动出现在注册表，系统提示增加 Skill 索引。
- 使用本地 Node stdio Mock MCP 验证完整握手：`initialize` →
  `notifications/initialized` → `tools/list`，服务器状态变为 READY，
  `add` / `echo` 两个工具及只读标注正确显示。
- 通过管理页测试按钮验证 `tools/call`：参数 `{"a":2,"b":40}` 返回 `42`，
  结果摘要为 `MCP 完成：demo-mcp / add`；进程在刷新与编辑配置后可复用/重建。
- 使用 `plugin.json` 插件包验证一个 Skill + 一个 MCP 服务器同时安装；插件内相对脚本路径
  自动锚定到安装目录，安装后自动刷新工具；注册表从 17 增至 19 个工具。
- 设置页验证自动重试（0–5 次）、退避时间、降级供应商/模型、独立自动审查与 Git 分支隔离；
  Agent 执行路由已接入 Composer 的 Agent 模式。
- 英文界面下扩展页标题、说明、服务器表单、注册表与工具测试均完成翻译；用户安装的
  Skill 名称与描述作为内容保持原文。
- 测试过程中发现并修复 `task_get` 读取子代理列表时再次获取 SQLite 锁造成的自锁，
  该问题会导致全部 Tauri IPC 无响应。

### 流式停止与排队消息（2026-11-01）

- 本地 Mock OpenAI Chat 服务以约 2.1 秒流式回复验证：
  - 输入框为空时，发送按钮原位变为红色停止按钮。
  - 输入新内容后，按钮恢复为发送并提示“加入队列，本轮结束后发送”。
  - 第二条消息显示在输入框上方排队条，本轮结束后自动发送，最终形成两轮完整对话。
  - “插话”可停止当前输出并立即发送排队消息。
  - “编辑”把排队消息退回输入框，“删除”移除排队项。
- Agent 模式使用同一 Mock 服务验证：第一条任务执行期间第二条进入队列，
  任务 completed 后自动创建并执行第二条。
- 移除 `.chat-scroll` 的 smooth 行为、token 更新后的重复延时滚动和 Assistant
  入场动画；改用布局同步跟随与 ResizeObserver，流式输出不再来回抖动。

### 空会话标题对齐（0.2.2）

- 隔离实例创建无消息会话后检查 `.brand-mark`、`h1`、`.sub` 与正文位置。
- 品牌图标与标题顶部对齐，副标题位于标题下方，原先标题左侧的空白块消失。
- 配色卡片：玄武 / 森林 / 薄荷 / 蜜蜂 / 羊皮纸，深浅主题下整页背景、表面、强调色、460ms 过渡与持久化。
- 思考强度选择器 8 档、会话持久化与各协议请求字段。
- Agent 任务创建、联网搜索、网页读取、只读子代理与时间线输出。
- 浅色/深色主题、窄屏布局。

旧的 13 步 Mock 冒烟脚本已随 Mock 移除而删除。

当前界面截图（隔离演示数据库 `gateway-core`，不含真实密钥/项目数据）：

| 截图 | 内容 |
| --- | --- |
| `readme-workbench.png` | 工作区、会话气泡、内联工具、缓存条与上下文占用 |
| `readme-agent.png` | Agent 完成后的工具摘要、测试 PASS/FAIL 与 Git Diff |
| `readme-reasoning.png` | 思考强度 8 档选择（含说明与选中标记） |
| `readme-providers.png` | 供应商、协议、鉴权与模型能力编辑器 |
| `readme-settings.png` | 主题、五套配色、语言与终端/编辑器设置 |
| `readme-languages.png` | 简体中文 / English / 日本語 / 한국어 切换菜单 |
| `readme-terminal.png` | 全宽自适应 PowerShell（`git status` 输出） |

测试发现的真实缺陷及修复：

- `task_get` 在持有 SQLite 锁时调用 `list_changes`，造成自锁，导致任务无法启动。
  已拆分 `list_changes_conn`，在锁内复用连接。
- 快速“新建会话”后立即发送消息可能创建重复空会话。已增加在途创建去重。
- PowerShell 终端 cwd 使用了 Windows verbatim 前缀 `\\?\`，提示符显示异常。
  已增加 `display_path` 去除前缀。

### 最终回归（干净首次启动）

在清除应用数据与 WebView2 配置目录后再次运行：
**13/13 通过，0 console error，0 page exception**。截图已更新。

## 3. 发布构建

| 产物 | 结果 |
| --- | --- |
| `release/Basalt.exe` | 构建成功（23,377,920 字节，约 22.30 MiB） |
| `release/Basalt_0.2.2_x64-setup.exe` | NSIS 安装包构建成功（8.11 MiB） |
| `release/Basalt-0.2.2-x64-portable.zip` | 便携压缩包构建成功（9.84 MiB） |
| 发布 EXE 冒烟 | 文件版本 0.2.2；空会话标题与品牌图标对齐已修正 |
| 应用图标 | `gpt-image-2.5-sunburst` 生成 1254px 源图，透明圆角，`tauri icon` 生成全套各平台图标 |
| 首次启动 | 成功；窗口 “Basalt Workbench”；数据目录正常创建 |
| 遥测 / 网络 | 启动不联网；仅供应商请求、Agent 联网工具与用户配置的 MCP 服务器会访问外部服务 |
| 静默安装 | `/S /D=<temp>`，退出码 0 |
| 安装后启动 | 安装目录 `basalt.exe` 启动成功，窗口标题 `Basalt Workbench` |
| 静默卸载 | `uninstall.exe /S` 退出码 0，安装目录已移除 |

构建说明：GitHub 下载 Tauri NSIS 工具超时，使用可用镜像下载官方
`nsis-3.13.zip` 与 `nsis_tauri_utils-v0.5.3` DLL（SHA1 校验一致）后，
部署到 `%LOCALAPPDATA%/tauri/NSIS` 完成打包。安装包未签名。

## 4. 未执行的真实服务验证

由于没有可用真实 API Key / Ollama 服务，以下只完成协议实现与单元测试，
没有声称真实供应商验证通过：

- OpenAI Chat / Responses、Anthropic、Gemini 的真实流式与工具调用。
- OpenAI Images / Gemini Images 的真实生成、编辑、远程 URL 下载。
- 真实视觉输入、蒙版编辑、供应商间路由切换。
- 真实 API 的视觉模型、代码审查模型。

请配置真实供应商后，在供应商页面逐项执行连接测试与实际调用，并记录每项能力结果。

## 5. 其他未覆盖项

- 不同 DPI / 多显示器视觉回归。
- 超大项目与超长终端输出压力测试。
- 撤销冲突流程的自动化 UI 测试（后端 hash 冲突逻辑已实现；单元测试未构造
  AppState 级冲突场景）。
- 供应商新增/编辑/删除/真实连接测试的 UI 自动化（页面加载已验证；真实调用需要 API Key）。
- 拒绝审批、取消运行任务、项目目录选择器、用户文件保存、终端交互输入、图像编辑的
  UI 自动化；这些路径已实现，但未纳入本次 13 步脚本。
- 多个真实供应商之间的路由切换与自动能力验证（需要真实凭据）。
