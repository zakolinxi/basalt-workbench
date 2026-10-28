# 供应商接入指南

## 在应用内新增供应商

1. 打开「供应商」→「新增供应商」。
2. 选择分类（对话 / 图像）和协议；分类决定可用协议与字段。
3. 填写：
   - 显示名称
   - Base URL
   - API Key（仅一次写入）
   - 超时（5–600 秒）
   - 流式开关（仅对话）
   - 启用状态
4. 添加模型并设置能力状态；也可以保存后「测试连接 / 拉取模型」自动发现。
5. 设置默认聊天 / 视觉 / 图像模型。
6. 保存，在「设置 → 模型路由」中把用途指向该供应商/模型。

API Key 保存在 Windows Credential Manager 的 `com.basalt.workbench` 服务下，
账户名等于 provider id。数据库只保存 `has_key` 派生状态，不保存明文。

## Base URL 约定

| 协议 | Base URL 示例 | 适配器追加路径 |
| --- | --- | --- |
| OpenAI Chat Completions | `https://api.openai.com/v1` | `/chat/completions`、`/models` |
| OpenAI Responses | `https://api.openai.com/v1` | `/responses`、`/models` |
| Anthropic Messages | `https://api.anthropic.com/v1` | `/messages`、`/models` |
| Gemini | `https://generativelanguage.googleapis.com/v1beta` | `/models/{model}:generateContent` |
| Ollama | `http://127.0.0.1:11434` 或带 `/api` | `/api/chat`、`/api/tags` |
| OpenAI Images | `https://api.openai.com/v1` | `/images/generations`、`/images/edits` |
| Gemini Images | Gemini base | `/models/{model}:generateContent` |

OpenAI 兼容服务只需支持相应端点。兼容格式不等于兼容全部能力；以能力矩阵与真实测试为准。

## 能力矩阵

每个模型可独立标记：

- 对话
- 流式
- 工具调用
- 视觉
- 文本生图
- 图像编辑
- JSON / 结构化输出
- 长上下文
- 多轮工具调用
- 思考强度（标记为不支持时，输入框会隐藏强度选择）

状态：`verified`（已通过真实调用/接口元数据）、`unverified`、`unsupported`、
`failed`、`unavailable`。

- 测试连接成功只表示模型列表/认证可用，不等同于所有能力已验证。
- 聊天/工具/图像成功调用后，应用会把对应能力自动标记为 `verified`。
- Agent 模式会明确阻止标记为 `unsupported` 的模型进行工具调用。
- 图像工作区根据 `ImageCapabilities` 隐藏/禁用不支持参数，并在后端再次校验。

## 添加新的对话协议

1. 在 `src-tauri/src/providers/` 新建适配器模块。
2. 实现 `ChatAdapter`：
   - `list_models`
   - `chat`（解析流式/非流式，并发送规范化 `ProviderEvent`）
3. 在 `providers/mod.rs::chat_adapter` 注册协议字符串。
4. 在 `ProviderInput` 验证协议白名单和 `db.rs` 模板中加入协议。
5. 如需前端标签，更新 `src/lib/format.ts::providerProtocolLabel`。
6. 补充 URL/SSE/错误映射测试。

不要修改聊天 UI、Agent Runtime 或任务数据库结构。

## 添加新的图像协议

1. 实现 `ImageAdapter`：`capabilities`、`list_models`、`generate`、`edit`。
2. 注册到 `image_adapter`。
3. 在 `ImageCapabilities` 中如实声明尺寸、质量、数量、参考图、蒙版、透明背景、
   异步能力与限制说明。
4. 返回 `ImageOutput { bytes, mime, width, height, mock }`；服务层负责保存与历史。
5. 不允许静默忽略不支持参数；服务端会再次拒绝。

## 预置模板

应用会写入 17 个模板（仅配置骨架，不含密钥）：

OpenAI Chat Completions、OpenAI Responses、Anthropic、Gemini、DeepSeek、MiniMax、
Moonshot Kimi、阿里云百炼 Qwen、智谱 GLM、xAI Grok、OpenAI 兼容接口、自建网关、
Ollama、OpenAI 图像、Gemini 图像、OpenAI 兼容图像、自建图像网关。

对话模板内置当前主力模型的预置目录：上下文窗口、视觉、工具调用与思考强度支持情况。
预置能力均标记为 `unverified`，仍以真实调用验证为准；模板目录会随应用启动自动刷新。
使用模板创建的是独立供应商，名称直接沿用模板名称；同名时自动追加序号，不再添加“副本”。
模板本身保持只读。

## 思考强度

输入框的“思考”选择器支持 8 档：`off` / `minimal` / `low` / `medium` / `high` /
`xhigh` / `max` / `ultra`，按会话保存。各协议映射：

| 协议 | 关闭 / 极低 | low ~ max | ultra |
| --- | --- | --- | --- |
| OpenAI Chat / Responses | `off → none`、`minimal → minimal`（GPT-6 系退 `low`） | 同名透传 | `max` |
| Anthropic Messages | `off / minimal → low`（新模型思考常开） | `output_config.effort` 同名透传 | `max` |
| Gemini 3/4 | `off / minimal → low`（思考常开） | `low / medium / high`，更高档取 `high` | `high` |
| Gemini 2.5 | budget `0`（Pro 为 128）/ `512` | `1024 / 8192 / 24576 / 32768` | `32768` |
| Ollama | `think: false` / `"low"` | `low / medium / high` | `"max"` |
| DeepSeek（OpenAI 兼容） | `thinking.type = disabled` | `low`、`high`，更高档取 `max` | `max` |
| MiniMax（OpenAI 兼容） | 退 `low`（新模型思考常开） | `low / medium / high / xhigh / max` | `max` |
| Kimi / GLM（OpenAI 兼容） | 退 `low` | `low / high`，更高档取 `max` | `max` |

供应商自定义参数仍可覆盖这些字段（core 字段除外）。Anthropic 在 `xhigh` / `max` /
`ultra` 下会自动提高 `max_tokens`，避免思考被输出上限截断。

## 请求行为

- 自定义请求头会合并，但禁止覆盖 Authorization / x-api-key 等核心认证头。
- 自定义参数合并到请求体，保护 model、messages、tools、stream 等核心字段。
- 全局请求 User-Agent 可在「设置 → 安全与数据」中修改；供应商自定义 `User-Agent`
  请求头优先。默认使用 Windows Chrome 标识并附加 `Basalt/0.1.2`。
- 每个请求有连接与总超时；对话支持取消。
- 错误按 401/403/404/429/5xx 分类为可读提示，不记录密钥。
