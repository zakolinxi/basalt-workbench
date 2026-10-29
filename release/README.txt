BASALT 0.1.10 — Windows x64
===========================

Basalt 是一个本地优先的 AI 编程工作台（Tauri 2 + React + Rust）。

本次更新
--------
- 左侧 Rail 底部新增 GitHub 图标入口，位于主题切换按钮上方。
- 图标使用 GitHub Octocat 描边；悬停时放大并显示 “GitHub” 提示。
- 点击使用系统默认浏览器打开 https://github.com/zakolinxi/basalt-workbench。
- 通过 Tauri opener 插件与最小权限打开外链，不修改工作区或应用数据。
- 工具执行摘要默认折叠、整页配色与 460ms 颜色过渡保持上一版行为。

运行要求
--------
- Windows 10/11 x64
- Microsoft Edge WebView2 Runtime（Windows 10/11 通常已预装）
- 可选：Git（用于 Git 状态与 HEAD Diff）
- 可选：Windows PowerShell（Agent 命令与默认终端）

两种方式
--------
1. 安装版：运行 Basalt_0.1.10_x64-setup.exe，按向导安装。
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
