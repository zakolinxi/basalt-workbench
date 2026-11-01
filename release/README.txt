BASALT 0.1.17 — Windows x64
===========================

Basalt 是一个本地优先的 AI 编程工作台（Tauri 2 + React + Rust）。

本次更新
--------
- 新增模型接口自动选择：首次成功调用后，后台探测同一供应商可用的
  OpenAI Chat / Responses 端点（其他协议供应商使用自身协议）。
- 内置模型接口库：按模型家族给出推荐协议、成本档、速度档与来源；
  覆盖 OpenAI、Anthropic、Gemini、DeepSeek、MiniMax/Kimi/Qwen/GLM、Grok、
  Ollama 等。
- 探测请求极小（max_tokens=8、非流式、5–20 秒），按实测延迟选择；
  若库推荐协议在最快结果 +60ms 内则优先推荐协议（缓存/工具更友好）。
- 结果写入 model_endpoints；后续聊天、Agent、代码审查、子代理自动使用。
- 供应商模型卡片显示自动接口、延迟、成本/速度档，并可手动「检测接口」。
- 只在当前供应商 / base URL / API Key 内选择协议，不会擅自切换其他账户。

运行要求
--------
- Windows 10/11 x64
- Microsoft Edge WebView2 Runtime（Windows 10/11 通常已预装）
- 可选：Git（用于分支、Git 状态与 HEAD Diff）
- 可选：Windows PowerShell（Agent 命令与默认终端）

两种方式
--------
1. 安装版：运行 Basalt_0.1.17_x64-setup.exe，按向导安装。
2. 便携版：直接运行 Basalt.exe。便携版仍使用 %APPDATA%\com.basalt.workbench
   保存数据库、附件与设置。

首次使用
--------
- 打开「供应商」，从模板创建供应商并填写 API Key。
- Base URL 只填域名即可，保存时自动补全版本路径。
- 从模板创建供应商时会附带当前主力模型的上下文、视觉与工具预设。
- 在模型卡片中填写上下文窗口，按需分组标记能力；卡片底部可设置默认模型。
- 添加本地项目目录后，输入框默认处于 Agent 模式。
- Agent 内置联网搜索（Bing / DuckDuckGo）、公网网页读取与只读子代理。
- 输入框「思考」可选择 off / minimal / low / medium / high / xhigh / max / ultra，
  按会话保存并按协议自动映射。
- 使用 /img 提示词调用独立图像供应商。
- 全局请求 User-Agent 可在「设置 → 安全与数据」中修改。

许可
----
- 界面字体 Inter / JetBrains Mono 使用 SIL OFL 1.1 许可，全文见 OFL.txt。

安全
----
- API Key 保存在 Windows 凭据管理器，不写入 SQLite 或日志。
- Agent 写入与命令默认需要批准；危险命令会被策略拒绝。
- 仅访问你明确添加的项目目录。
- 本项目构建未进行代码签名；Windows SmartScreen 可能显示提示。

许可
----
- 项目代码使用 MIT License，全文见 LICENSE。
