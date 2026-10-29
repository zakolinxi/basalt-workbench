BASALT 0.1.9 — Windows x64
==========================

Basalt 是一个本地优先的 AI 编程工作台（Tauri 2 + React + Rust）。

本次更新
--------
- 工具执行摘要默认折叠，不再自动展开；运行中也保持折叠。
- 折叠态改为一行简洁摘要：方形箭头按钮 + “执行工具 N 次”，
  失败时追加“其中 N 次失败”，与参考样式一致。
- 移除了折叠态的图标、工具标签和耗时堆叠；展开后仍显示紧凑工具步骤、
  失败状态、参数与真实输出。
- 整页配色与 460ms 颜色过渡保持上一版行为。

运行要求
--------
- Windows 10/11 x64
- Microsoft Edge WebView2 Runtime（Windows 10/11 通常已预装）
- 可选：Git（用于 Git 状态与 HEAD Diff）
- 可选：Windows PowerShell（Agent 命令与默认终端）

两种方式
--------
1. 安装版：运行 Basalt_0.1.9_x64-setup.exe，按向导安装。
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
