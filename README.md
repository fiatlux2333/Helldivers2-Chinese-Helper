# HD2CN（Helldivers 2 中文助手）

✨ 面向 Windows 版《HELLDIVERS™ 2》的非官方中文输入辅助工具 ✨

在助手主面板或游戏右侧中文输入栏里用中文输入法写好句子，再把文字发送到游戏聊天框。

## 🔌 兼容性

| 项 | 要求 |
| --- | --- |
| 系统 | Windows 10 / 11 **x64** |
| 游戏显示 | **窗口化 / 无边框**（不支持独占全屏） |
| 运行时 | 本机需有 **WebView2**（Win10/11 通常自带） |
| 权限 | 助手与游戏尽量同一权限级别（都普通或都管理员） |
| 技术栈 | Vue 3 + TypeScript + pnpm · Rust · Tauri 2 |

## 📖 介绍

HD2CN 提供独立的中文 IME 输入、显式直发、中译英、快捷喊话，以及基于 Windows OCR 的游戏聊天截图翻译。

- **双输入模式** — 可在助手主界面输入，也可开启游戏右侧中文输入栏
- **独立编辑** — 用微软拼音等任意输入法完成组字后再发送到游戏
- **目标锁定** — 先捕获 HD2，逐批前再校验前台窗口身份
- **两种提交** — `Enter` 填字后直接发送，`Ctrl+Enter` 只填入供检查
- **连续多句** — 成功填入后保留目标会话，不必每句重新捕获
- **全局热键** — 可自定义唤回键；单击唤回，连按两次收起
- **本地 OCR** — 截图不上传，只把识别文本交给自定义 OpenAI 兼容接口翻译
- **双向提示词** — 英译中与中译英可分别自定义；中译英先理解整句，词库只约束明确命中的 HD2 专用术语
- **快捷喊话** — 最多保存 12 条常用消息，并可为每条消息录制全局热键

它不修改游戏、不注入进程、不读内存，也不分析游戏网络协议。

## 💿 安装

### 方式一：安装包（推荐自己用）

1. 打开本仓库的 [Releases](https://github.com/fiatlux2333/Helldivers2-Chinese-Helper/releases) 页面
2. 下载并双击 `Helldivers 2 中文助手_0.5.1_x64-setup.exe`
3. 按向导安装（默认当前用户，一般无需管理员）
4. 从开始菜单启动 **Helldivers 2 中文助手**
5. 安装目录中的 `必看说明.txt` 包含简明使用流程，首次使用建议先阅读

> 若 Windows 提示“未知发布者”，是因为安装包尚未代码签名，选择仍要运行即可。

### 方式二：开发模式（改代码 / 真机调试）

1. 准备工具链：Node 22+、pnpm 11、Rust（MSVC）、Visual Studio Build Tools（含 `link.exe`）、WebView2
2. 安装依赖并启动：

```powershell
# 若本机把 Rust/pnpm 装到自定义目录，请先把 cargo、node、pnpm 加入 PATH
# PowerShell 若拦截 pnpm.ps1，请改用 pnpm.cmd

pnpm install
pnpm tauri dev
```

也可双击项目根目录的 `dev-start.cmd`；它会调用同目录的 PowerShell 7 启动脚本。

> 第一次 `tauri dev` 会编译 Rust，可能较慢；编译成功后会弹出助手窗口，**不要关掉终端**。

### 方式三：自行打包 NSIS 安装程序

```powershell
pnpm tauri build
```

产物位置：

```text
src-tauri/target/release/bundle/nsis/*-setup.exe      # 安装程序
src-tauri/target/release/helldivers2-cn-helper.exe    # 绿色主程序
```

正式版本请优先从 GitHub Releases 下载，避免误用旧的本地构建产物。

## 🎮 怎么用

### 中文侧栏模式

```text
1. 在助手主界面开启「中文侧栏」，点击「捕获 HD2」并按提示切回游戏
2. 在游戏中按聊天键，右侧中文输入栏会出现并取得输入焦点
3. 选择「直发」时发送原中文；选择「中译英」时先调用翻译 API，只在翻译成功后发送英文译文
4. 使用中文输入法完成组字后按 `Enter` 发送
5. 发送或取消成功后右侧输入栏自动隐藏；下次按真实游戏聊天键时重新出现
6. 拖动输入栏左侧手柄可调整位置；位置会持久化，发送后的隐藏/恢复不会覆盖它
```

### 主界面模式

```text
1. 在助手主界面关闭「中文侧栏」
2. 游戏使用窗口化 / 无边框并先打开聊天框
3. 助手点击「捕获 HD2」，在倒计时内切回游戏
4. 回到助手输入中文，选择「中文直发」或「中译英」
5. 按 `Enter` 直接发送；按 `Ctrl+Enter` 仅填入并回游戏检查
6. 首次使用聊天翻译时校准区域；之后按截图翻译热键读取聊天
```

### 翻译与聊天校准

1. 在“设置”中填写 OpenAI 兼容 API 地址、API Key 和模型名；网络受限时可填写 HTTP/SOCKS5 代理
2. 按需编辑“游戏聊天英译中提示词”和“输入消息中译英提示词”；留空时使用内置 HD2 提示词
3. 内置中译英提示词会先理解整句语义、否定和动作关系；词库仅作参考，只有明确的武器、敌人、派系、战略配备、任务目标或游戏机制等专用术语才使用固定英文，普通表达不会机械套词
4. 捕获 HD2 后进入“聊天翻译”，点击“校准区域”
5. 助手最小化并截取游戏客户区；在预览图上拖选聊天消息区域并保存
6. 点击“读取并翻译”或使用截图翻译热键；相同的旧聊天行不会重复提交 API

游戏画面只在本机截图并交给 Windows OCR，发送到翻译 API 的是 OCR 识别出的新增聊天文本。

### 快捷喊话

在“设置”的“快捷喊话预设”中可保存最多 12 条常用消息，并为每条消息录制全局热键。触发后助手会重新验证已锁定的 HD2 窗口，再将预设消息发送到游戏。

### 记事本探针（推荐先练手）

1. 记事本另存为 `HELLDIVERS-input-test.txt`（标题需含 `HELLDIVERS`）
2. 光标放在编辑区
3. 助手捕获 → 4 秒内切到记事本 → 输入 → 填入
4. 记事本应出现完整文字，**没有自动回车**

### 热键

| 操作 | 默认行为 |
| --- | --- |
| 助手唤回热键 | `Ctrl+Shift+H`（可在界面自定义） |
| 截图翻译热键 | `Ctrl+Shift+T`（可在界面自定义） |
| 按 1 次 | 唤回助手前台并聚焦输入框 |
| 约 0.5 秒内连按 2 次 | 最小化助手，取消前台 |
| 持久化 | 保存在浏览器 `localStorage` 键 `hd2cn.restoreHotkey` |

两个热键不能相同；截图翻译热键只在目标会话有效且游戏位于前台时工作。

## 📝 功能特性

- ⌨️ **中文 IME 友好** — 组字 / 候选期间 Enter 交给输入法，结束后再提交
- 🎮 **可切换中文侧栏** — 开启时由游戏聊天键唤出可拖动且位置持久化的右侧输入栏，并可选择中文直发或中译英；关闭时恢复助手主界面输入
- 🎯 **前台身份复核** — 每批发送前重新校验目标；目标变了立刻停
- 🧩 **HD2 窗口回退** — `stingray_window` 上标准 HWND→PID 可能被挡时，用唯一 `helldivers2.exe` 合法回退
- 🛡️ **UIPI 门禁** — 助手完整性级别必须 ≥ 目标，否则拒绝填入
- ✂️ **文本清理与注入** — 剔除控制字符、CR/LF/NUL 等；单条上限 100 字；默认使用 GBK Alt 数字码，保留 Unicode `SendInput` 作为排障选项
- 🔁 **连续会话** — 填入成功保留目标，支持连续多句
- 🌐 **中译英** — API 成功并通过最终长度校验后才进入注入链路
- 🔎 **聊天翻译** — 内置 Windows 离线中英混合 OCR，按行合并识别结果，只把新出现的消息正文送往现有 OpenAI 兼容翻译接口
- 📚 **HD2 双向提示词** — 英译中保留玩家黑话映射；中译英只固定映射明确命中的专用术语，普通表达按整句上下文翻译
- 🔌 **显式网络代理** — API 默认直连；需要代理时在设置中填写 HTTP/SOCKS5 地址
- ⬆️ **检查更新** — 启动时后台检查一次，也可在设置中手动检查；只提示并打开 GitHub 下载页，不自动更新
- 📣 **快捷喊话** — 最多 12 条预设消息、独立全局热键和可调聚焦等待时间
- 🔽 **不抢焦点** — 主界面发送后保持最小化；右侧输入栏发送或取消成功后自动隐藏
- 🎹 **自定义全局热键** — 录制新组合；被占用可回退默认
- 📦 **NSIS 安装包** — 约 3 MB 的 `*-setup.exe`（WebView2 使用系统组件）

## ⚠️ 注意事项

1. **直发会注入最终 Enter**
   普通 `Enter` 和主按钮会自动发送；需要先检查时使用 `Ctrl+Enter` 或“仅填入”。文字或目标验证失败时不会补发 Enter。

2. **不要用独占全屏**  
   首批只保证窗口化 / 无边框。全屏焦点行为不在支持范围。

3. **权限一致**  
   游戏管理员 + 助手普通用户 → 会触发 UIPI 拒绝。请两边权限对齐。

4. **误投风险仍存在**  
   `SendInput` 不能把输入原子绑定到某个 HWND。设计是「目标一变就中止」，而不是盲发。中途切窗可能留下半截字，**不要直接重试整段**。

5. **反作弊不是零风险**  
   使用的是 Windows 用户态输入 API，**不代表**反作弊零风险。请自行遵守游戏与平台条款；有疑虑请立刻停用。

6. **非官方**  
   与 Arrowhead、Sony、PlayStation 等**无任何隶属或授权关系**。

7. **隐私**  
   不要输入密码、令牌、真实姓名等敏感内容做测试。

8. **磁盘占用**  
   开发目录若到数 GB，主要是 `src-tauri/target` 编译缓存，不是安装包本身。可清理：

```powershell
Remove-Item -Recurse -Force ".\src-tauri\target"
```

9. **Windows OCR 语言**
   中英双引擎需要系统同时安装简体中文和英语 OCR 组件。只有中文组件时会自动进入“中英混合兼容模式”，仍可读取中文及常见拉丁字符，但英文准确率可能较低。

10. **更新由用户手动安装**
    助手只检查 GitHub 最新正式版并提示下载，不会自动下载、自动安装或替换当前程序。

## 🛠️ 技术实现（摘要）

```text
Vue 输入面板
  │  草稿 / 会话 ID / 显式操作
  ▼
Tauri IPC 编排层
  │  命令校验 · 单一会话 · 稳定错误码
  ├───────────────┐
  ▼               ▼
纯 Rust 核心       Windows 适配层
文本 / 配置 / 状态机   窗口诊断 / UIPI / 键盘事件注入
```

- 默认中文填入路径将字符编码为 GBK Alt 数字码，并通过 `keybd_event` 发送物理小键盘扫描码；`KEYEVENTF_UNICODE` `SendInput` 仅保留为排障选项（见 `src-tauri/src/platform/windows/injector.rs`）
- 每批前 `validate_foreground`；失败标记 `partial_prefix_possible`
- 窗口匹配默认关键词 `HELLDIVERS`（`src-tauri/src/core/config.rs`）
- IME 闩锁：`src/composables/useCompositionLatch.ts`
- 热键：`@tauri-apps/plugin-global-shortcut` + `src/composables/useRestoreHotkey.ts`

更细的模块与状态机见 [`docs/architecture.md`](docs/architecture.md)。  
真机验收矩阵见 [`docs/windows-probe.md`](docs/windows-probe.md) 与 [`docs/manual-rounds-checklist.md`](docs/manual-rounds-checklist.md)。

## 🧪 开发与检查命令

```powershell
# PowerShell 7 开发启动（同时写入 UTF-8 调试日志）
.\dev-start.ps1

# 前端
pnpm install
pnpm run typecheck
pnpm run test:run
pnpm run build

# Rust 核心
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --lib -- -D warnings

# 桌面
pnpm tauri dev
pnpm tauri build
```

### 当前状态（本机实测）

| 项目 | 状态 |
| --- | --- |
| Vue 面板 / 类型检查 / 前端测试 | ✅ 通过 |
| Rust 核心测试 | ✅ 通过 |
| 记事本探针 | ✅ 通过 |
| HD2 窗口化填字 | ✅ 通过（含进程名回退） |
| 连续多句 / 不抢焦点 / 自定义热键 | ✅ 通过 |
| NSIS 安装包 | ✅ 0.5.1 可生成（约 3 MB） |
| 正式商店发布 / 自动更新 / 代码签名 | ❌ 不在当前范围 |

## 📄 许可证

本项目以 [MIT License](LICENSE) 发布。第三方依赖与声明见 [NOTICE.md](NOTICE.md)。

## ⚠ 非官方与商标

“HELLDIVERS”“HELLDIVERS 2” 及相关名称、标志属于其各自权利人。本项目仅为兼容说明作必要文字引用，**不表示得到官方认可**。

## 📮 反馈与建议

遇到捕获失败、误投、输入法异常或安装包问题，请带上：

- Windows 版本与权限（管理员 / 普通）
- 游戏显示模式（窗口化 / 无边框）
- 助手提示文案或错误码（**不要贴聊天正文**）

欢迎在本仓库提 Issue 或 PR。
