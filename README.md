# HD2CN（Helldivers 2 中文输入助手）

✨ 面向 Windows 版《HELLDIVERS™ 2》的非官方中文输入辅助工具 ✨

在独立面板里用中文输入法写好句子，再把文字填进游戏聊天框。

## 🔌 兼容性

| 项 | 要求 |
| --- | --- |
| 系统 | Windows 10 / 11 **x64** |
| 游戏显示 | **窗口化 / 无边框**（不支持独占全屏） |
| 运行时 | 本机需有 **WebView2**（Win10/11 通常自带） |
| 权限 | 助手与游戏尽量同一权限级别（都普通或都管理员） |
| 技术栈 | Vue 3 + TypeScript + pnpm · Rust · Tauri 2 |

## 📖 介绍

HD2CN 只做一件事：让你在独立、支持 IME 的面板里编辑中文，再把已确认的文字安全地填入已捕获的游戏（或探针）窗口。

- **独立编辑** — 用微软拼音等任意输入法写完再填入
- **目标锁定** — 先捕获 HD2，逐批前再校验前台窗口身份
- **只填字** — 绝不自动 Enter / Esc，发送永远由你在游戏里完成
- **连续多句** — 成功填入后保留目标会话，不必每句重新捕获
- **全局热键** — 可自定义唤回键；单击唤回，连按两次收起

它不是翻译器、外挂框架，也不修改游戏、不注入进程、不读内存。

## 💿 安装

### 方式一：安装包（推荐自己用）

1. 打开本仓库的 `release` 目录
2. 双击 `HD2CN-0.1.0-x64-setup.exe`（约 1.7 MB）
3. 按向导安装（默认当前用户，一般无需管理员）
4. 从开始菜单启动 **Helldivers 2 中文助手**

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

也可双击项目根目录的 `dev-start.cmd`（若已按本机路径写好环境变量）。

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

本仓库若已构建过，也可直接使用 `release/` 目录下的安装包副本。

## 🎮 怎么用

### 日常流程

```text
1. 游戏用窗口化 / 无边框，先打开聊天框
2. 助手点「捕获 HD2」，在倒计时内切回游戏
3. 在助手输入中文 →「填入游戏」
4. 在游戏里自己按 Enter 发送
5. 热键唤回助手 → 下一句（通常无需重新捕获）
6. 连按两次热键 → 助手最小化，交还游戏
```

### 记事本探针（推荐先练手）

1. 记事本另存为 `HELLDIVERS-input-test.txt`（标题需含 `HELLDIVERS`）
2. 光标放在编辑区
3. 助手捕获 → 4 秒内切到记事本 → 输入 → 填入
4. 记事本应出现完整文字，**没有自动回车**

### 热键

| 操作 | 默认行为 |
| --- | --- |
| 默认热键 | `Ctrl+Shift+H`（可在界面自定义） |
| 按 1 次 | 唤回助手前台并聚焦输入框 |
| 约 0.5 秒内连按 2 次 | 最小化助手，取消前台 |
| 持久化 | 保存在浏览器 `localStorage` 键 `hd2cn.restoreHotkey` |

热键**只负责窗口前台**，不会自动捕获、不会填字、不会发 Enter。

## 📝 功能特性

- ⌨️ **中文 IME 友好** — 组字 / 候选期间 Enter 交给输入法，结束后再提交
- 🎯 **前台身份复核** — 每批发送前重新校验目标；目标变了立刻停
- 🧩 **HD2 窗口回退** — `stingray_window` 上标准 HWND→PID 可能被挡时，用唯一 `helldivers2.exe` 合法回退
- 🛡️ **UIPI 门禁** — 助手完整性级别必须 ≥ 目标，否则拒绝填入
- ✂️ **文本清理** — 剔除控制字符、CR/LF/NUL 等；单条上限 100 字，分批 `KEYEVENTF_UNICODE`
- 🔁 **连续会话** — 填入成功保留目标，支持连续多句
- 🔽 **不抢焦点** — 填入成功后助手保持最小化，方便你在游戏里按 Enter
- 🎹 **自定义全局热键** — 录制新组合；被占用可回退默认
- 📦 **NSIS 安装包** — 约 1.7 MB 的 `*-setup.exe`（WebView2 使用系统组件）

## ⚠️ 注意事项

1. **绝不自动发送**  
   工具只填字。最终 Enter 必须由你在游戏中手动按下。

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
文本 / 配置 / 状态机   窗口诊断 / UIPI / SendInput
```

- 填入路径仅 `KEYEVENTF_UNICODE` 按下 + 抬起（见 `src-tauri/src/platform/windows/injector.rs`）
- 每批前 `validate_foreground`；失败标记 `partial_prefix_possible`
- 窗口匹配默认关键词 `HELLDIVERS`（`src-tauri/src/core/config.rs`）
- IME 闩锁：`src/composables/useCompositionLatch.ts`
- 热键：`@tauri-apps/plugin-global-shortcut` + `src/composables/useRestoreHotkey.ts`

更细的模块与状态机见 [`docs/architecture.md`](docs/architecture.md)。  
真机验收矩阵见 [`docs/windows-probe.md`](docs/windows-probe.md) 与 [`docs/manual-rounds-checklist.md`](docs/manual-rounds-checklist.md)。

## 🧪 开发与检查命令

```bash
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
| NSIS 安装包 | ✅ 可生成（约 1.7 MB） |
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
