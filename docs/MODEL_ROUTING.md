# 模型接口自动选择

Basalt 会为每个已保存的对话模型记录一个“已检测接口”，并在后续对话、Agent 任务、
代码审查和子代理调用中使用它。

## 检测流程

1. 模型第一次被真实调用时，先使用供应商配置的协议（也兼容旧配置）。
2. 调用成功后，后台对同一供应商的候选协议发起一次极小的探测请求
   （`max_tokens = 8`、非流式、超时 5–20 秒）。
3. 候选只会在同一个供应商/base URL/API Key 内选择：
   - OpenAI 家族：`openai_chat` 与 `openai_responses`；
   - Anthropic / Gemini / Ollama：只有自身协议，不做跨协议探测。
4. 成功的候选按实测延迟排序；若内置接口库推荐协议在最快结果的 +60ms 内，
   优先使用推荐协议（例如 Responses 的推理状态与缓存复用）。
5. 结果写入 `model_endpoints` 表；后续聊天、Agent、代码审查与子代理通过
   `chat_adapter_for` 读取该记录选择适配器。
6. 供应商页面模型卡片提供「检测接口」按钮，可随时强制重新检测。

## 内置接口库

`src-tauri/src/model_interfaces.rs` 按模型家族匹配，记录推荐协议、成本档、
速度档与来源。当前重点：

| 家族 | 推荐协议 | 成本档 | 说明 |
| --- | --- | --- | --- |
| OpenAI GPT-6 / GPT-5 / o-series | Responses | 高 | 原生推理状态、工具与缓存复用 |
| OpenAI GPT-4.x | Chat Completions | 中 | 兼容面最广 |
| Anthropic Claude | Messages | 高 / 中 / 低 | Haiku 低成本，Opus/Fable 高成本 |
| Google Gemini | generateContent | 中 / 低 | Pro/Argon 与 Flash 分档 |
| DeepSeek V4 | Chat Completions | 低 | OpenAI 兼容；峰谷计价 |
| MiniMax / Kimi / Qwen / GLM | Chat Completions | 低 | OpenAI 兼容 |
| xAI Grok | Chat Completions | 中 | OpenAI 兼容 |
| Ollama | `/api/chat` | 低 | 本地服务，不做跨协议切换 |

## 边界

- 只在同一供应商内选择协议，不会擅自切换到其他供应商的 API Key。
- 同一模型的 Chat 与 Responses 列表 token 单价通常相同；“省钱”主要来自
  缓存复用、减少失败重试、按模型成本档选择合适模型，而不是端点本身的折扣。
- 探测会消耗极少量真实 token；每个供应商/模型只自动探测一次。
- 如果网关之后移除已选端点，调用会失败；在供应商模型卡片点击「检测接口」
  可重新检测并更新。接口库价格/推荐会随官方文档变化，需要按来源核对。
- 模型上下文窗口、供应商限速与审批策略仍然生效。

## 参考来源

- OpenAI Responses API / Chat Completions 对比与推荐：
  https://docs.apiyi.com/api-capabilities/openai/native
- OpenAI 2026 价格与模型档位汇总：
  https://www.aiexpress.news/wiki/llm-api-pricing-2026
- Anthropic Claude 价格：
  https://claude.com/pricing
- Google Gemini API 价格：
  https://ai.google.dev/gemini-api/docs/pricing
- DeepSeek API 价格与 OpenAI/Anthropic 兼容：
  https://platform.deepseek.com/pricing
  https://devtk.ai/zh/blog/deepseek-api-pricing-guide-2026/

资料核对时间：2026-10-31。价格和端点支持随时可能调整，以官方页面为准。
