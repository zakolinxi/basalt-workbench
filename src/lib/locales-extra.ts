import type { Language } from "./types";

type LocaleDictionary = Record<string, string>;

const en: LocaleDictionary = {
  "刚刚": "just now",
  "{0} 分钟前": "{0} min ago",
  "{0} 小时前": "{0} hr ago",
  "{0} 天前": "{0} days ago",
  "已中断": "Interrupted",
  "排队中": "Queued",
  "已完成": "Completed",
  "，其中 {0} 次失败": ", {0} failed",
  "，{0} 次取消": ", {0} cancelled",
  "，正在执行": ", running",
  "执行工具 {0} 次": "Tools run: {0}",
  "执行工具 {0} 次，其中 {1} 次失败": "Tools run: {0}, {1} failed",
  "执行工具 {0} 次，{1} 次取消": "Tools run: {0}, {1} cancelled",
  "执行工具 {0} 次，正在执行": "Tools run: {0}, running",
  "执行工具 {0} 次，其中 {1} 次失败，正在执行":
    "Tools run: {0}, {1} failed, running",
  "执行工具 {0} 次，{1} 次取消，正在执行":
    "Tools run: {0}, {1} cancelled, running",
  "从变更面板打开文件": "Open a file from Changes",
  "CTRL+S 保存 · READ-ONLY 预览": "CTRL+S SAVE · READ-ONLY PREVIEW",
  "没有打开的终端": "No open terminal",
  "新建 PowerShell": "New PowerShell",
  "Agent 执行的命令会记录在任务时间线中":
    "Agent commands are recorded in the task timeline",
  "图像工作区": "Image studio",
  "GitHub 仓库": "GitHub repository",
  "主题：深色": "Theme: Dark",
  "主题：浅色": "Theme: Light",
  "主题：跟随系统": "Theme: System",
  "切换主题": "Toggle theme",
  "当前工作区": "Current workspace",
  "折叠会话": "Collapse sessions",
  "所有数据保存在本地": "All data stays local",
  "收起侧栏": "Collapse sidebar",
  "打开终端": "Open terminal",
  "收起右侧面板": "Collapse right panel",
  "普通对话：不会操作文件或执行命令":
    "Chat: no file access or command execution",
  "Agent：在授权项目中执行编程任务":
    "Agent: run coding tasks in the authorized project",
  "审批策略：完全访问（对后续 Agent 任务生效）":
    "Approval policy: Full access (applies to new Agent tasks)",
  "审批策略：每次询问（对后续 Agent 任务生效）":
    "Approval policy: Ask every time (applies to new Agent tasks)",
  "审批策略：自动审批（对后续 Agent 任务生效）":
    "Approval policy: Auto approve (applies to new Agent tasks)",
  "工具调用能力尚未经过真实测试":
    "Tool calling has not been verified with a real request",
  "打开 Diff": "Open Diff",
  "新建终端": "New terminal",
  "选择终端类型": "Choose terminal type",
  "搜索输出": "Search output",
  "复制选中内容": "Copy selection",
  "清屏": "Clear",
  "终止当前进程": "Terminate current process",
  "收起终端": "Collapse terminal",
  "最小化": "Minimize",
  "最大化": "Maximize",
  "还原": "Restore",
  "模型路由 · 审批策略 · 终端与界面":
    "Model routing · Approval policy · Terminal and interface",
  "数据目录": "Data folder",
  "保存设置": "Save settings",
  "外观与编辑器": "Appearance and editor",
  "深色与浅色使用同一套设计 Token；跟随系统会实时响应 Windows 主题。":
    "Dark and light share the same design tokens; System follows the Windows theme live.",
  "深色": "Dark",
  "浅色": "Light",
  "跟随系统": "System",
  "整套界面的背景、表面与强调色都会随配色变化；切换时颜色平滑过渡。":
    "The page background, surfaces and accent change together; switching is animated smoothly.",
  "玄武": "Basalt",
  "森林": "Forest",
  "薄荷": "Mint",
  "蜜蜂": "Honey",
  "羊皮纸": "Parchment",
  "界面语言": "Interface language",
  "切换界面文案；模型回复、代码与终端输出保持原样。":
    "Switch interface text only; model replies, code and terminal output stay unchanged.",
  "界面圆角": "Corner style",
  "立即切换全局直角或圆角风格，偏好会记录在本机，下次启动继续生效。":
    "Switch between sharp and rounded corners. The preference is saved locally.",
  "直角": "Sharp",
  "圆角": "Rounded",
  "终端 Shell": "Terminal shell",
  "新终端默认使用；已有的终端不会被重启。":
    "Used for new terminals; existing terminals are not restarted.",
  "WSL（若已安装）": "WSL (if installed)",
  "编辑器字号": "Editor font size",
  "Monaco 编辑器字体大小（10–24 px）。": "Monaco editor font size (10–24 px).",
  "对话字号": "Chat font size",
  "消息正文与输入框字号（11–22 px）。": "Message and composer font size (11–22 px).",
  "模型路由": "Model routing",
  "不同用途可以指向不同模型。路由只在已配置供应商的模型列表中推荐， 也允许手动填写自定义模型 ID。未授权的自动升级或收费更高的回退不会发生。":
    "Different purposes can use different models. Routing suggests models only from configured providers and allows a custom model ID. No unauthorized upgrades or costlier fallbacks are performed.",
  "默认聊天": "Default chat",
  "普通对话与快捷问题的默认路由": "Default route for chat and quick questions",
  "未设置": "Not set",
  "Agent 规划": "Agent planning",
  "用于任务规划（当前执行器使用 Agent 执行模型）":
    "Used for task planning (the executor currently uses the Agent execution model)",
  "Agent 执行": "Agent execution",
  "Agent Runtime 调用工具与执行任务使用的模型":
    "Model used by Agent Runtime for tools and task execution",
  "代码审查": "Code review",
  "审查 Diff 与结果时使用的模型": "Model used to review diffs and results",
  "视觉理解": "Vision",
  "处理图片理解任务的默认模型": "Default model for image understanding",
  "图像生成": "Image generation",
  "对话内生图与图像工作区的默认生成路由":
    "Default generation route for in-chat and image studio work",
  "图像编辑": "Image editing",
  "参考图编辑的默认路由；未设置时回退到图像生成路由":
    "Default route for reference-image edits; falls back to image generation",
  "打开供应商管理": "Open provider management",
  "Agent 与审批": "Agent and approvals",
  "审批策略": "Approval policy",
  "每次询问：写入与命令都暂停；自动审批：文件写入自动执行，命令仍需批准； 完全访问：写入与命令都自动执行。危险命令始终被权限策略硬拒绝。":
    "Ask every time: writes and commands pause; Auto approve: writes run automatically but commands still need approval; Full access: writes and commands run automatically. Dangerous commands are always denied by policy.",
  "每次询问": "Ask every time",
  "自动审批": "Auto approve",
  "完全访问已开启：Agent 的文件写入与终端命令都会自动执行。危险命令仍会被安全策略拒绝，请在受控项目中运行 Agent。":
    "Full access is on: Agent writes and terminal commands run automatically. Dangerous commands are still denied; run Agent in a controlled project.",
  "无论开关状态，路径越界、UNC 网络路径、注册表、提权、格式化与下载执行等 高风险命令都在权限策略中被硬拒绝。":
    "Regardless of these switches, path escapes, UNC paths, registry changes, privilege escalation, formatting and download-and-execute commands are hard-denied by policy.",
  "安全与数据": "Security and data",
  "请求 User-Agent": "Request User-Agent",
  "所有供应商请求使用的 UA；个别网关会拦截非浏览器 UA。留空则回退到内置标识。":
    "User-Agent for all provider requests; some gateways reject non-browser agents. Leave empty to use the built-in value.",
  "API Key 存储": "API key storage",
  "API Key 写入 Windows 凭据管理器，不进入 SQLite、配置文件或普通日志。 普通日志也会过滤疑似令牌字符串。":
    "API keys are stored in Windows Credential Manager, never in SQLite, config files or normal logs. Logs also redact token-like strings.",
  "项目边界": "Project boundary",
  "Agent 只能通过相对路径访问当前授权的项目目录；路径穿越、符号链接逃逸、 .git 写入与项目外绝对路径写入会被阻止。命令中的项目外绝对路径会被拒绝。":
    "Agent can only reach the authorized project through relative paths. Traversal, symlink escapes, .git writes and absolute writes outside the project are blocked. Absolute paths outside the project in commands are rejected.",
  "进程与审批": "Processes and approvals",
  "Agent 命令带超时、取消与进程树终止；用户终端与 Agent 命令分开记录。 命令不通过字符串拼接注入；模型不能直接获得系统 Shell。":
    "Agent commands have timeouts, cancellation and process-tree termination; user terminals and Agent commands are tracked separately. Commands are not injected through string concatenation, and the model cannot access a system shell directly.",
  "快捷键": "Shortcuts",
  "新建 Agent 任务": "New Agent task",
  "切换侧栏": "Toggle sidebar",
  "切换终端": "Toggle terminal",
  "保存当前文件（编辑模式）": "Save current file (edit mode)",
  "发送 / 换行": "Send / newline",
  "保存全部设置": "Save all settings",
  "文件写入与终端命令都会暂停，等待你逐项批准。":
    "Writes and terminal commands pause for approval.",
  "文件写入自动执行，终端命令仍需你批准。":
    "Writes run automatically; terminal commands still need approval.",
  "文件写入与终端命令自动执行；危险命令仍由安全策略强制拒绝。":
    "Writes and terminal commands run automatically; dangerous commands are still denied.",
  "思考强度：高（更多思考预算，适合复杂任务）":
    "Reasoning effort: High (more budget for complex tasks)",
};

const ja: LocaleDictionary = {
  "刚刚": "たった今",
  "{0} 分钟前": "{0} 分前",
  "{0} 小时前": "{0} 時間前",
  "{0} 天前": "{0} 日前",
  "已中断": "中断",
  "排队中": "待機中",
  "已完成": "完了",
  "，其中 {0} 次失败": "、{0} 件失敗",
  "，{0} 次取消": "、{0} 件キャンセル",
  "，正在执行": "、実行中",
  "执行工具 {0} 次": "ツール実行 {0} 回",
  "执行工具 {0} 次，其中 {1} 次失败": "ツール実行 {0} 回、{1} 件失敗",
  "执行工具 {0} 次，{1} 次取消": "ツール実行 {0} 回、{1} 件キャンセル",
  "执行工具 {0} 次，正在执行": "ツール実行 {0} 回、実行中",
  "执行工具 {0} 次，其中 {1} 次失败，正在执行":
    "ツール実行 {0} 回、{1} 件失敗、実行中",
  "执行工具 {0} 次，{1} 次取消，正在执行":
    "ツール実行 {0} 回、{1} 件キャンセル、実行中",
  "从变更面板打开文件": "変更パネルからファイルを開く",
  "CTRL+S 保存 · READ-ONLY 预览": "CTRL+S 保存 · READ-ONLY プレビュー",
  "没有打开的终端": "開いているターミナルはありません",
  "新建 PowerShell": "PowerShell を新規作成",
  "Agent 执行的命令会记录在任务时间线中":
    "エージェントのコマンドはタスクタイムラインに記録されます",
  "图像工作区": "画像ワークスペース",
  "GitHub 仓库": "GitHub リポジトリ",
  "主题：深色": "テーマ：ダーク",
  "主题：浅色": "テーマ：ライト",
  "主题：跟随系统": "テーマ：システム",
  "切换主题": "テーマを切り替え",
  "当前工作区": "現在のワークスペース",
  "折叠会话": "会話を折りたたむ",
  "所有数据保存在本地": "すべてのデータはローカルに保存されます",
  "收起侧栏": "サイドバーを閉じる",
  "打开终端": "ターミナルを開く",
  "收起右侧面板": "右パネルを閉じる",
  "普通对话：不会操作文件或执行命令":
    "チャット：ファイル操作やコマンド実行は行いません",
  "Agent：在授权项目中执行编程任务":
    "エージェント：許可されたプロジェクトでコーディングタスクを実行",
  "审批策略：完全访问（对后续 Agent 任务生效）":
    "承認ポリシー：完全アクセス（新しいエージェントタスクに適用）",
  "审批策略：每次询问（对后续 Agent 任务生效）":
    "承認ポリシー：毎回確認（新しいエージェントタスクに適用）",
  "审批策略：自动审批（对后续 Agent 任务生效）":
    "承認ポリシー：自動承認（新しいエージェントタスクに適用）",
  "工具调用能力尚未经过真实测试":
    "ツール呼び出しは実際のリクエストで検証されていません",
  "打开 Diff": "Diff を開く",
  "新建终端": "新しいターミナル",
  "选择终端类型": "ターミナルの種類を選択",
  "搜索输出": "出力を検索",
  "复制选中内容": "選択内容をコピー",
  "清屏": "クリア",
  "终止当前进程": "現在のプロセスを終了",
  "收起终端": "ターミナルを閉じる",
  "最小化": "最小化",
  "最大化": "最大化",
  "还原": "元に戻す",
  "模型路由 · 审批策略 · 终端与界面":
    "モデルルーティング · 承認ポリシー · ターミナルと画面",
  "数据目录": "データフォルダー",
  "保存设置": "設定を保存",
  "外观与编辑器": "外観とエディター",
  "深色与浅色使用同一套设计 Token；跟随系统会实时响应 Windows 主题。":
    "ダークとライトは同じデザイントークンを使用し、システムは Windows テーマに追従します。",
  "深色": "ダーク",
  "浅色": "ライト",
  "跟随系统": "システム",
  "整套界面的背景、表面与强调色都会随配色变化；切换时颜色平滑过渡。":
    "背景、サーフェス、アクセントが配色に合わせて変化し、切り替えは滑らかにアニメーションします。",
  "玄武": "玄武",
  "森林": "フォレスト",
  "薄荷": "ミント",
  "蜜蜂": "ハニー",
  "羊皮纸": "パーチメント",
  "界面语言": "表示言語",
  "切换界面文案；模型回复、代码与终端输出保持原样。":
    "UI の文言のみを切り替えます。モデルの返答、コード、ターミナル出力は変わりません。",
  "界面圆角": "角のスタイル",
  "立即切换全局直角或圆角风格，偏好会记录在本机，下次启动继续生效。":
    "直角と角丸を切り替えます。設定はローカルに保存されます。",
  "直角": "直角",
  "圆角": "角丸",
  "终端 Shell": "ターミナルシェル",
  "新终端默认使用；已有的终端不会被重启。":
    "新しいターミナルに使用されます。既存のターミナルは再起動されません。",
  "WSL（若已安装）": "WSL（インストール済みの場合）",
  "编辑器字号": "エディターの文字サイズ",
  "Monaco 编辑器字体大小（10–24 px）。": "Monaco エディターの文字サイズ（10–24 px）。",
  "对话字号": "チャットの文字サイズ",
  "消息正文与输入框字号（11–22 px）。": "メッセージと入力欄の文字サイズ（11–22 px）。",
  "模型路由": "モデルルーティング",
  "不同用途可以指向不同模型。路由只在已配置供应商的模型列表中推荐， 也允许手动填写自定义模型 ID。未授权的自动升级或收费更高的回退不会发生。":
    "用途ごとに異なるモデルを指定できます。設定済みプロバイダーのモデルのみを提案し、カスタムモデル ID も入力できます。無断のアップグレードや高額なフォールバックは行いません。",
  "默认聊天": "既定のチャット",
  "普通对话与快捷问题的默认路由": "チャットと簡単な質問の既定ルート",
  "未设置": "未設定",
  "Agent 规划": "エージェント計画",
  "用于任务规划（当前执行器使用 Agent 执行模型）":
    "タスク計画に使用（現在の実行器はエージェント実行モデルを使用）",
  "Agent 执行": "エージェント実行",
  "Agent Runtime 调用工具与执行任务使用的模型":
    "Agent Runtime がツール呼び出しとタスク実行に使用するモデル",
  "代码审查": "コードレビュー",
  "审查 Diff 与结果时使用的模型": "Diff と結果のレビューに使用するモデル",
  "视觉理解": "画像理解",
  "处理图片理解任务的默认模型": "画像理解タスクの既定モデル",
  "图像生成": "画像生成",
  "对话内生图与图像工作区的默认生成路由":
    "チャット内生成と画像ワークスペースの既定生成ルート",
  "图像编辑": "画像編集",
  "参考图编辑的默认路由；未设置时回退到图像生成路由":
    "参照画像編集の既定ルート。未設定時は画像生成ルートを使用",
  "打开供应商管理": "プロバイダー管理を開く",
  "Agent 与审批": "エージェントと承認",
  "审批策略": "承認ポリシー",
  "每次询问：写入与命令都暂停；自动审批：文件写入自动执行，命令仍需批准； 完全访问：写入与命令都自动执行。危险命令始终被权限策略硬拒绝。":
    "毎回確認：書き込みとコマンドを停止します。自動承認：書き込みは自動実行し、コマンドは承認が必要です。完全アクセス：書き込みとコマンドを自動実行します。危険なコマンドは常にポリシーで拒否されます。",
  "每次询问": "毎回確認",
  "自动审批": "自動承認",
  "完全访问已开启：Agent 的文件写入与终端命令都会自动执行。危险命令仍会被安全策略拒绝，请在受控项目中运行 Agent。":
    "完全アクセスが有効です。エージェントの書き込みとターミナルコマンドは自動実行されます。危険なコマンドは拒否されるため、管理されたプロジェクトで実行してください。",
  "无论开关状态，路径越界、UNC 网络路径、注册表、提权、格式化与下载执行等 高风险命令都在权限策略中被硬拒绝。":
    "設定にかかわらず、パス脱出、UNC パス、レジストリ、権限昇格、フォーマット、ダウンロード実行などの高リスクコマンドはポリシーで拒否されます。",
  "安全与数据": "セキュリティとデータ",
  "请求 User-Agent": "リクエスト User-Agent",
  "所有供应商请求使用的 UA；个别网关会拦截非浏览器 UA。留空则回退到内置标识。":
    "すべてのプロバイダーリクエストに使用する UA。一部のゲートウェイは非ブラウザ UA を拒否します。空欄の場合は組み込み値を使用します。",
  "API Key 存储": "API キーの保存",
  "API Key 写入 Windows 凭据管理器，不进入 SQLite、配置文件或普通日志。 普通日志也会过滤疑似令牌字符串。":
    "API キーは Windows 資格情報マネージャーに保存され、SQLite、設定ファイル、通常ログには書き込まれません。ログはトークンらしき文字列も伏せ字にします。",
  "项目边界": "プロジェクト境界",
  "Agent 只能通过相对路径访问当前授权的项目目录；路径穿越、符号链接逃逸、 .git 写入与项目外绝对路径写入会被阻止。命令中的项目外绝对路径会被拒绝。":
    "エージェントは相対パスでのみ許可されたプロジェクトにアクセスできます。トラバーサル、シンボリックリンク脱出、.git への書き込み、プロジェクト外への絶対パス書き込みはブロックされます。コマンド内のプロジェクト外絶対パスは拒否されます。",
  "进程与审批": "プロセスと承認",
  "Agent 命令带超时、取消与进程树终止；用户终端与 Agent 命令分开记录。 命令不通过字符串拼接注入；模型不能直接获得系统 Shell。":
    "エージェントコマンドにはタイムアウト、キャンセル、プロセスツリー終了があります。ユーザーのターミナルとエージェントコマンドは別々に記録されます。コマンドは文字列連結で注入されず、モデルはシステムシェルに直接アクセスできません。",
  "快捷键": "ショートカット",
  "新建 Agent 任务": "新しいエージェントタスク",
  "切换侧栏": "サイドバーを切り替え",
  "切换终端": "ターミナルを切り替え",
  "保存当前文件（编辑模式）": "現在のファイルを保存（編集モード）",
  "发送 / 换行": "送信 / 改行",
  "保存全部设置": "すべての設定を保存",
  "文件写入与终端命令都会暂停，等待你逐项批准。":
    "書き込みとターミナルコマンドは承認待ちで停止します。",
  "文件写入自动执行，终端命令仍需你批准。":
    "書き込みは自動実行され、ターミナルコマンドは承認が必要です。",
  "文件写入与终端命令自动执行；危险命令仍由安全策略强制拒绝。":
    "書き込みとターミナルコマンドは自動実行されます。危険なコマンドは引き続き拒否されます。",
  "思考强度：高（更多思考预算，适合复杂任务）":
    "思考の強さ：高（複雑なタスク向けの余裕ある予算）",
};

const ko: LocaleDictionary = {
  "刚刚": "방금",
  "{0} 分钟前": "{0}분 전",
  "{0} 小时前": "{0}시간 전",
  "{0} 天前": "{0}일 전",
  "已中断": "중단됨",
  "排队中": "대기 중",
  "已完成": "완료됨",
  "，其中 {0} 次失败": ", {0}회 실패",
  "，{0} 次取消": ", {0}회 취소",
  "，正在执行": ", 실행 중",
  "执行工具 {0} 次": "도구 실행 {0}회",
  "执行工具 {0} 次，其中 {1} 次失败": "도구 실행 {0}회, {1}회 실패",
  "执行工具 {0} 次，{1} 次取消": "도구 실행 {0}회, {1}회 취소",
  "执行工具 {0} 次，正在执行": "도구 실행 {0}회, 실행 중",
  "执行工具 {0} 次，其中 {1} 次失败，正在执行":
    "도구 실행 {0}회, {1}회 실패, 실행 중",
  "执行工具 {0} 次，{1} 次取消，正在执行":
    "도구 실행 {0}회, {1}회 취소, 실행 중",
  "从变更面板打开文件": "변경 패널에서 파일 열기",
  "CTRL+S 保存 · READ-ONLY 预览": "CTRL+S 저장 · READ-ONLY 미리보기",
  "没有打开的终端": "열린 터미널 없음",
  "新建 PowerShell": "새 PowerShell",
  "Agent 执行的命令会记录在任务时间线中":
    "에이전트 명령은 작업 타임라인에 기록됩니다",
  "图像工作区": "이미지 작업 공간",
  "GitHub 仓库": "GitHub 저장소",
  "主题：深色": "테마: 다크",
  "主题：浅色": "테마: 라이트",
  "主题：跟随系统": "테마: 시스템",
  "切换主题": "테마 전환",
  "当前工作区": "현재 작업 공간",
  "折叠会话": "세션 접기",
  "所有数据保存在本地": "모든 데이터는 로컬에 저장됩니다",
  "收起侧栏": "사이드바 접기",
  "打开终端": "터미널 열기",
  "收起右侧面板": "오른쪽 패널 접기",
  "普通对话：不会操作文件或执行命令":
    "대화: 파일 조작이나 명령 실행을 하지 않습니다",
  "Agent：在授权项目中执行编程任务":
    "에이전트: 권한이 부여된 프로젝트에서 코딩 작업 실행",
  "审批策略：完全访问（对后续 Agent 任务生效）":
    "승인 정책: 전체 액세스 (새 에이전트 작업에 적용)",
  "审批策略：每次询问（对后续 Agent 任务生效）":
    "승인 정책: 매번 확인 (새 에이전트 작업에 적용)",
  "审批策略：自动审批（对后续 Agent 任务生效）":
    "승인 정책: 자동 승인 (새 에이전트 작업에 적용)",
  "工具调用能力尚未经过真实测试":
    "도구 호출은 실제 요청으로 검증되지 않았습니다",
  "打开 Diff": "Diff 열기",
  "新建终端": "새 터미널",
  "选择终端类型": "터미널 유형 선택",
  "搜索输出": "출력 검색",
  "复制选中内容": "선택 항목 복사",
  "清屏": "지우기",
  "终止当前进程": "현재 프로세스 종료",
  "收起终端": "터미널 접기",
  "最小化": "최소화",
  "最大化": "최대화",
  "还原": "복원",
  "模型路由 · 审批策略 · 终端与界面":
    "모델 라우팅 · 승인 정책 · 터미널 및 인터페이스",
  "数据目录": "데이터 폴더",
  "保存设置": "설정 저장",
  "外观与编辑器": "모양 및 편집기",
  "深色与浅色使用同一套设计 Token；跟随系统会实时响应 Windows 主题。":
    "다크와 라이트는 같은 디자인 토큰을 사용하며, 시스템은 Windows 테마를 실시간으로 따릅니다.",
  "深色": "다크",
  "浅色": "라이트",
  "跟随系统": "시스템",
  "整套界面的背景、表面与强调色都会随配色变化；切换时颜色平滑过渡。":
    "배경, 표면, 강조 색상이 팔레트에 따라 바뀌며 전환은 부드럽게 애니메이션됩니다.",
  "玄武": "현무",
  "森林": "포레스트",
  "薄荷": "민트",
  "蜜蜂": "허니",
  "羊皮纸": "퍼치먼트",
  "界面语言": "인터페이스 언어",
  "切换界面文案；模型回复、代码与终端输出保持原样。":
    "인터페이스 문구만 전환합니다. 모델 응답, 코드, 터미널 출력은 그대로 유지됩니다.",
  "界面圆角": "모서리 스타일",
  "立即切换全局直角或圆角风格，偏好会记录在本机，下次启动继续生效。":
    "직각과 둥근 모서리를 전환합니다. 설정은 로컬에 저장됩니다.",
  "直角": "직각",
  "圆角": "둥근 모서리",
  "终端 Shell": "터미널 셸",
  "新终端默认使用；已有的终端不会被重启。":
    "새 터미널에 사용됩니다. 기존 터미널은 다시 시작되지 않습니다.",
  "WSL（若已安装）": "WSL (설치된 경우)",
  "编辑器字号": "편집기 글자 크기",
  "Monaco 编辑器字体大小（10–24 px）。": "Monaco 편집기 글자 크기 (10–24 px).",
  "对话字号": "대화 글자 크기",
  "消息正文与输入框字号（11–22 px）。": "메시지와 입력창 글자 크기 (11–22 px).",
  "模型路由": "모델 라우팅",
  "不同用途可以指向不同模型。路由只在已配置供应商的模型列表中推荐， 也允许手动填写自定义模型 ID。未授权的自动升级或收费更高的回退不会发生。":
    "용도별로 다른 모델을 지정할 수 있습니다. 설정된 공급자의 모델만 추천하며 사용자 지정 모델 ID도 입력할 수 있습니다. 무단 업그레이드나 더 비싼 폴백은 수행하지 않습니다.",
  "默认聊天": "기본 대화",
  "普通对话与快捷问题的默认路由": "대화와 빠른 질문의 기본 경로",
  "未设置": "설정 안 됨",
  "Agent 规划": "에이전트 계획",
  "用于任务规划（当前执行器使用 Agent 执行模型）":
    "작업 계획에 사용 (현재 실행기는 에이전트 실행 모델 사용)",
  "Agent 执行": "에이전트 실행",
  "Agent Runtime 调用工具与执行任务使用的模型":
    "Agent Runtime이 도구 호출과 작업 실행에 사용하는 모델",
  "代码审查": "코드 검토",
  "审查 Diff 与结果时使用的模型": "Diff와 결과 검토에 사용하는 모델",
  "视觉理解": "이미지 이해",
  "处理图片理解任务的默认模型": "이미지 이해 작업의 기본 모델",
  "图像生成": "이미지 생성",
  "对话内生图与图像工作区的默认生成路由":
    "대화 내 이미지 생성과 이미지 작업 공간의 기본 생성 경로",
  "图像编辑": "이미지 편집",
  "参考图编辑的默认路由；未设置时回退到图像生成路由":
    "참조 이미지 편집의 기본 경로. 설정되지 않으면 이미지 생성 경로 사용",
  "打开供应商管理": "공급자 관리 열기",
  "Agent 与审批": "에이전트 및 승인",
  "审批策略": "승인 정책",
  "每次询问：写入与命令都暂停；自动审批：文件写入自动执行，命令仍需批准； 完全访问：写入与命令都自动执行。危险命令始终被权限策略硬拒绝。":
    "매번 확인: 쓰기와 명령을 일시 중지합니다. 자동 승인: 쓰기는 자동 실행되고 명령은 승인이 필요합니다. 전체 액세스: 쓰기와 명령 모두 자동 실행됩니다. 위험한 명령은 항상 정책으로 거부됩니다.",
  "每次询问": "매번 확인",
  "自动审批": "자동 승인",
  "完全访问已开启：Agent 的文件写入与终端命令都会自动执行。危险命令仍会被安全策略拒绝，请在受控项目中运行 Agent。":
    "전체 액세스가 켜져 있습니다. 에이전트의 파일 쓰기와 터미널 명령이 자동 실행됩니다. 위험한 명령은 거부되므로 통제된 프로젝트에서 실행하세요.",
  "无论开关状态，路径越界、UNC 网络路径、注册表、提权、格式化与下载执行等 高风险命令都在权限策略中被硬拒绝。":
    "설정과 관계없이 경로 탈출, UNC 경로, 레지스트리, 권한 상승, 포맷, 다운로드 실행 등 고위험 명령은 정책으로 거부됩니다.",
  "安全与数据": "보안 및 데이터",
  "请求 User-Agent": "요청 User-Agent",
  "所有供应商请求使用的 UA；个别网关会拦截非浏览器 UA。留空则回退到内置标识。":
    "모든 공급자 요청에 사용하는 UA입니다. 일부 게이트웨이는 비브라우저 UA를 차단합니다. 비워 두면 내장 값을 사용합니다.",
  "API Key 存储": "API 키 저장",
  "API Key 写入 Windows 凭据管理器，不进入 SQLite、配置文件或普通日志。 普通日志也会过滤疑似令牌字符串。":
    "API 키는 Windows 자격 증명 관리자에 저장되며 SQLite, 구성 파일, 일반 로그에 기록되지 않습니다. 로그에서는 토큰처럼 보이는 문자열도 가립니다.",
  "项目边界": "프로젝트 경계",
  "Agent 只能通过相对路径访问当前授权的项目目录；路径穿越、符号链接逃逸、 .git 写入与项目外绝对路径写入会被阻止。命令中的项目外绝对路径会被拒绝。":
    "에이전트는 상대 경로로만 승인된 프로젝트에 접근할 수 있습니다. 경로 탈출, 심볼릭 링크 이탈, .git 쓰기, 프로젝트 외부 절대 경로 쓰기는 차단됩니다. 명령의 프로젝트 외부 절대 경로도 거부됩니다.",
  "进程与审批": "프로세스 및 승인",
  "Agent 命令带超时、取消与进程树终止；用户终端与 Agent 命令分开记录。 命令不通过字符串拼接注入；模型不能直接获得系统 Shell。":
    "에이전트 명령에는 시간 제한, 취소, 프로세스 트리 종료가 있습니다. 사용자 터미널과 에이전트 명령은 별도로 기록됩니다. 명령은 문자열 연결로 주입되지 않으며 모델은 시스템 셸에 직접 접근할 수 없습니다.",
  "快捷键": "단축키",
  "新建 Agent 任务": "새 에이전트 작업",
  "切换侧栏": "사이드바 전환",
  "切换终端": "터미널 전환",
  "保存当前文件（编辑模式）": "현재 파일 저장 (편집 모드)",
  "发送 / 换行": "전송 / 줄바꿈",
  "保存全部设置": "모든 설정 저장",
  "文件写入与终端命令都会暂停，等待你逐项批准。":
    "쓰기와 터미널 명령이 승인을 기다리며 일시 중지됩니다.",
  "文件写入自动执行，终端命令仍需你批准。":
    "쓰기는 자동 실행되고 터미널 명령은 승인이 필요합니다.",
  "文件写入与终端命令自动执行；危险命令仍由安全策略强制拒绝。":
    "쓰기와 터미널 명령이 자동 실행됩니다. 위험한 명령은 계속 거부됩니다.",
  "思考强度：高（更多思考预算，适合复杂任务）":
    "사고 강도: 높음 (복잡한 작업을 위한 더 많은 예산)",
};

export const extraDictionaries: Record<
  Exclude<Language, "zh-CN">,
  LocaleDictionary
> = {
  "en-US": en,
  "ja-JP": ja,
  "ko-KR": ko,
};
