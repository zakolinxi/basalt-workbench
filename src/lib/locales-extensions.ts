import type { Language } from "./types";

type LocaleDictionary = Record<string, string>;

const en: LocaleDictionary = {
  "扩展": "Extensions",
  "插件包": "Plugin packs",
  "工具注册表": "Tool registry",
  "扩展目录": "Extensions folder",
  "安装 Skill": "Install Skill",
  "添加服务器": "Add server",
  "安装插件": "Install plugin",
  "统一工具注册表 · Skill 工作流 · MCP 连接与权限":
    "Unified tool registry · Skill workflows · MCP connections and permissions",
  "Skill 是可复用的工作流说明包，包含 SKILL.md 以及可选脚本和模板。 启用后 Agent 会在任务匹配时先读取说明，再按其中的步骤执行。":
    "Skills are reusable workflow packages with a SKILL.md plus optional scripts and templates. When a task matches, the agent reads the skill first and follows its steps.",
  "还没有安装 Skill": "No Skills installed yet",
  "选择一个包含 SKILL.md 的文件夹导入。你可以从社区下载 Skill， 也可以为自己的项目编写发布、测试或文档流程。":
    "Import a folder containing SKILL.md. Download Skills from the community, or write release, test or documentation workflows for your own projects.",
  "打开 Skill 目录": "Open Skill folder",
  "删除 Skill": "Delete Skill",
  "已停用": "Disabled",
  "MCP 服务器把外部工具接入 Agent。默认每次调用都会请求你的批准； 只有明确开启“自动授权”的服务器才会直接执行。stdio 服务器会在首次 使用时启动，禁用或删除时结束进程。":
    "MCP servers connect external tools to the agent. By default every call asks for your approval; only servers with auto-approve enabled run directly. stdio servers start on first use and stop when disabled or deleted.",
  "还没有配置 MCP 服务器": "No MCP servers configured yet",
  "添加 stdio 命令（例如 npx 启动的 MCP 包）或 Streamable HTTP 服务，保存后刷新工具即可让 Agent 使用。":
    "Add a stdio command (for example an npx MCP package) or a Streamable HTTP service, save it, then refresh tools to make it available to the agent.",
  "自动授权": "Auto approve",
  "刷新工具": "Refresh tools",
  "编辑服务器": "Edit server",
  "删除服务器": "Delete server",
  "尚未加载工具。点击刷新建立连接。":
    "Tools are not loaded yet. Click refresh to connect.",
  "无描述": "No description",
  "插件包用一个 plugin.json 清单同时分发多个 Skill 与 MCP 服务器。 卸载插件会移除它安装的 Skill、MCP 配置和本地副本，不会影响你的项目文件。":
    "A plugin pack distributes multiple Skills and MCP servers through one plugin.json manifest. Uninstalling removes the Skills, MCP configuration and local copy it installed; project files are untouched.",
  "还没有安装插件包": "No plugin packs installed yet",
  "文件夹根目录放置 plugin.json，声明 skills 与 mcpServers 后即可导入。":
    "Put a plugin.json at the folder root declaring skills and mcpServers, then import it.",
  "打开插件目录": "Open plugin folder",
  "卸载插件": "Uninstall plugin",
  "没有提供说明": "No description provided",
  "这是当前 Agent 实际可用的统一工具注册表：内置工具、Skill 加载器与 已启用的 MCP 工具。审批策略在服务端强制执行，不能由模型绕过。":
    "This is the unified tool registry currently available to the agent: built-in tools, the Skill loader and tools from enabled MCP servers. Approval policy is enforced server-side and cannot be bypassed by the model.",
  "内置工具": "Built-in tools",
  "SKILL 工具": "Skill tools",
  "MCP 工具": "MCP tools",
  "需批准": "Approval required",
  "自动": "Automatic",
  "Skill 与 MCP 配置保存在本机应用数据目录；MCP 认证信息会随配置保存在 SQLite 中，请勿在不可信设备上导入未知插件。需要更高安全性时请使用系统凭据 管理器保存的密钥，不要在环境变量中写入长期令牌。":
    "Skill and MCP configuration stays in the local app data folder. MCP credentials are stored with the configuration in SQLite, so do not import unknown plugins on untrusted devices. For stronger security, keep keys in the system credential store and avoid long-lived tokens in environment variables.",
  "编辑 MCP 服务器": "Edit MCP server",
  "添加 MCP 服务器": "Add MCP server",
  "保存": "Save",
  "快速模板": "Quick presets",
  "名称": "Name",
  "例如 docs 或 filesystem": "e.g. docs or filesystem",
  "传输方式": "Transport",
  "stdio（本地进程）": "stdio (local process)",
  "启动命令": "Command",
  "在 Windows 上通过 cmd /C 启动，因此 npx、uvx 等 .cmd 包可以直接使用。":
    "Started through cmd /C on Windows, so .cmd packages such as npx and uvx work directly.",
  "参数（每行一个）": "Arguments (one per line)",
  "环境变量（JSON）": "Environment variables (JSON)",
  "令牌会以明文保存在本机 SQLite 的服务器配置中。":
    "Tokens are stored as plain text in this device's SQLite server configuration.",
  "服务器 URL": "Server URL",
  "请求头（JSON）": "Headers (JSON)",
  "启用服务器": "Enable server",
  "禁用时隐藏其工具并结束已启动的本地进程。":
    "When disabled, its tools are hidden and any running local process is stopped.",
  "自动授权工具调用": "Auto-approve tool calls",
  "开启后该服务器的工具不再逐次请求批准。只对完全信任的服务器启用。":
    "When enabled, this server's tools no longer request approval per call. Only enable it for fully trusted servers.",
  "保存后工具缓存会清空。点击服务器卡片上的刷新按钮重新连接并加载工具。":
    "Saving clears the tool cache. Click refresh on the server card to reconnect and load tools.",
  "将删除": "Delete",
  "及其本地副本。项目文件不会被修改。":
    " and its local copy. Project files are not modified.",
  "的配置、工具缓存并结束进程。":
    " configuration and tool cache, and stop its process.",
  "以及它安装的 Skill 和 MCP 服务器。你的项目文件不会被修改。":
    " and the Skills and MCP servers it installed. Project files are not modified.",
  "卸载": "Uninstall",
  "无法加载扩展": "Unable to load extensions",
  "选择包含 SKILL.md 的文件夹": "Select a folder containing SKILL.md",
  "Skill 已安装": "Skill installed",
  "安装 Skill 失败": "Failed to install Skill",
  "选择包含 plugin.json 的插件文件夹":
    "Select a plugin folder containing plugin.json",
  "插件已安装": "Plugin installed",
  "安装插件失败": "Failed to install plugin",
  "更新 Skill 失败": "Failed to update Skill",
  "删除 Skill 失败": "Failed to delete Skill",
  "MCP 工具已刷新": "MCP tools refreshed",
  "连接 MCP 失败": "Failed to connect to MCP",
  "更新 MCP 服务器失败": "Failed to update MCP server",
  "删除 MCP 服务器失败": "Failed to delete MCP server",
  "卸载插件失败": "Failed to uninstall plugin",
  "读取 MCP 工具失败": "Failed to read MCP tools",
  "请填写 MCP 服务器名称": "Enter an MCP server name",
  "环境变量必须是 JSON 对象": "Environment variables must be a JSON object",
  "请求头必须是 JSON 对象": "Headers must be a JSON object",
  "MCP 服务器已保存": "MCP server saved",
  "点击“刷新工具”建立连接并加载工具":
    "Click Refresh tools to connect and load tools",
  "保存 MCP 服务器失败": "Failed to save MCP server",
  "测试": "Test",
  "关闭": "Close",
  "运行调用": "Run call",
  "调用结果": "Call result",
  "测试调用会直接连接 MCP 服务器并执行 tools/call，不经过 Agent 的逐次审批。请先确认参数不会产生你不希望发生的副作用。":
    "This test connects directly to the MCP server and runs tools/call without the agent's per-call approvals. Check the arguments before triggering side effects you do not want.",
  "工具参数必须是 JSON 对象": "Tool arguments must be a JSON object",
  "环境变量的值必须全部是字符串": "Environment variable values must all be strings",
  "请求头的值必须全部是字符串": "Header values must all be strings",
  "请先启用 MCP 服务器": "Enable the MCP server first",
  "已自动降级到备用供应商": "Automatically fell back to the backup provider",
  "瞬时错误自动重试": "Transient error retried automatically",
  "第 {0} 次": "Attempt {0}",
  "{0} 个 Agent 正在运行": "{0} agents running",
  "{0} 个任务排队中": "{0} tasks queued",
  "排队消息": "Queued messages",
  "附件消息": "Attachment message",
  "停止当前输出并立即发送": "Stop the current response and send now",
  "编辑排队消息": "Edit queued message",
  "删除排队消息": "Delete queued message",
  "插话": "Interrupt",
  "加入队列，本轮结束后发送": "Queue and send after this response",

  "提交": "Commit",
  "Git 检测到 {0} 个未解决的合并冲突。请先在编辑器中解决冲突并标记， 然后再提交。冲突文件不会被自动覆盖。":
    "Git detected {0} unresolved merge conflicts. Resolve and mark them in the editor before committing. Conflicted files are never overwritten automatically.",
  "还原到 HEAD": "Restore to HEAD",
  "提交当前分支的变更": "Commit changes on the current branch",
  "提交全部变更": "Commit all changes",
  "将暂存并提交当前工作区的全部变更（包括未跟踪文件）。提交只发生在本地， 不会自动推送到远程仓库。":
    "Stages and commits all working-tree changes, including untracked files. The commit is local and is never pushed automatically.",
  "描述这次提交的原因和内容…": "Describe the reason and contents of this commit…",
  "还原文件": "Restore file",
  "将把": "Restore",
  "的已暂存和工作区内容都还原为 HEAD 版本。该文件当前未提交的修改会丢失。":
    " staged and working-tree content to HEAD. Uncommitted changes to this file will be lost.",
  "未跟踪文件不会被删除；如需清理请手动删除。":
    "Untracked files are not deleted; remove them manually if needed.",
  "还原中…": "Restoring…",
  "已创建 Git 提交": "Git commit created",
  "已还原到 HEAD": "Restored to HEAD",
  "提交失败": "Commit failed",
  "还原失败": "Restore failed",

  "子代理仍在执行，完成后会显示报告。":
    "The subagent is still running; its report will appear when it finishes.",
  "{0} 轮": "{0} rounds",
  "{0} 次工具调用": "{0} tool calls",
  "子代理 · {0} 个": "Subagents · {0}",

  "planner 子代理进行方案比较与任务拆解时使用的模型":
    "Model used by the planner subagent for option comparison and task breakdown",
  "Agent Runtime 主执行、工具调用与 researcher 子代理使用的模型":
    "Model used by the main Agent Runtime, tool calls and researcher subagents",
  "手动/自动审查与 reviewer 子代理使用的独立模型":
    "Independent model for manual/automatic review and reviewer subagents",
  "打开扩展与工具注册表": "Open extensions and tool registry",
  "故障恢复": "Failure recovery",
  "自动重试": "Automatic retry",
  "遇到网络中断、429 或 5xx 时自动重试。只在尚未输出任何文本前重试， 不会重复生成或重复计费已成功的内容。":
    "Retry on network interruptions, 429 and 5xx. Retries happen only before any text is emitted, so successful output is never duplicated or billed twice.",
  "不重试": "No retry",
  "重试 {0} 次": "Retry {0}×",
  "退避时间": "Backoff",
  "首次重试前的等待时间，之后按指数增长并限制在 15 秒内。":
    "Wait before the first retry; later retries grow exponentially and are capped at 15 seconds.",
  "降级供应商": "Fallback provider",
  "主供应商重试仍失败且没有输出内容时，切换到该供应商完成当前任务。 留空表示不降级，不会在未经配置的情况下产生额外费用。":
    "If the primary provider still fails before emitting content, switch to this provider for the current task. Leave empty to disable fallback so no unconfigured extra cost is incurred.",
  "不启用降级": "Fallback disabled",
  "降级模型": "Fallback model",
  "留空时使用降级供应商的默认聊天模型。":
    "Leave empty to use the fallback provider's default chat model.",
  "重试只针对瞬时错误；认证失败、参数错误和模型不存在不会盲目重试。 若主供应商已经输出部分文本后中断，任务会保留已有输出并报告错误， 不会静默重发导致内容重复。":
    "Only transient errors are retried; authentication, validation and missing-model errors are not. If the primary provider breaks after emitting partial text, existing output is kept and the error is reported instead of silently resending.",
  "独立自动审查": "Independent automatic review",
  "任务完成且存在文件变更后，使用“代码审查”路由的独立模型再检查一遍， 结果会作为一条新回复写入会话。会增加一次模型调用费用，默认关闭。":
    "After a task completes with file changes, run an independent check with the review route model. The result is posted as a new reply in the session. This adds one model call and is off by default.",
  "Git 分支隔离": "Git branch isolation",
  "每个 Agent 任务在 Git 仓库中自动创建并切换到 basalt/任务ID 分支，避免影响当前工作分支。同一项目的任务仍会串行执行。":
    "Each agent task creates and checks out a basalt/task-id branch in Git repositories, keeping the current branch untouched. Tasks in the same project still run serially.",
  "（已禁用）": " (disabled)",
};

const ja: LocaleDictionary = {
  "扩展": "拡張",
  "插件包": "プラグインパック",
  "工具注册表": "ツールレジストリ",
  "扩展目录": "拡張フォルダー",
  "安装 Skill": "Skill を導入",
  "添加服务器": "サーバーを追加",
  "安装插件": "プラグインを導入",
  "统一工具注册表 · Skill 工作流 · MCP 连接与权限":
    "統合ツールレジストリ · Skill ワークフロー · MCP 接続と権限",
  "Skill 是可复用的工作流说明包，包含 SKILL.md 以及可选脚本和模板。 启用后 Agent 会在任务匹配时先读取说明，再按其中的步骤执行。":
    "Skill は SKILL.md と任意のスクリプト・テンプレートを含む再利用可能なワークフローです。タスクが一致すると Agent はまず説明を読み、手順に従って実行します。",
  "还没有安装 Skill": "Skill はまだ導入されていません",
  "选择一个包含 SKILL.md 的文件夹导入。你可以从社区下载 Skill， 也可以为自己的项目编写发布、测试或文档流程。":
    "SKILL.md を含むフォルダーを選択して導入します。コミュニティの Skill を利用するか、自分のプロジェクト用のリリース・テスト・ドキュメント手順を作成できます。",
  "打开 Skill 目录": "Skill フォルダーを開く",
  "删除 Skill": "Skill を削除",
  "已停用": "無効",
  "MCP 服务器把外部工具接入 Agent。默认每次调用都会请求你的批准； 只有明确开启“自动授权”的服务器才会直接执行。stdio 服务器会在首次 使用时启动，禁用或删除时结束进程。":
    "MCP サーバーは外部ツールを Agent に接続します。既定では呼び出しごとに承認を求め、自動承認を有効にしたサーバーだけが直接実行されます。stdio サーバーは初回利用時に起動し、無効化または削除時に終了します。",
  "还没有配置 MCP 服务器": "MCP サーバーはまだ設定されていません",
  "添加 stdio 命令（例如 npx 启动的 MCP 包）或 Streamable HTTP 服务，保存后刷新工具即可让 Agent 使用。":
    "stdio コマンド（npx で起動する MCP パッケージなど）または Streamable HTTP サービスを追加し、保存後にツールを更新すると Agent から利用できます。",
  "自动授权": "自動承認",
  "刷新工具": "ツールを更新",
  "编辑服务器": "サーバーを編集",
  "删除服务器": "サーバーを削除",
  "尚未加载工具。点击刷新建立连接。":
    "ツールはまだ読み込まれていません。更新をクリックして接続してください。",
  "无描述": "説明なし",
  "插件包用一个 plugin.json 清单同时分发多个 Skill 与 MCP 服务器。 卸载插件会移除它安装的 Skill、MCP 配置和本地副本，不会影响你的项目文件。":
    "プラグインパックは 1 つの plugin.json で複数の Skill と MCP サーバーを配布します。アンインストールすると導入された Skill、MCP 設定、ローカルコピーが削除されますが、プロジェクトファイルには影響しません。",
  "还没有安装插件包": "プラグインパックはまだありません",
  "文件夹根目录放置 plugin.json，声明 skills 与 mcpServers 后即可导入。":
    "フォルダーのルートに plugin.json を置き、skills と mcpServers を宣言してから導入してください。",
  "打开插件目录": "プラグインフォルダーを開く",
  "卸载插件": "プラグインをアンインストール",
  "没有提供说明": "説明はありません",
  "这是当前 Agent 实际可用的统一工具注册表：内置工具、Skill 加载器与 已启用的 MCP 工具。审批策略在服务端强制执行，不能由模型绕过。":
    "これは現在 Agent が利用できる統合ツールレジストリです。組み込みツール、Skill ローダー、有効な MCP ツールが含まれます。承認ポリシーはサーバー側で強制され、モデルが回避することはできません。",
  "内置工具": "組み込みツール",
  "SKILL 工具": "Skill ツール",
  "MCP 工具": "MCP ツール",
  "需批准": "承認が必要",
  "自动": "自動",
  "Skill 与 MCP 配置保存在本机应用数据目录；MCP 认证信息会随配置保存在 SQLite 中，请勿在不可信设备上导入未知插件。需要更高安全性时请使用系统凭据 管理器保存的密钥，不要在环境变量中写入长期令牌。":
    "Skill と MCP の設定はローカルのアプリデータに保存されます。MCP の認証情報は設定とともに SQLite に保存されるため、信頼できない端末で不明なプラグインを導入しないでください。より高い安全性が必要な場合はシステムの資格情報ストアを使い、環境変数に長期トークンを置かないでください。",
  "编辑 MCP 服务器": "MCP サーバーを編集",
  "添加 MCP 服务器": "MCP サーバーを追加",
  "保存": "保存",
  "快速模板": "クイックテンプレート",
  "名称": "名前",
  "例如 docs 或 filesystem": "例：docs または filesystem",
  "传输方式": "トランスポート",
  "stdio（本地进程）": "stdio（ローカルプロセス）",
  "启动命令": "起動コマンド",
  "在 Windows 上通过 cmd /C 启动，因此 npx、uvx 等 .cmd 包可以直接使用。":
    "Windows では cmd /C 経由で起動するため、npx や uvx などの .cmd パッケージもそのまま使用できます。",
  "参数（每行一个）": "引数（1 行に 1 つ）",
  "环境变量（JSON）": "環境変数（JSON）",
  "令牌会以明文保存在本机 SQLite 的服务器配置中。":
    "トークンはこの端末の SQLite サーバー設定に平文で保存されます。",
  "服务器 URL": "サーバー URL",
  "请求头（JSON）": "リクエストヘッダー（JSON）",
  "启用服务器": "サーバーを有効化",
  "禁用时隐藏其工具并结束已启动的本地进程。":
    "無効にするとツールが非表示になり、起動中のローカルプロセスは終了します。",
  "自动授权工具调用": "ツール呼び出しを自動承認",
  "开启后该服务器的工具不再逐次请求批准。只对完全信任的服务器启用。":
    "有効にすると、このサーバーのツールは呼び出しごとに承認を求めません。完全に信頼できるサーバーだけに使用してください。",
  "保存后工具缓存会清空。点击服务器卡片上的刷新按钮重新连接并加载工具。":
    "保存するとツールキャッシュが消去されます。サーバーカードの更新をクリックして再接続し、ツールを読み込んでください。",
  "将删除": "削除します：",
  "及其本地副本。项目文件不会被修改。":
    " とローカルコピー。プロジェクトファイルは変更されません。",
  "的配置、工具缓存并结束进程。":
    " の設定とツールキャッシュを削除し、プロセスを終了します。",
  "以及它安装的 Skill 和 MCP 服务器。你的项目文件不会被修改。":
    " および導入された Skill と MCP サーバー。プロジェクトファイルは変更されません。",
  "卸载": "アンインストール",
  "无法加载扩展": "拡張を読み込めません",
  "选择包含 SKILL.md 的文件夹": "SKILL.md を含むフォルダーを選択",
  "Skill 已安装": "Skill を導入しました",
  "安装 Skill 失败": "Skill の導入に失敗しました",
  "选择包含 plugin.json 的插件文件夹":
    "plugin.json を含むプラグインフォルダーを選択",
  "插件已安装": "プラグインを導入しました",
  "安装插件失败": "プラグインの導入に失敗しました",
  "更新 Skill 失败": "Skill の更新に失敗しました",
  "删除 Skill 失败": "Skill の削除に失敗しました",
  "MCP 工具已刷新": "MCP ツールを更新しました",
  "连接 MCP 失败": "MCP への接続に失敗しました",
  "更新 MCP 服务器失败": "MCP サーバーの更新に失敗しました",
  "删除 MCP 服务器失败": "MCP サーバーの削除に失敗しました",
  "卸载插件失败": "プラグインのアンインストールに失敗しました",
  "读取 MCP 工具失败": "MCP ツールの読み込みに失敗しました",
  "请填写 MCP 服务器名称": "MCP サーバー名を入力してください",
  "环境变量必须是 JSON 对象": "環境変数は JSON オブジェクトである必要があります",
  "请求头必须是 JSON 对象": "リクエストヘッダーは JSON オブジェクトである必要があります",
  "MCP 服务器已保存": "MCP サーバーを保存しました",
  "点击“刷新工具”建立连接并加载工具":
    "「ツールを更新」をクリックして接続し、ツールを読み込んでください",
  "保存 MCP 服务器失败": "MCP サーバーの保存に失敗しました",
  "测试": "テスト",
  "关闭": "閉じる",
  "运行调用": "呼び出しを実行",
  "调用结果": "呼び出し結果",
  "测试调用会直接连接 MCP 服务器并执行 tools/call，不经过 Agent 的逐次审批。请先确认参数不会产生你不希望发生的副作用。":
    "テストは MCP サーバーへ直接接続して tools/call を実行し、Agent の都度承認を通過しません。望まない副作用を起こす引数でないか確認してください。",
  "工具参数必须是 JSON 对象": "ツール引数は JSON オブジェクトである必要があります",
  "环境变量的值必须全部是字符串": "環境変数の値はすべて文字列である必要があります",
  "请求头的值必须全部是字符串": "リクエストヘッダーの値はすべて文字列である必要があります",
  "请先启用 MCP 服务器": "先に MCP サーバーを有効化してください",
  "已自动降级到备用供应商": "予備プロバイダーへ自動フォールバックしました",
  "瞬时错误自动重试": "一時的なエラーを自動再試行しました",
  "第 {0} 次": "{0} 回目",
  "{0} 个 Agent 正在运行": "{0} 個の Agent 実行中",
  "{0} 个任务排队中": "{0} 件のタスク待機中",
  "排队消息": "キュー内のメッセージ",
  "附件消息": "添付メッセージ",
  "停止当前输出并立即发送": "現在の出力を停止して即送信",
  "编辑排队消息": "キューのメッセージを編集",
  "删除排队消息": "キューのメッセージを削除",
  "插话": "割り込み",
  "加入队列，本轮结束后发送": "キューに追加し、この応答後に送信",

  "提交": "コミット",
  "Git 检测到 {0} 个未解决的合并冲突。请先在编辑器中解决冲突并标记， 然后再提交。冲突文件不会被自动覆盖。":
    "Git が {0} 件の未解決のマージ競合を検出しました。エディターで解決してマークしてからコミットしてください。競合ファイルが自動で上書きされることはありません。",
  "还原到 HEAD": "HEAD に戻す",
  "提交当前分支的变更": "現在のブランチの変更をコミット",
  "提交全部变更": "すべての変更をコミット",
  "将暂存并提交当前工作区的全部变更（包括未跟踪文件）。提交只发生在本地， 不会自动推送到远程仓库。":
    "未追跡ファイルを含むワークツリーのすべての変更をステージしてコミットします。コミットはローカルのみで、自動的にリモートへプッシュされません。",
  "描述这次提交的原因和内容…": "このコミットの理由と内容を入力…",
  "还原文件": "ファイルを戻す",
  "将把": "復元します：",
  "的已暂存和工作区内容都还原为 HEAD 版本。该文件当前未提交的修改会丢失。":
    " のステージ済みおよびワークツリーの内容を HEAD に戻します。このファイルの未コミット変更は失われます。",
  "未跟踪文件不会被删除；如需清理请手动删除。":
    "未追跡ファイルは削除されません。必要に応じて手動で削除してください。",
  "还原中…": "復元中…",
  "已创建 Git 提交": "Git コミットを作成しました",
  "已还原到 HEAD": "HEAD に戻しました",
  "提交失败": "コミットに失敗しました",
  "还原失败": "復元に失敗しました",

  "子代理仍在执行，完成后会显示报告。":
    "サブエージェントはまだ実行中です。完了後にレポートが表示されます。",
  "{0} 轮": "{0} ラウンド",
  "{0} 次工具调用": "{0} 回のツール呼び出し",
  "子代理 · {0} 个": "サブエージェント · {0} 件",

  "planner 子代理进行方案比较与任务拆解时使用的模型":
    "planner サブエージェントが案の比較とタスク分割に使うモデル",
  "Agent Runtime 主执行、工具调用与 researcher 子代理使用的模型":
    "メイン Agent Runtime、ツール呼び出し、researcher サブエージェントが使うモデル",
  "手动/自动审查与 reviewer 子代理使用的独立模型":
    "手動/自動レビューと reviewer サブエージェント用の独立モデル",
  "打开扩展与工具注册表": "拡張とツールレジストリを開く",
  "故障恢复": "障害回復",
  "自动重试": "自動再試行",
  "遇到网络中断、429 或 5xx 时自动重试。只在尚未输出任何文本前重试， 不会重复生成或重复计费已成功的内容。":
    "ネットワーク切断、429、5xx で自動再試行します。テキスト出力前のみ再試行するため、成功した内容が重複生成・二重課金されることはありません。",
  "不重试": "再試行しない",
  "重试 {0} 次": "{0} 回再試行",
  "退避时间": "バックオフ",
  "首次重试前的等待时间，之后按指数增长并限制在 15 秒内。":
    "最初の再試行前の待機時間。以降は指数関数的に増え、最大 15 秒です。",
  "降级供应商": "フォールバックプロバイダー",
  "主供应商重试仍失败且没有输出内容时，切换到该供应商完成当前任务。 留空表示不降级，不会在未经配置的情况下产生额外费用。":
    "主要プロバイダーが出力前に失敗し続けた場合、このプロバイダーで現在のタスクを続行します。空欄ならフォールバックせず、未設定の追加費用は発生しません。",
  "不启用降级": "フォールバックなし",
  "降级模型": "フォールバックモデル",
  "留空时使用降级供应商的默认聊天模型。":
    "空欄の場合はフォールバックプロバイダーの既定チャットモデルを使います。",
  "重试只针对瞬时错误；认证失败、参数错误和模型不存在不会盲目重试。 若主供应商已经输出部分文本后中断，任务会保留已有输出并报告错误， 不会静默重发导致内容重复。":
    "再試行は一時的なエラーのみが対象です。認証・パラメータ・モデル不在のエラーは再試行しません。部分的なテキスト出力後に中断した場合は既存の出力を保持してエラーを報告し、黙って再送信して重複させません。",
  "独立自动审查": "独立自動レビュー",
  "任务完成且存在文件变更后，使用“代码审查”路由的独立模型再检查一遍， 结果会作为一条新回复写入会话。会增加一次模型调用费用，默认关闭。":
    "ファイル変更のあるタスク完了後、コードレビュールートの独立モデルで再確認し、結果を新しい返信としてセッションに追加します。モデル呼び出しが 1 回増えるため、既定では無効です。",
  "Git 分支隔离": "Git ブランチ分離",
  "每个 Agent 任务在 Git 仓库中自动创建并切换到 basalt/任务ID 分支，避免影响当前工作分支。同一项目的任务仍会串行执行。":
    "各 Agent タスクは Git リポジトリで basalt/タスクID ブランチを作成して切り替え、現在の作業ブランチに影響しません。同じプロジェクトのタスクは引き続き直列実行されます。",
  "（已禁用）": "（無効）",
};

const ko: LocaleDictionary = {
  "扩展": "확장",
  "插件包": "플러그인 팩",
  "工具注册表": "도구 레지스트리",
  "扩展目录": "확장 폴더",
  "安装 Skill": "Skill 설치",
  "添加服务器": "서버 추가",
  "安装插件": "플러그인 설치",
  "统一工具注册表 · Skill 工作流 · MCP 连接与权限":
    "통합 도구 레지스트리 · Skill 워크플로 · MCP 연결 및 권한",
  "Skill 是可复用的工作流说明包，包含 SKILL.md 以及可选脚本和模板。 启用后 Agent 会在任务匹配时先读取说明，再按其中的步骤执行。":
    "Skill은 SKILL.md와 선택적 스크립트·템플릿을 포함하는 재사용 가능한 워크플로 패키지입니다. 작업이 일치하면 Agent가 먼저 설명을 읽고 절차를 따릅니다.",
  "还没有安装 Skill": "아직 설치된 Skill이 없습니다",
  "选择一个包含 SKILL.md 的文件夹导入。你可以从社区下载 Skill， 也可以为自己的项目编写发布、测试或文档流程。":
    "SKILL.md가 포함된 폴더를 선택해 가져오세요. 커뮤니티 Skill을 내려받거나 프로젝트용 릴리스·테스트·문서 절차를 직접 작성할 수 있습니다.",
  "打开 Skill 目录": "Skill 폴더 열기",
  "删除 Skill": "Skill 삭제",
  "已停用": "비활성화됨",
  "MCP 服务器把外部工具接入 Agent。默认每次调用都会请求你的批准； 只有明确开启“自动授权”的服务器才会直接执行。stdio 服务器会在首次 使用时启动，禁用或删除时结束进程。":
    "MCP 서버는 외부 도구를 Agent에 연결합니다. 기본적으로 호출마다 승인을 요청하며, 자동 승인을 켠 서버만 바로 실행됩니다. stdio 서버는 처음 사용할 때 시작하고 비활성화하거나 삭제하면 종료됩니다.",
  "还没有配置 MCP 服务器": "아직 구성된 MCP 서버가 없습니다",
  "添加 stdio 命令（例如 npx 启动的 MCP 包）或 Streamable HTTP 服务，保存后刷新工具即可让 Agent 使用。":
    "stdio 명령(npx로 실행하는 MCP 패키지 등) 또는 Streamable HTTP 서비스를 추가하고 저장한 뒤 도구를 새로 고치면 Agent가 사용할 수 있습니다.",
  "自动授权": "자동 승인",
  "刷新工具": "도구 새로 고침",
  "编辑服务器": "서버 편집",
  "删除服务器": "서버 삭제",
  "尚未加载工具。点击刷新建立连接。":
    "아직 도구를 불러오지 않았습니다. 새로 고침을 눌러 연결하세요.",
  "无描述": "설명 없음",
  "插件包用一个 plugin.json 清单同时分发多个 Skill 与 MCP 服务器。 卸载插件会移除它安装的 Skill、MCP 配置和本地副本，不会影响你的项目文件。":
    "플러그인 팩은 하나의 plugin.json으로 여러 Skill과 MCP 서버를 배포합니다. 제거하면 설치된 Skill, MCP 구성, 로컬 복사본이 삭제되지만 프로젝트 파일은 변경되지 않습니다.",
  "还没有安装插件包": "아직 설치된 플러그인 팩이 없습니다",
  "文件夹根目录放置 plugin.json，声明 skills 与 mcpServers 后即可导入。":
    "폴더 루트에 plugin.json을 두고 skills와 mcpServers를 선언한 뒤 가져오세요.",
  "打开插件目录": "플러그인 폴더 열기",
  "卸载插件": "플러그인 제거",
  "没有提供说明": "설명이 없습니다",
  "这是当前 Agent 实际可用的统一工具注册表：内置工具、Skill 加载器与 已启用的 MCP 工具。审批策略在服务端强制执行，不能由模型绕过。":
    "현재 Agent가 실제로 사용할 수 있는 통합 도구 레지스트리입니다. 내장 도구, Skill 로더, 활성화된 MCP 도구가 포함됩니다. 승인 정책은 서버에서 강제되며 모델이 우회할 수 없습니다.",
  "内置工具": "내장 도구",
  "SKILL 工具": "Skill 도구",
  "MCP 工具": "MCP 도구",
  "需批准": "승인 필요",
  "自动": "자동",
  "Skill 与 MCP 配置保存在本机应用数据目录；MCP 认证信息会随配置保存在 SQLite 中，请勿在不可信设备上导入未知插件。需要更高安全性时请使用系统凭据 管理器保存的密钥，不要在环境变量中写入长期令牌。":
    "Skill 및 MCP 구성은 로컬 앱 데이터 폴더에 저장됩니다. MCP 인증 정보는 구성과 함께 SQLite에 저장되므로 신뢰할 수 없는 기기에서 알 수 없는 플러그인을 가져오지 마세요. 보안이 더 필요하면 시스템 자격 증명 저장소를 사용하고 환경 변수에 장기 토큰을 넣지 마세요.",
  "编辑 MCP 服务器": "MCP 서버 편집",
  "添加 MCP 服务器": "MCP 서버 추가",
  "保存": "저장",
  "快速模板": "빠른 템플릿",
  "名称": "이름",
  "例如 docs 或 filesystem": "예: docs 또는 filesystem",
  "传输方式": "전송 방식",
  "stdio（本地进程）": "stdio(로컬 프로세스)",
  "启动命令": "실행 명령",
  "在 Windows 上通过 cmd /C 启动，因此 npx、uvx 等 .cmd 包可以直接使用。":
    "Windows에서는 cmd /C로 실행하므로 npx, uvx 같은 .cmd 패키지를 바로 사용할 수 있습니다.",
  "参数（每行一个）": "인수(한 줄에 하나)",
  "环境变量（JSON）": "환경 변수(JSON)",
  "令牌会以明文保存在本机 SQLite 的服务器配置中。":
    "토큰은 이 기기의 SQLite 서버 구성에 평문으로 저장됩니다.",
  "服务器 URL": "서버 URL",
  "请求头（JSON）": "요청 헤더(JSON)",
  "启用服务器": "서버 활성화",
  "禁用时隐藏其工具并结束已启动的本地进程。":
    "비활성화하면 도구가 숨겨지고 실행 중인 로컬 프로세스가 종료됩니다.",
  "自动授权工具调用": "도구 호출 자동 승인",
  "开启后该服务器的工具不再逐次请求批准。只对完全信任的服务器启用。":
    "켜면 이 서버의 도구는 호출마다 승인을 요청하지 않습니다. 완전히 신뢰하는 서버에만 사용하세요.",
  "保存后工具缓存会清空。点击服务器卡片上的刷新按钮重新连接并加载工具。":
    "저장하면 도구 캐시가 지워집니다. 서버 카드의 새로 고침을 눌러 다시 연결하고 도구를 불러오세요.",
  "将删除": "삭제:",
  "及其本地副本。项目文件不会被修改。":
    " 및 로컬 복사본. 프로젝트 파일은 변경되지 않습니다.",
  "的配置、工具缓存并结束进程。":
    " 구성과 도구 캐시를 삭제하고 프로세스를 종료합니다.",
  "以及它安装的 Skill 和 MCP 服务器。你的项目文件不会被修改。":
    " 및 설치된 Skill과 MCP 서버. 프로젝트 파일은 변경되지 않습니다.",
  "卸载": "제거",
  "无法加载扩展": "확장을 불러올 수 없습니다",
  "选择包含 SKILL.md 的文件夹": "SKILL.md가 포함된 폴더 선택",
  "Skill 已安装": "Skill 설치됨",
  "安装 Skill 失败": "Skill 설치 실패",
  "选择包含 plugin.json 的插件文件夹":
    "plugin.json이 포함된 플러그인 폴더 선택",
  "插件已安装": "플러그인 설치됨",
  "安装插件失败": "플러그인 설치 실패",
  "更新 Skill 失败": "Skill 업데이트 실패",
  "删除 Skill 失败": "Skill 삭제 실패",
  "MCP 工具已刷新": "MCP 도구 새로 고침 완료",
  "连接 MCP 失败": "MCP 연결 실패",
  "更新 MCP 服务器失败": "MCP 서버 업데이트 실패",
  "删除 MCP 服务器失败": "MCP 서버 삭제 실패",
  "卸载插件失败": "플러그인 제거 실패",
  "读取 MCP 工具失败": "MCP 도구 읽기 실패",
  "请填写 MCP 服务器名称": "MCP 서버 이름을 입력하세요",
  "环境变量必须是 JSON 对象": "환경 변수는 JSON 객체여야 합니다",
  "请求头必须是 JSON 对象": "요청 헤더는 JSON 객체여야 합니다",
  "MCP 服务器已保存": "MCP 서버 저장됨",
  "点击“刷新工具”建立连接并加载工具":
    "‘도구 새로 고침’을 눌러 연결하고 도구를 불러오세요",
  "保存 MCP 服务器失败": "MCP 서버 저장 실패",
  "测试": "테스트",
  "关闭": "닫기",
  "运行调用": "호출 실행",
  "调用结果": "호출 결과",
  "测试调用会直接连接 MCP 服务器并执行 tools/call，不经过 Agent 的逐次审批。请先确认参数不会产生你不希望发生的副作用。":
    "테스트는 MCP 서버에 직접 연결해 tools/call을 실행하며 Agent의 호출별 승인을 거치지 않습니다. 원하지 않는 부작용을 일으키는 인수가 아닌지 확인하세요.",
  "工具参数必须是 JSON 对象": "도구 인수는 JSON 객체여야 합니다",
  "环境变量的值必须全部是字符串": "환경 변수 값은 모두 문자열이어야 합니다",
  "请求头的值必须全部是字符串": "요청 헤더 값은 모두 문자열이어야 합니다",
  "请先启用 MCP 服务器": "먼저 MCP 서버를 활성화하세요",
  "已自动降级到备用供应商": "대체 공급자로 자동 전환했습니다",
  "瞬时错误自动重试": "일시적 오류를 자동 재시도했습니다",
  "第 {0} 次": "{0}회차",
  "{0} 个 Agent 正在运行": "Agent {0}개 실행 중",
  "{0} 个任务排队中": "작업 {0}개 대기 중",
  "排队消息": "대기 중인 메시지",
  "附件消息": "첨부 메시지",
  "停止当前输出并立即发送": "현재 출력을 멈추고 바로 전송",
  "编辑排队消息": "대기 메시지 편집",
  "删除排队消息": "대기 메시지 삭제",
  "插话": "끼어들기",
  "加入队列，本轮结束后发送": "대기열에 추가하고 이번 응답 후 전송",

  "提交": "커밋",
  "Git 检测到 {0} 个未解决的合并冲突。请先在编辑器中解决冲突并标记， 然后再提交。冲突文件不会被自动覆盖。":
    "Git에서 해결되지 않은 병합 충돌 {0}개를 감지했습니다. 편집기에서 해결하고 표시한 뒤 커밋하세요. 충돌 파일은 자동으로 덮어쓰지 않습니다.",
  "还原到 HEAD": "HEAD로 복원",
  "提交当前分支的变更": "현재 브랜치의 변경 사항 커밋",
  "提交全部变更": "모든 변경 사항 커밋",
  "将暂存并提交当前工作区的全部变更（包括未跟踪文件）。提交只发生在本地， 不会自动推送到远程仓库。":
    "추적되지 않은 파일을 포함해 작업 트리의 모든 변경을 스테이징하고 커밋합니다. 커밋은 로컬에서만 이루어지며 자동으로 원격에 푸시되지 않습니다.",
  "描述这次提交的原因和内容…": "이 커밋의 이유와 내용을 입력하세요…",
  "还原文件": "파일 복원",
  "将把": "복원:",
  "的已暂存和工作区内容都还原为 HEAD 版本。该文件当前未提交的修改会丢失。":
    " 의 스테이징 및 작업 트리 내용을 HEAD로 되돌립니다. 이 파일의 커밋되지 않은 변경은 사라집니다.",
  "未跟踪文件不会被删除；如需清理请手动删除。":
    "추적되지 않은 파일은 삭제되지 않습니다. 필요하면 직접 삭제하세요.",
  "还原中…": "복원 중…",
  "已创建 Git 提交": "Git 커밋 생성됨",
  "已还原到 HEAD": "HEAD로 복원됨",
  "提交失败": "커밋 실패",
  "还原失败": "복원 실패",

  "子代理仍在执行，完成后会显示报告。":
    "하위 에이전트가 아직 실행 중입니다. 완료되면 보고서가 표시됩니다.",
  "{0} 轮": "{0} 라운드",
  "{0} 次工具调用": "도구 호출 {0}회",
  "子代理 · {0} 个": "하위 에이전트 · {0}개",

  "planner 子代理进行方案比较与任务拆解时使用的模型":
    "planner 하위 에이전트가 방안 비교와 작업 분해에 사용하는 모델",
  "Agent Runtime 主执行、工具调用与 researcher 子代理使用的模型":
    "기본 Agent Runtime, 도구 호출, researcher 하위 에이전트가 사용하는 모델",
  "手动/自动审查与 reviewer 子代理使用的独立模型":
    "수동/자동 검토와 reviewer 하위 에이전트용 독립 모델",
  "打开扩展与工具注册表": "확장 및 도구 레지스트리 열기",
  "故障恢复": "장애 복구",
  "自动重试": "자동 재시도",
  "遇到网络中断、429 或 5xx 时自动重试。只在尚未输出任何文本前重试， 不会重复生成或重复计费已成功的内容。":
    "네트워크 중단, 429, 5xx에서 자동 재시도합니다. 텍스트가 출력되기 전에만 재시도하므로 성공한 내용을 중복 생성하거나 이중 과금하지 않습니다.",
  "不重试": "재시도 안 함",
  "重试 {0} 次": "{0}회 재시도",
  "退避时间": "백오프",
  "首次重试前的等待时间，之后按指数增长并限制在 15 秒内。":
    "첫 재시도 전 대기 시간입니다. 이후 지수적으로 늘어나며 최대 15초입니다.",
  "降级供应商": "대체 공급자",
  "主供应商重试仍失败且没有输出内容时，切换到该供应商完成当前任务。 留空表示不降级，不会在未经配置的情况下产生额外费用。":
    "주 공급자가 출력 전에 계속 실패하면 이 공급자로 전환해 현재 작업을 완료합니다. 비워 두면 대체하지 않으며 구성하지 않은 추가 비용이 발생하지 않습니다.",
  "不启用降级": "대체 사용 안 함",
  "降级模型": "대체 모델",
  "留空时使用降级供应商的默认聊天模型。":
    "비워 두면 대체 공급자의 기본 채팅 모델을 사용합니다.",
  "重试只针对瞬时错误；认证失败、参数错误和模型不存在不会盲目重试。 若主供应商已经输出部分文本后中断，任务会保留已有输出并报告错误， 不会静默重发导致内容重复。":
    "일시적 오류만 재시도합니다. 인증, 매개변수, 모델 없음 오류는 재시도하지 않습니다. 일부 텍스트 출력 후 중단되면 기존 출력을 유지하고 오류를 보고하며, 조용히 다시 보내 중복시키지 않습니다.",
  "独立自动审查": "독립 자동 검토",
  "任务完成且存在文件变更后，使用“代码审查”路由的独立模型再检查一遍， 结果会作为一条新回复写入会话。会增加一次模型调用费用，默认关闭。":
    "파일 변경이 있는 작업이 완료되면 코드 검토 라우트의 독립 모델로 다시 확인하고 결과를 새 답변으로 세션에 기록합니다. 모델 호출이 한 번 추가되며 기본적으로 꺼져 있습니다.",
  "Git 分支隔离": "Git 브랜치 격리",
  "每个 Agent 任务在 Git 仓库中自动创建并切换到 basalt/任务ID 分支，避免影响当前工作分支。同一项目的任务仍会串行执行。":
    "각 Agent 작업은 Git 저장소에서 basalt/작업ID 브랜치를 만들어 전환하므로 현재 작업 브랜치에 영향을 주지 않습니다. 같은 프로젝트의 작업은 계속 순차 실행됩니다.",
  "（已禁用）": " (비활성화됨)",
};

export const extensionDictionaries: Record<
  Exclude<Language, "zh-CN">,
  LocaleDictionary
> = {
  "en-US": en,
  "ja-JP": ja,
  "ko-KR": ko,
};
