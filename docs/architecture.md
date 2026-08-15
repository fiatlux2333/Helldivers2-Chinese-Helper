# 🛠 架构与安全设计

> 记录 HD2CN 当前架构与**不可突破**的安全边界。前端、纯 Rust 核心、Tauri IPC、Windows 适配层已按本文实现；记事本与 HD2（窗口化）已在本机通过基础链路，仍不声称反作弊零风险或全版本兼容。

目标平台：Windows 10/11 **x64** · Vue 3 + TypeScript · Tauri 2 · Rust。

## 🎯 设计目标与非目标

### 目标

- 在独立、支持中文 IME 的输入框或外置游戏侧栏中完成文字编辑
- 仅向**用户发起且身份复核通过**的前台窗口填入受保护文字；中文默认走 Unicode SendInput
- 文本处理与会话状态机做成平台无关、可在 Linux 测试的纯 Rust 核心
- 目标变化、权限不匹配、部分注入或状态不确定时**安全停止**
- 普通 Enter 可在文字完整注入且目标再次验证后发送最终 Enter；`Ctrl+Enter` 保留只填入
- 使用本地 Windows 中英双 OCR 读取用户校准的聊天区域，只把相对上一帧新增的聊天行交给显式配置的 OpenAI 兼容 API 翻译

### 非目标

- DLL 注入、进程内插件、内存读写
- 反作弊绕过、隐藏、伪装或规避检测
- 自动聊天、连发、无人值守输入
- 修改游戏文件、网络协议分析、进程内游戏覆盖层
- 独占全屏支持
- 向任意前台窗口提供无条件文本注入接口

## 📦 模块边界

```text
Vue 输入面板
  │  仅传递用户草稿、会话 ID 和显式操作
  ▼
Tauri IPC / 应用编排层
  │  校验命令、序列化单一会话、映射稳定错误码
  ├───────────────┐
  ▼               ▼
纯 Rust 核心       Windows 平台适配层
文本/配置/状态机   窗口快照/按键采样/权限诊断/键盘事件注入
  │               │
  └────结构化数据──┘
```

### Vue 输入面板

**职责**

- 单行中文 IME 编辑、字符计数、状态提示
- 处理 `compositionstart/update/end`，避免候选 Enter 被当成提交
- 将 DOM 草稿与 session ID 交给后端
- 区分直发和只填入；失败保留草稿且不自动重试或补键
- 管理中译英、聊天截图翻译、区域校准和 API 设置
- 全局热键唤回 / 连按收起（不填字）
- 管理独立中文侧栏窗口、译文 HUD 和手动配置的一键战备

**禁止**

- 直接调用 Win32
- 自行决定目标窗口
- 在前端拼装或循环发送输入事件
- 把隐藏窗口或卸载前端等同于“已安全发送”

### Tauri IPC 与应用编排层

- 最小、强类型命令
- 每次请求携带 generation / session ID
- 同一时间只有一个输入事务
- Windows 错误 → 稳定错误码 + 可读消息
- 拒绝当前状态不允许的调用和迟到结果

命令覆盖诊断、快照、受保护输入、限定的 OpenAI 兼容请求、GitHub 最新正式版查询和已锁定游戏客户区截图；不开放 shell、任意文件或任意窗口输入。

### 纯 Rust 核心（`src-tauri/src/core`）

不依赖 Tauri / Win32 / GTK / WebKit：

- 删除 NUL、CR/LF、C0/C1 及 Unicode 行/段分隔符
- 按 Unicode 标量检查上限与分批，再 `encode_utf16()`（不拆非 BMP 代理对）
- 配置：批大小、批延迟、字符上限、标题关键词
- 会话状态机、generation、结构化错误
- **只决策**（继续 / 等待 / 中止），不执行平台副作用

因此 Linux 可跑有意义的核心测试，无需 GTK/WebKit。

### Windows 平台适配层

仅 `cfg(target_os = "windows")`：

- `GetForegroundWindow`、`GetWindowTextW`、`GetWindowThreadProcessId`
- 进程创建时间、可见性、最小化、DWM cloaked
- 完整性级别只读诊断（UIPI 门禁）
- 默认中文路径：`SendInput` + `KEYEVENTF_UNICODE` 的 UTF-16 按下/抬起
- 旧 GBK Alt 数字码路径保留为内部兼容代码，但普通配置会迁移到 Unicode
- 文字全部成功后再次验证目标，再按显式 `submit` 参数注入扫描码 Enter
- 战备路径只发送用户手动配置的菜单键和方向键序列，不做 OCR、内存读取或自动识别
- GDI 客户区截图、Windows 离线中英双 OCR、按纵向位置合并和已安装 OCR 语言枚举

**HD2 身份回退（受限）**：部分 `stingray_window` 上标准 HWND→PID 会被挡（PID=0）。允许在**类名/标题像 HD2**且本机**唯一** `helldivers2.exe` 时，用进程名绑定 PID，仍走 `OpenProcess` / 创建时间 / 完整性校验。  
**禁止**：仅凭标题盲发；不在发行构建提供绕过目标验证的后门。

### 配置与日志

- 不记录聊天正文
- 默认可记时间、状态、错误码、批次数等元数据
- 不记录不必要的窗口标题全文
- 配置解析失败 → 安全默认值，不放宽目标验证
- 热键偏好可存前端 `localStorage`（`hd2cn.restoreHotkey`）
- 中文侧栏位置可存前端 `localStorage`（`hd2cn.overlay.position.v1`）；仅用户拖动事件写入，最小化和程序定位不会覆盖
- 独立中文侧栏使用固定 `chat-overlay` Tauri 窗口，主助手窗口不再通过改尺寸/装饰模拟侧栏
- 战备设置、官方词库提示词、OCR 区域和 HUD 位置存于翻译设置 JSON；旧配置缺失字段时走安全默认值
- 更新检查只读取固定仓库的 GitHub `releases/latest`，使用语义版本比较；下载按钮只允许打开该仓库的 Releases URL，不下载或安装文件
- NSIS bundle 包含 `必看说明.txt`，作为安装目录中的离线使用说明
- 翻译设置存于当前用户应用配置目录；API Key 按用户选择明文保存但不回传前端或写日志

## 🔄 输入会话状态机

```text
Idle
  └─ 用户显式捕获且检测到合格目标 ─► Editing
       ├─ 取消 ─► Idle
       └─ 用户请求填字 ─► RestoringTarget / Injecting
            ├─ 全部批次成功 ─► Editing（保留目标，便于下一句）
            └─ 任一不确定/失败 ─► Failed
Failed
  ├─ 保留草稿 ─► Editing
  └─ 用户取消 ─► Idle
```

**关键约束**

- IME 候选期间 Enter 不提交；长按 Enter 不重复提交
- `Ctrl+Enter` 只填入；Alt/Shift/Win + Enter 不提交
- 每次会话 generation 递增，旧异步结果不得污染新会话
- 全局事务门：不允许并发第二次填入
- `SendInput` 返回异常时标记输入状态不确定，阻止后续注入
- `Failed` **不**自动重试；文字失败不补 Enter，最终 Enter 部分失败则锁定后续注入

## 🚪 目标窗口身份与安全门

一次快照至少包含：

```text
HWND + PID + GUI thread ID + process creation time
+ title match + visible + minimized + cloaked
```

允许填字前须同时满足：

1. 会话由用户显式发起  
2. 标题匹配配置关键词（默认 `HELLDIVERS`）  
3. HWND / PID / 线程 / 创建时间一致（或合法进程名回退后仍可校验）  
4. 目标为前台、可见、未最小化、未 cloaked  
5. `current_integrity >= target_integrity`  
6. 文本清理后非空且未超限  
7. 处于唯一合法的注入入口  

每个 UTF-16 批次前**重复**前台与身份检查；失败立即停止。

## 🛡 为什么是这些限制

| 限制 | 原因 |
| --- | --- |
| 不进进程、不读内存 | 超出“填字”必要范围，风险远大于收益 |
| 不用低层键盘 Hook | 权限、隐私、稳定性与误捕获风险高 |
| Enter 只在文字完整后发送 | 避免文字部分失败后仍提交残缺消息 |
| 不支持独占全屏 | 焦点时序极不稳定，易黑屏/误投 |

## ⚠ 已知残余风险

- `SendInput` 校验后到事件到达前的焦点竞态  
- 游戏 / 反作弊拒绝、截断或重排合成输入  
- 部分批次只发了一半 → 游戏里留半截字  
- UIPI / 管理员差异  
- 高负载、远程桌面、覆盖层  
- 游戏更新改标题或聊天行为  

失败结果须含：批次数、失败位置、`partial_prefix_possible`。  
**不能**声称已回滚：`SendInput` 没有事务回滚。

## 🧪 测试边界

| 环境 | 能证明什么 |
| --- | --- |
| Linux / 沙箱 | 前端 typecheck/test/build；纯 Rust 核心 |
| Windows CI | 可编译、可单测；**不能**替代真机 |
| 真机记事本 + HD2 窗口化 | 焦点、IME、键盘注入、权限、热键和侧栏位置等（本机已基础通过） |

人工矩阵：[`windows-probe.md`](windows-probe.md) · [`manual-rounds-checklist.md`](manual-rounds-checklist.md)

## 🗺 里程碑（摘要）

| 阶段 | 内容 |
| --- | --- |
| Phase 0–2 | 探针、诊断、受保护填字、热键与安装包 — **本机已可用** |
| Phase 3 | 托盘、单实例、可审计热键增强等（需单独评审） |
| Phase 4 | 签名安装包、NOTICE 自动化、多版本回归 |

**仍不进入范围**：无人值守自动聊天、进程内游戏覆盖层、独占全屏、进程注入、内存读取和反作弊规避。
