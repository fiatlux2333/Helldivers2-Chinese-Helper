# 🧾 代码符合性审计

对照 [`windows-probe.md`](windows-probe.md)（不可违反规则、测试分层、矩阵与发布门槛），逐条核对**当前源码**是否落实。

| 项 | 内容 |
| --- | --- |
| 审计范围 | 源码静态审计 + 本机自动化测试 |
| 审计起点 | 2026-07-19；其后本机已完成 HD2 填字 / 热键 / NSIS 打包，运行时判定由 🟡 可升为 ✅（以真机记录为准） |
| 匹配关键词 | `HELLDIVERS`（`core/config.rs`） |

## 🗂 判定图例

- ✅ **已证明**：源码正确，且有自动化测试或可静态确证
- 🟡 **代码就绪，待/可真机确认**：运行时行为需 Windows + HD2（或记事本）确认
- ⚠️ **流程项**：测试者纪律或发布流程，无对应产品代码

---

## 🚫 不可违反的规则

| 规则 | 代码依据 | 判定 |
| --- | --- | --- |
| 不测独占全屏 | 文档声明 + 目标须可见/未 cloaked；无全屏专用路径 | ⚠️/🟡 |
| 每次捕获/填字由人显式触发，无无人值守循环 | `App.vue` 均为按钮触发；无定时自动提交 | ✅ |
| 先捕获目标、恢复并复核同一目标，绝不盲发 | `inject_probe_text`：`restore_foreground`→1.5s `validate_foreground` 循环→每批再 `validate_foreground` | ✅(逻辑)/🟡(运行时) |
| 只填字，绝不自动按最终 Enter | `injector.rs` 仅发 `KEYEVENTF_UNICODE` 按下+抬起；全仓无 `VK_RETURN` 注入 | ✅ |
| 失败不自动重试、不补发 Enter/Esc | 失败分支直接 `fail_injection` 返回错误并**保留完整草稿**；无重试循环（`session.rs` 测试 `failure_preserves_complete_draft_without_retry_transition`） | ✅ |
| 焦点/身份/权限/状态不确定即中止 | `input_state_uncertain` 门、`SubmitKeyStillDown`、`TargetChanged`、`IntegrityIncompatible` 全部走中止 | ✅(逻辑)/🟡 |
| 不绕过反作弊/UIPI/权限 | 仅用用户态 `SendInput` + 权限诊断；不提权、不 Hook、不改内存 | ✅/⚠️ |
| 首次用私人环境、非敏感短文本 | — | ⚠️ |

---

## 📚 测试分层

### A. 纯逻辑与诊断接收器（本机可验证）

| A 层检查 | 代码/测试 | 判定 |
| --- | --- | --- |
| 文本清理与字符上限 | `core/text.rs` `clean_text`/`preview_text`；测试 `removes_line_breaks_nul_and_control_characters`、`validates_cleaned_length` | ✅ |
| BMP 中文与非 BMP 代理对不拆开 | 按 `char`（Unicode 标量）切批；测试 `chunks_by_scalar_without_splitting_surrogate_pairs`、`counts_unicode_scalars_not_bytes_or_utf16_units` | ✅ |
| SendInput 事件数与按下/释放配对 | `unicode_inputs`：每个 u16 生成 down+up 两个事件；`inserted != inputs.len()` 即失败，`inserted % 2 != 0` 置 `key_state_uncertain` | ✅(结构)/🟡(实际计数) |
| 中途切焦点立即停止 | `injector.rs` 每批前 `validate_foreground`，变化即 `TargetChanged` 并停 | ✅(逻辑)/🟡 |
| 部分失败结果标记 | `InjectionReport.partial_prefix_possible` / `failed_batch_index` | ✅ |
| 从不产生最终 Enter/Esc | 见「不可违反的规则」 | ✅ |

### B. HD2 只读诊断

- 代码：`target.rs` 的 `foreground_diagnostic` / `identity_for`（标题、HWND、PID、GUI 线程、进程创建时间、可见 / 最小化 / cloaked）
- 另有 `stingray_window` 场景下**唯一** `helldivers2.exe` 进程名回退（仍走 OpenProcess / 创建时间 / 完整性）
- 判定：逻辑 ✅ · 本机真机读窗 / 捕获 ✅（以用户实测为准）

### C. HD2 手动单次填字

- 依赖真实游戏窗口与焦点时序
- 判定：逻辑 ✅ · 本机窗口化填字 / 连续多句 ✅（用户确认）；全矩阵留痕仍建议按清单勾完

---

## 📝 文本矩阵

| ID | 检查点 | 代码依据 | 判定 |
| --- | --- | --- | --- |
| T01 短 BMP 中文 | 完整、顺序 | 逐 `char` 收集，不重排 | ✅ |
| T02 中英数字标点 | ASCII 与中文不重排 | 同上 | ✅ |
| T03 非 BMP `测试𠮷字符` | 代理对不拆分 | 测试 `chunks_by_scalar...` | ✅ |
| T04 边界长度=上限 | 允许、不截断 | `scalar_count > limit` 才拒；`==limit` 通过 | ✅ |
| T05 上限+1 | 发送前拒绝 | `TextError::TooLong`（测试覆盖） | ✅ |
| T06 控制字符 | 清理可预览、不产生回车 | `is_disallowed`：CR/LF/NUL/C0/C1/U+2028/U+2029 | ✅ |
| T07 多批文本 | 顺序、无重复/遗漏 | `chunks(batch_size=5)` 顺序编码 | ✅ |
| T08 清理后为空 | 明确拒绝 | `TextError::Empty`（测试覆盖） | ✅ |
| T09 IME 候选 Enter | 不触发填字 | `useCompositionLatch`（5 项前端测试） | ✅ |
| T10 长按提交键 | 只响应稳定释放后一次 | 前端 `event.repeat` 忽略；后端 `wait_for_input_release` 需连续 3 次全释放 | ✅ |

---

## 🛡 中止与误投矩阵

| ID | 预期 | 代码依据 | 判定 |
| --- | --- | --- | --- |
| F01 捕获后切记事本再填 | 拒绝，记事本无文字 | `restore_foreground` 身份不符→`Ok(false)`→`TargetChanged`，不进注入 | ✅(逻辑)/🟡 |
| F02 恢复期 Alt+Tab 抢焦点 | 中止，新窗口不收批次 | 1.5s `validate_foreground` 只认原目标，否则 `TargetChanged` | ✅(逻辑)/🟡 |
| F03 捕获后最小化游戏 | 拒绝发送 | `window_is_available` 查 `!IsIconic` | ✅(逻辑)/🟡 |
| F04 关闭并重开游戏 | 旧快照失效 | `TargetIdentity` 含 `process_creation_time`+PID+HWND+线程，`identity_for` 比对不符→失效（防 PID 复用） | ✅(逻辑)/🟡 |
| F05 同标题普通窗口 | 不得仅凭标题过 | 复核为**身份全等**（非仅 title），`validate_foreground` 同时要 identity 匹配 | ✅(逻辑)/🟡 |
| F06 多批中切焦点 | 停止并报可能有部分前缀 | `injector` 每批前校验；`partial_prefix_possible = successful_events>0` | ✅(逻辑)/🟡 |
| F07 游戏管理员/工具普通 | 诊断不兼容，不盲发 | `integrity.rs` `current>=target` 为 false→`IntegrityIncompatible`（发生在注入前） | ✅(逻辑)/🟡 |
| F08 面板仍在 composition | 不进入恢复/填字 | `canSubmit` 要求 `!isComposing && !isLatched`；`onKeydown` 早退 | ✅ |
| F09 连点两次填字 | 只一个事务，第二次拒绝 | `injection_gate.try_lock` + 会话 `Injecting`→`InjectionInProgress`（测试 `submit_release_gates_injection_and_transaction_is_single`） | ✅ |
| F10 SendInput 部分返回 | 立即失败，不重试/补键 | `inserted != len`→`SendInputFailed`；无重试；`key_state_uncertain` 置位并锁后续 | ✅(逻辑)/🟡 |
| F11 标题暂时不匹配 | 拒绝，不放宽 | `validate_target`/`validate_foreground` 要求 `title_matches` | ✅(逻辑)/🟡 |
| F12 修饰键+Enter | 不触发新会话或提交 | 前端 `submitOnEnterRelease = !(ctrl||alt||shift||meta)`；后端 `all_released` 要求无修饰键（测试 `modified_enter_does_not_trigger`） | ✅ |

> **发布阻断项 F01/F02/F06**：代码路径均以“中止 + 不向未确认目标发送”为唯一出口，符合设计；真机须实测确认无文字进入错误窗口。

---

## 🚨 门槛（流程 / 发布项）

| 项 | 判定 |
| --- | --- |
| Win10 / Win11 物理机、无边框 + 窗口化各自通过、反作弊无告警 | ⚠️ 真机 + 人工，代码无法自证 |
| 无后台自动 Enter 监听 / 无托盘无人值守发送 | ✅ 当前源码确无这些接口 |
| 本地 NSIS 安装包（自用） | ✅ 已可生成；**不等于**对外宣称稳定发布 |

---

## ✅ 结论

- **A 层**：源码与自动化测试覆盖完整，可判定通过
- **B / C 层**：按「中止优先、绝不盲发、不自动 Enter/Esc」实现；本机已验证记事本 + HD2 基础链路；F01–F12 仍建议按清单系统勾完留痕
- **未发现需立刻改代码的设计缺口**
- 下一步：按 [`manual-rounds-checklist.md`](manual-rounds-checklist.md) 勾阻断项，用结果模板留痕
