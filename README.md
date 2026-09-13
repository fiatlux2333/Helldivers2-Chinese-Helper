# HD2CN（Helldivers 2 中文助手）

Windows 版《HELLDIVERS™ 2》的非官方中文输入辅助工具：在助手主面板或游戏右侧中文输入栏用中文输入法写好句子，再发送到游戏聊天框。

## 兼容性

| 项 | 要求 |
| --- | --- |
| 系统 | Windows 10 / 11 x64 |
| 游戏显示 | 窗口化 / 无边框（不支持独占全屏） |
| 运行时 | WebView2（Win10/11 通常自带） |
| 权限 | 助手与游戏必须同一权限级别，优先都普通运行 |

## 功能

- 中文侧栏：由游戏聊天键唤出可拖动的右侧输入栏，位置持久化，支持中文直发或中译英；唤出后立刻打字不丢字，发送可随时取消（草稿保留）
- 聊天键可选仅响应小键盘回车，与主键盘回车区分（需同步修改 HD2 内聊天键绑定，详见必看说明）
- 主界面输入：捕获 HD2 后在助手内输入，`Enter` 自动打开游戏聊天框发送，`Ctrl+Enter` 仅填入供检查
- 中译英 / 英译中：调用 OpenAI 兼容接口，按消息匹配 HD2 官方词库术语，翻译成功并通过长度校验后才注入；相同内容本地缓存，重复发送秒发
- 聊天翻译：本地 Windows OCR 读取游戏聊天，按行拆分增量消息，可在翻译页或游戏内透明 HUD 显示
- 快捷喊话：最多 12 条预设消息，每条可录制全局热键
- 战备：预设库或手动配置方向序列，WASD / 方向键可选，可绑定全局热键，支持统一延迟设置
- 输入法保护：游戏前台自动保持 CapsLock 游戏态，侧栏/助手输入时临时恢复，降低 HD2 中文 IME 卡键风险
- 连续会话：发送成功后保留目标，不必每句重新捕获
- 热键：助手唤回、截图翻译、喊话、战备均可自定义
- 更新检查：只提示 GitHub 最新版，不自动下载或安装

安全边界：不修改游戏文件、不注入进程、不读内存、不分析网络协议；文字注入使用 Windows 用户态 `SendInput`。

## 安装

### 安装包（推荐）

1. 从 [Releases](https://github.com/fiatlux2333/Helldivers2-Chinese-Helper/releases) 下载 `Helldivers 2 中文助手_*_x64-setup.exe`
2. 按向导安装（默认当前用户，一般无需管理员），从开始菜单启动
3. 安装目录中的 `必看说明.txt` 包含简明使用流程，首次使用建议先读

> Windows 提示"未知发布者"是因为安装包尚未代码签名，选择仍要运行即可。

### 开发模式

准备 Node 22+、pnpm 11、Rust（MSVC）、Visual Studio Build Tools（含 `link.exe`）、WebView2：

```powershell
pnpm install
pnpm tauri dev
```

也可双击根目录的 `dev-start.cmd`。首次编译较慢；编译成功后弹出助手窗口，不要关掉终端。

### 自行打包

```powershell
pnpm tauri build
```

产物在 `src-tauri/target/release/bundle/nsis/*-setup.exe`（安装程序）和 `src-tauri/target/release/helldivers2-cn-helper.exe`（绿色主程序）。正式版本请优先从 GitHub Releases 下载。

## 使用

### 中文侧栏模式

1. 主界面开启「中文侧栏」，点击「捕获 HD2」并按提示切回游戏
2. 游戏内按聊天键，右侧输入栏出现并取得输入焦点
3. 用中文输入法组字后按 `Enter` 发送；「中译英」先调用翻译 API，成功后才发送英文译文
4. 发送或取消成功后输入栏自动隐藏；拖动左侧手柄调整位置，位置会持久化

### 主界面模式

1. 关闭「中文侧栏」，捕获 HD2 后回到助手输入
2. `Enter` 自动打开游戏聊天框并直接发送；`Ctrl+Enter` 仅填入并回游戏检查
3. 首次聊天翻译需校准区域，之后按截图翻译热键读取

### 翻译设置

- 在设置中填写 OpenAI 兼容 API 地址、API Key 和模型名；网络受限可填 HTTP/SOCKS5 代理
- 英译中 / 中译英提示词可自定义，留空使用内置提示词；词库只约束明确命中的 HD2 专用术语
- 聊天翻译按行拆分增量消息，相同旧聊天行不重复提交 API
- 游戏画面只在本机截图并交给 Windows OCR，发送到翻译 API 的是识别出的新增聊天文本

### 记事本探针（建议先练手）

1. 记事本另存为 `HELLDIVERS-input-test.txt`（标题需含 `HELLDIVERS`）
2. 光标放在编辑区，助手捕获 → 4 秒内切到记事本 → 输入 → 填入
3. 记事本应出现完整文字且没有自动回车

### 热键

| 操作 | 默认 |
| --- | --- |
| 助手唤回 | `Ctrl+Shift+H`（可自定义） |
| 截图翻译 | `Ctrl+Shift+T`（可自定义） |
| 单击唤回键 | 唤回助手前台并聚焦输入框 |
| 约 0.5 秒内连按两次 | 最小化助手 |

两个热键不能相同；截图翻译热键只在目标会话有效且游戏位于前台时工作。

## 注意事项

1. **直发会注入最终 Enter**：需要先检查时用 `Ctrl+Enter` 或「仅填入」。文字或目标验证失败时不会补发 Enter。
2. **不要用独占全屏**：只保证窗口化 / 无边框。
3. **权限一致**：Windows 会阻止低权限助手控制高权限游戏。优先游戏和助手都普通运行；若游戏必须管理员运行，助手也必须管理员运行。
4. **输入法保护不是根治**：CapsLock 策略只降低卡键风险，HD2 自身仍可能错误处理中文 IME 指令；异常时先切出游戏再切回。
5. **误投风险**：`SendInput` 不能把输入绑定到某个窗口，设计是「目标一变就中止」。中途切窗可能留下半截字，不要直接重试整段。
6. **反作弊**：使用的是 Windows 用户态输入 API，不代表零风险，请自行遵守游戏与平台条款，有疑虑立即停用。
7. **非官方**：与 Arrowhead、Sony、PlayStation 无任何隶属或授权关系。
8. **隐私**：不要输入密码、令牌、真实姓名等敏感内容。
9. **OCR 语言**：中英双引擎需系统同时安装简体中文和英语 OCR 组件；缺英语组件时自动进入兼容模式，英文准确率可能较低。
10. **更新手动安装**：助手只检查并提示 GitHub 最新版，不自动更新。

## 技术实现

```text
Vue 输入面板（草稿 / 会话 ID / 显式操作）
  ↓
Tauri IPC 编排层（命令校验 · 单一会话 · 稳定错误码）
  ├──────────────┐
  ↓              ↓
纯 Rust 核心      Windows 适配层
文本 / 配置 / 状态机  窗口诊断 / UIPI / 键盘事件注入
```

- 中文填入默认逐字符 `KEYEVENTF_UNICODE` `SendInput`（`src-tauri/src/platform/windows/injector.rs`）
- CapsLock 游戏态保护与前后台切换（`src-tauri/src/platform/windows/game_monitor.rs`）
- 每批发送前 `validate_foreground`，失败标记 `partial_prefix_possible`
- 窗口匹配默认关键词 `HELLDIVERS`（`src-tauri/src/core/config.rs`）
- IME 闩锁：`src/composables/useCompositionLatch.ts`；热键：`src/composables/useRestoreHotkey.ts`

模块与状态机详见 [`docs/architecture.md`](docs/architecture.md)，真机验收见 [`docs/windows-probe.md`](docs/windows-probe.md) 与 [`docs/manual-rounds-checklist.md`](docs/manual-rounds-checklist.md)。

## 开发与检查

```powershell
.\dev-start.ps1                  # PowerShell 7 开发启动（UTF-8 调试日志）

pnpm run typecheck               # 前端类型检查
pnpm run test:run                # 前端测试
pnpm run build                   # 前端生产构建

cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --lib -- -D warnings
```

开发目录到数 GB 是 `src-tauri/target` 编译缓存，可 `Remove-Item -Recurse -Force .\src-tauri\target` 清理。

## 许可证

以 [MIT License](LICENSE) 发布，第三方依赖见 [NOTICE.md](NOTICE.md)。"HELLDIVERS""HELLDIVERS 2" 及相关名称、标志属于其各自权利人，本项目仅为兼容说明作必要引用，不代表得到官方认可。

## 反馈

提 Issue 时请附：Windows 版本与权限（管理员/普通）、游戏显示模式、助手提示文案或错误码（不要贴聊天正文）。
