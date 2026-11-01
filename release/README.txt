BASALT 0.1.27 — Windows x64
===========================

Basalt 是一个本地优先的 AI 编程工作台（Tauri 2 + React + Rust）。

本次更新
--------
- 终端面板占满可用宽度，默认高度缩小，最大限制在窗口 32% 内；
  拖拽与 xterm 自动适配，避免压缩上方聊天空间。
- 用户消息改为右对齐圆角气泡；Assistant 回复使用轻量头像、Basalt 名称与模型胶囊，
  时间/复制悬停显示，连续回复只保留首个角色头。
- 输入框收敛为单一圆角面板，操作栏与会话状态行不再嵌套第二层边框；
  发送按钮改为圆形，减少未验证工具徽标等视觉噪音。
- 保留 0.1.26 的会话级缓存统计、按需加载与费用优化。
- 统一解析各协议缓存字段：OpenAI Chat（prompt_tokens_details.cached_tokens、
  DeepSeek prompt_cache_hit_tokens）、OpenAI Responses（input_tokens_details）、
  Anthropic（缓存读取 + 写入归一化）、Gemini（cachedContentTokenCount）。
- OpenAI Chat 流式请求自动携带 stream_options.include_usage，确保能收到 usage。
- 缓存数据写入本地 usage_stats 表，删除供应商或移除模型时同步清理。

运行要求
--------
- Windows 10/11 x64
- Microsoft Edge WebView2 Runtime（Windows 10/11 通常已预装）
- 可选：Git（用于分支、Git 状态与 HEAD Diff）
- 可选：Windows PowerShell（Agent 命令与默认终端）

两种方式
--------
1. 安装版：运行 Basalt_0.1.27_x64-setup.exe，按向导安装。
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
