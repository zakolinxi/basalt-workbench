BASALT 0.2.1 — Windows x64
==========================

Basalt 是一个本地优先的 AI 编程工作台（Tauri 2 + React + Rust）。

本次更新
--------
- 新增扩展工作区：统一管理 Skills、MCP 服务器、插件包与工具注册表。
- Skill 支持导入、启停、删除与按需读取；Agent 会先加载匹配的工作流说明再执行。
- MCP 支持 stdio 与 Streamable HTTP，包含工具发现、权限审批、连接状态与工具测试。
- 插件包可通过 plugin.json 一次安装多个 Skill 与 MCP 服务器。
- 主 Agent 可并行调度最多 6 个专业子代理，右侧显示子代理任务树与报告。
- 规划、执行与代码审查模型可独立路由；任务失败支持自动重试与供应商降级。
- 新增独立自动审查、Git 提交/还原/冲突提示、任务分支隔离与多项目并行队列。
- 修复流式输出时页面抖动；停止按钮现在只在输入框为空时覆盖发送按钮。
- AI 输出期间可继续发送消息，消息会显示在输入框上方排队，本轮结束后自动发送，
  也可点击“插话”立即停止当前输出并发送。

运行要求
--------
- Windows 10/11 x64
- Microsoft Edge WebView2 Runtime（Windows 10/11 通常已预装）
- 可选：Git（用于分支、提交、还原、Git 状态与 HEAD Diff）
- 可选：Windows PowerShell（Agent 命令与默认终端）
- 可选：Node.js / uvx（运行外部 MCP stdio 服务器）

两种方式
--------
1. 安装版：运行 Basalt_0.2.1_x64-setup.exe，按向导安装。
2. 便携版：直接运行 Basalt.exe。便携版仍使用 %APPDATA%\com.basalt.workbench
   保存数据库、附件、Skills、MCP 配置与设置。

首次使用
--------
- 打开「供应商」，从模板创建供应商并填写 API Key。
- 在「设置 → 模型路由」分别配置 Agent 执行、规划与代码审查模型。
- 在「扩展」导入 Skill 文件夹，或添加 MCP 服务器后刷新工具。
- 添加本地项目目录后，输入框默认处于 Agent 模式。
- Agent 内置联网搜索、公网网页读取、并行子代理、Skill 与 MCP 工具。
- 输入框「思考」可选择 off / minimal / low / medium / high / xhigh / max / ultra。
- 使用 /img 提示词调用独立图像供应商。
- 全局请求 User-Agent 可在「设置 → 安全与数据」中修改。

安全
----
- API Key 保存在 Windows 凭据管理器，不写入 SQLite 或日志。
- Agent 写入与命令默认需要批准；危险命令会被策略拒绝。
- MCP 工具默认逐次审批；自动授权只对完全信任的服务器启用。
- Skill 与 MCP 配置保存在本机应用数据目录，请勿导入不可信插件。
- 仅访问你明确添加的项目目录。
- 本项目构建未进行代码签名；Windows SmartScreen 可能显示提示。

许可
----
- 项目代码使用 MIT License，全文见 LICENSE。
- 界面字体 Inter / JetBrains Mono 使用 SIL OFL 1.1 许可，全文见 OFL.txt。
