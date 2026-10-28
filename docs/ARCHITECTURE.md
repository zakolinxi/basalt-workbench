# 架构与协议适配

## 总览

```
┌────────────────────────────────────────────────────────────┐
│ React UI (Tauri WebView2)                                  │
│  Workbench · Providers · Image Studio · Settings           │
│  Zustand Store · Event listeners · Monaco · xterm.js       │
└───────────────▲───────────────────────────────▲────────────┘
                │ invoke(commands)              │ Tauri Events
┌───────────────┴───────────────────────────────┴────────────┐
│ Rust Core (src-tauri)                                      │
│  Provider Adapters ── Agent Runtime ── Image Providers     │
│  Permission Policy · Tool Execution · Event Bus            │
│  Terminal Manager · Change/Diff Service · Git Service      │
│  SQLite Persistence · Windows Credential Manager · Logger  │
└────────────────────────────────────────────────────────────┘
```

三条强制解耦边界：

1. **UI 不直接调用供应商 HTTP**：所有请求在 Rust 适配器层完成。
2. **Agent Runtime 不依赖供应商私有实现**：只依赖规范化事件与工具调用结构。
3. **Image Provider 独立于聊天模型**：图像工作区与 `/img` 只要求图像路由，不要求当前聊天模型支持生图。

## 后端模块

| 模块 | 职责 |
| --- | --- |
| `providers/` | 协议适配器、SSE/NDJSON 解析、模型发现、错误规范化 |
| `agent/` | 任务队列、状态机、循环、工具调度、审批等待 |
| `policy.rs` | 危险命令、路径边界、写入审批策略 |
| `changes.rs` | 写入前快照、变更记录、接受/撤销、冲突检测 |
| `git.rs` | Git 状态、HEAD ↔ 工作区 Diff（固定参数调用，无 shell 拼接） |
| `terminal.rs` | ConPTY 终端、多会话、实时输出、退出码、进程终止 |
| `files.rs` | 项目根边界、符号链接校验、读取上限、编码回退 |
| `db.rs` | SQLite schema、种子数据、DTO 映射 |
| `secrets.rs` | Windows 凭据管理器（service: `com.basalt.workbench`） |
| `events.rs` | 任务事件持久化 + 广播 |
| `messaging.rs` | 会话消息读写、附件到模型消息的转换 |
| `commands/` | Tauri 命令边界（只做校验、调度与序列化） |

## 前端模块

- `lib/api.ts`：唯一 IPC 入口，所有命令与参数类型集中管理。
- `lib/store.ts`：Zustand 状态机；消费 Tauri 事件，维护任务、流式消息、审批、变更、
  终端标签与布局。
- `lib/diff.ts`：基于 jsdiff 的统一 / 并排行模型。
- `lib/monaco.ts`：本地 Monaco + 本地 worker，禁止 CDN。
- `components/`：可复用 UI、聊天、时间线、Diff、终端、供应商表单。
- `views/`：工作台、供应商、图像工作区、设置。

## Provider Adapter

统一对话接口（Rust trait）：

```rust
async fn list_models(client, runtime) -> Vec<DiscoveredModel>;
async fn chat(client, runtime, request, emit, cancel) -> Result<()>;
```

规范化输入：

```rust
ChatRequest { model, messages, tools, max_tokens, temperature }
ChatMessage { role, content, images, tool_calls, tool_call_id, tool_name }
```

规范化流式事件：

```rust
TextDelta(String)
ToolCallDelta { index, id, name, args_delta }
Usage { input_tokens, output_tokens }
Done { stop_reason }
```

新增供应商时：实现适配器、在 `providers/mod.rs` 注册协议、增加模板即可；聊天 UI、任务系统
和图像工作区不需要修改。

图像接口独立：

```rust
trait ImageAdapter {
    fn capabilities(runtime, model) -> ImageCapabilities;
    async fn list_models(..);
    async fn generate(..) -> Vec<ImageOutput>;
    async fn edit(..) -> Vec<ImageOutput>;
}
```

`ImageCapabilities` 描述尺寸、质量、数量、参考图、蒙版、透明背景与同步/异步能力；
UI 仅展示真实支持项，服务端再次校验，不会静默忽略不支持参数。

## 协议适配细节

### OpenAI Chat Completions

- 端点：`{base}/chat/completions`、`{base}/models`。
- 消息：支持文本、`image_url` data URL、assistant `tool_calls`、tool 结果。
- 流式：SSE `data:`，处理 `delta.content`、`delta.tool_calls`、`finish_reason`、`[DONE]`。
- 非流式：`choices[0].message` 同构转换为相同 ProviderEvent。
- 自定义参数合并时保护 `model/messages/tools/stream` 核心字段。

### OpenAI Responses

- 端点：`{base}/responses`。
- 输入：`input_text`、`input_image`、`function_call`、`function_call_output`。
- 工具：`{type:"function", name, description, parameters}`。
- 事件：`response.output_text.delta`、`response.output_item.added`、
  `response.function_call_arguments.delta`、`response.completed` 等分别解析。
- 不使用 Chat Completions 中转，保留 Responses 的工具与事件语义。

### Anthropic Messages

- 端点：`{base}/messages`、`{base}/models`；header `x-api-key` + `anthropic-version`。
- system 独立字段；tool_result 作为 user content block；assistant 的 tool_use 保留 `id/input`。
- SSE：`content_block_start`（tool_use）、`content_block_delta`（text / input_json）、
  `message_delta`、`message_stop`。
- 默认 `max_tokens=4096`，可由供应商自定义参数覆盖。

### Google Gemini

- 端点：`{base}/models/{model}:generateContent` 与 `...:streamGenerateContent?alt=sse`。
- 使用 `x-goog-api-key` header，不把密钥放 URL。
- content → contents/parts；system → `systemInstruction`；
  tool → `functionDeclarations`；结果 → `functionResponse`。
- 工具调用没有原生 ID，适配层生成稳定合成 ID，并通过函数名回填结果。
- 参数 schema 会移除 Gemini 不支持的关键字（如 `additionalProperties`）。

### Ollama

- 端点：`/api/chat`、`/api/tags`；base 可填根地址或已带 `/api`。
- 请求/响应为 NDJSON；支持 `images` base64、`tool_calls`、`done_reason` 与 token 统计。
- 非流式与流式共用规范化事件。

### OpenAI Images / Gemini Images

- OpenAI：`/images/generations`（JSON）与 `/images/edits`（multipart，含可选 mask）。
- Gemini：`generateContent` + `responseModalities: ["TEXT","IMAGE"]`，支持单张参考图，
  不支持蒙版与异步。
- 远程图片 URL：仅 HTTPS，拒绝 localhost / 私网 IP / 解析到内网的域名，
  禁止重定向并限制 30MB。

## Agent Runtime

```
task_create → status=queued → scheduler（全局单活跃任务）
  → running → provider.stream_chat(tools)
  → 文本增量 → ChatEvent(delta)
  → 工具调用 → Policy → 审批（可选）→ Tool Execution
  → TaskEvent(ToolCallStarted/Completed, FileChanged, TerminalOutput, TestCompleted)
  → tool result 回灌模型 → 下一轮
  → 无工具调用 → finalize(completed + report)
```

状态：`queued / running / waitingApproval / completed / failed / cancelled / interrupted`。

工具集：

| 工具 | 审批 | 说明 |
| --- | --- | --- |
| `update_plan` | 否 | 持久化计划步骤并触发时间线事件 |
| `list_files` / `search_files` / `read_file` | 否 | 项目边界内读取，大小上限 |
| `write_file` / `apply_patch` | 是（可配置） | 写入前快照；精确匹配补丁 |
| `git_status` / `git_diff` | 否 | 固定参数 Git 调用 |
| `web_search` / `fetch_url` | 否 | Bing / DuckDuckGo 搜索与公网 HTTPS 页面纯文本读取；拒绝内网、localhost、凭据 URL 与不受控重定向 |
| `spawn_subagent` | 否 | 只读子代理：同一模型的独立调查循环，仅可读文件、Git 与联网工具，不能写入、执行命令或再开子代理 |
| `run_command` / `run_tests` | 是（可配置） | PowerShell 非交互、超时、取消、进程树终止；策略可硬拒绝 |

- 任务串行执行；后续任务进入队列，前一个结束后自动启动。
- 审批等待 15 分钟超时；拒绝后工具结果回灌模型，任务继续。
- 取消会传递 `CancellationToken`，命令执行使用进程树 kill。
- 每个工具调用、结果、参数、时间与状态写入 SQLite。
- 每次 Agent 运行默认最多 200 轮（设置可调至 1000）；达到安全上限时任务转为
  `interrupted` 而非 `failed`，进度与变更保留，可提高上限后继续。旧版 20 轮默认值
  与因此失败的轮次上限任务会在启动迁移中一次性升级 / 转为可继续状态。
- 时间线把一次任务中的工具调用聚合为可折叠执行卡：折叠时按工具统计次数与总耗时，
  展开后为紧凑步骤；成功步骤只显示状态图标，不再逐行输出 `COMPLETED`。
- 完成任务前检查真实工具结果；报告附带真实变更文件与最近测试记录。

## 事件总线

Tauri 事件：

- `basalt:chat-event`：`userMessage / delta / message / error / cancelled`。
- `basalt:task-event`：`TaskCreated / TaskQueued / TaskStarted / PlanUpdated /
  StepStarted / StepCompleted / ToolCallStarted / ToolCallCompleted / ApprovalRequired /
  ApprovalResolved / FileChanged / DiffUpdated / TerminalOutput / TestCompleted /
  TaskCompleted / TaskFailed / TaskCancelled / TaskInterrupted`。
- `basalt:terminal-output` / `basalt:terminal-exit`（含退出码）。
- `basalt:image-event`：started / completed / failed。

每个任务事件都持久化到 `task_events`，UI 重启后可由 `task_get` 重建时间线。

## 持久化

SQLite 表：`settings`、`projects`、`providers`、`sessions`、`messages`、`tasks`、
`task_events`、`tool_calls`、`changes`、`image_generations`。

- API Key：不进 SQLite，使用 Windows Credential Manager。
- 附件、生成图片、变更快照、日志：应用数据目录。
- 布局与折叠状态：WebView localStorage（纯界面状态）。
- 恢复：启动时 `running/waitingApproval/queued` 任务标记为 `interrupted`，
  未完成工具调用标记 `cancelled`；用户可继续任务，不会自动重放旧工具调用。
- 变更快照在应用重启后仍可审查 / 撤销；冲突通过当前文件 hash 检测。

## 已知架构限制

- Agent 工具命令使用 PowerShell；用户终端可选 CMD/WSL，但 Agent 不自动选择 WSL。
- 没有操作系统级文件/网络沙箱；审批 + 路径/命令策略是安全边界。
- 规划的独立“Agent 规划模型”路由已持久化，但当前 Runtime 使用统一的 Agent 执行模型完成规划与执行。
- 视觉模型路由已配置，但对话仍使用会话选定模型；需要用户选择具备视觉能力的模型。
- 图像供应商均为同步接口；异步任务查询/取消未启用（能力矩阵标记为同步）。
- 重试与供应商故障自动切换没有自动策略；用户可手动重试，且不会静默切换计费更高的供应商。
