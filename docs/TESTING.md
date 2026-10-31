# 自动化测试与验证记录

验证日期：2026-10-27，更新：2026-10-29
环境：Windows 10.0.26300 x64 · Node 26.1.0 · Rust 1.99.1 (MSVC) · WebView2 155

> 2026-10-28 起内置 Mock 供应商与 Mock 演示沙盒已移除；本文中涉及 Mock 的
> 冒烟流程与截图仅作历史记录，不再可执行。

## 1. 静态检查与单元测试

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| TypeScript 类型检查 | `npx tsc --noEmit` | 通过，0 errors |
| 前端单元测试 | `npx vitest run` | 18 passed（Diff / 权限 / Base URL / 工具格式化） |
| 前端生产构建 | `npm run build` | 通过，生成 `dist/` |
| Rust 编译检查 | `cargo check` | 通过 |
| Rust 测试 | `cargo test` | **38 passed, 0 failed** |

前端测试覆盖 Diff、权限、Base URL 归一化与工具名称/目标格式化。

Rust 测试覆盖：

- `files`：`..` 穿越、绝对路径、`.git` 写入、二进制检测、嵌套相对路径。
- `policy`：读取允许、写入默认审批、项目外绝对路径拒绝、危险命令拒绝、
  `git push` 审批、命令内 `..` 拒绝。
- `providers/sse`：跨 chunk 分行、`[DONE]`、SSE JSON。
- `providers/http`：URL 拼接、非法 Base URL。
- `providers/base_url`：补全 scheme 与 `/v1`、Gemini `/v1beta`、Ollama 根地址、
  已有版本路径、粘贴完整端点、本机 / 局域网 http 识别。
- `agent/web`：HTML 清理与实体解码、DDG/Bing 结果解析、内网 URL 拒绝。
- `db`：模板种子、无 Mock、设置往返持久化。
- `git`：真实临时仓库的 untracked 状态、HEAD ↔ 工作区 Diff。
- `agent/tools`：真实 PowerShell 命令执行、stdout/stderr 捕获、退出码、
  取消后进程终止。
- `error` / `secrets`：错误友好化、令牌脱敏、Keyring 服务名。

## 2. 桌面端到端验证

当前验证方式：使用临时 `identifier` 启动开发实例，避免读写真实 `%APPDATA%` 数据；
通过 WebView2 调试端口执行真实界面操作并检查请求体。已覆盖：

- 默认 Agent 模式、无 Mock 供应商、空模型路由。
- 供应商模板创建与模型目录。
- Base URL 自动补全（OpenAI `/v1`、Gemini `/v1beta`、Ollama 根地址）与保存持久化。
- 模型卡片编辑、上下文预设、默认模型切换、分组能力矩阵与批量标记。
- 工具调用摘要行：18 次调用默认折叠为一行、显示次数与失败数、展开紧凑步骤。
- Rail GitHub 入口：图标、悬浮提示与 opener 权限配置。
- 输入框底部项目 / Git 分支状态栏；点击项目名切换工作区、添加目录，非 Git 项目隐藏分支。
- 配色卡片：玄武 / 森林 / 薄荷 / 蜜蜂 / 羊皮纸，深浅主题下整页背景、表面、强调色、460ms 过渡与持久化。
- 思考强度选择器 8 档、会话持久化与各协议请求字段。
- Agent 任务创建、联网搜索、网页读取、只读子代理与时间线输出。
- 浅色/深色主题、窄屏布局。

旧的 13 步 Mock 冒烟脚本已随 Mock 移除而删除。

当前界面截图（Ollama 本地供应商 + 演示工作区，不含真实密钥/项目数据）：

| 截图 | 内容 |
| --- | --- |
| `readme-workbench.png` | 工作区 + 会话侧栏、默认 Agent 输入框 |
| `readme-reasoning.png` | 思考强度 8 档选择 |
| `readme-providers.png` | 供应商、模板与模型卡片编辑器 |
| `readme-settings.png` | 外观配色与模型路由 |

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
| `release/Basalt.exe` | 构建成功（21,588,992 字节，约 20.59 MiB） |
| `release/Basalt_0.1.14_x64-setup.exe` | NSIS 安装包构建成功（7.61 MiB） |
| `release/Basalt-0.1.14-x64-portable.zip` | 便携压缩包构建成功（9.18 MiB） |
| 首次启动 | 成功；窗口 “Basalt Workbench”；数据目录正常创建 |
| 遥测 / 网络 | 启动不联网；仅供应商请求与 Agent 联网工具会访问外部服务 |
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
