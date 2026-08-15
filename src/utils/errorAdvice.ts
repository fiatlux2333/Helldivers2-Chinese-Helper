import type { IpcError, IpcErrorCode } from '@/types/ipc'

export interface ErrorAdviceContext {
  title?: string
}

export type ErrorRecoveryAction =
  | 'recaptureTarget'
  | 'calibrateRegion'
  | 'exportLogs'
  | 'openSettings'
  | 'openStratagem'
  | 'testApi'
  | 'focusComposer'

const DEFAULT_ERROR_ADVICE = '先回到 HD2 确认目标仍可用；如果还失败，请导出诊断日志发给作者。'

const TARGET_ADVICE = '切回 HD2，确认游戏没有最小化，然后重新捕获目标；刚切屏时等 1 秒再试。'
const WINDOW_VISIBILITY_ADVICE = '把 HD2 恢复到前台，建议使用无边框或窗口化；关闭可能遮挡游戏的覆盖层后再试。'
const PERMISSION_ADVICE = '关闭游戏和助手后重新打开，优先都不要管理员运行；如果游戏必须管理员运行，助手也必须管理员运行。'
const SEND_ADVICE = '先看 HD2 聊天框：如果文字已经在框里，手动按 Enter；如果没进框，重新捕获后直接在助手里发送，不用先手动打开聊天框。'
const PARTIAL_SEND_ADVICE = '文字可能只进了一部分。先看 HD2 聊天框，不要直接重发整段；确认聊天框内容后手动补齐或清空重来。'
const FINAL_SUBMIT_ADVICE = '文字可能已经在游戏聊天框里，但最后一次 Enter 没成功。先切回 HD2 检查，能看到文字就手动按 Enter。'
const OPEN_CHAT_ADVICE = '助手没能稳定打开游戏聊天框。确认助手里的“游戏聊天键”和 HD2 设置一致；连续失败时，回 HD2 手动按一次聊天键验证聊天框能正常打开。'
const INPUT_STATE_ADVICE = '完全松开 Enter、Esc、Ctrl、Alt、Shift、Win，等半秒再试；不要长按发送键。'
const CAPS_ADVICE = '先松开所有功能键并按一次 Esc；如果键盘还像被锁住，先导出诊断日志，再关闭助手重开。'
const API_CONFIG_ADVICE = '检查 API 地址、模型名和 Key；保存设置后先点接口测试。'
const API_REQUEST_ADVICE = '检查网络、代理和 API 服务是否可访问；代理变更后保存设置再测试。'
const API_RESPONSE_ADVICE = '确认接口兼容 OpenAI Chat Completions，模型可用；必要时查看诊断日志里的返回内容。'
const OCR_REGION_ADVICE = '重新框选有英文聊天文字的区域，别只框到空背景或边框；HDR 或聊天背景太透明时，先调清楚再试。'
const OCR_SETUP_ADVICE = '确认 Windows OCR 组件可用；重新打开助手后再校准聊天区域。'
const HOTKEY_ADVICE = '这个热键可能被占用或没有注册成功。换一个热键，保存设置后再试；如果是游戏内热键，确认助手和游戏权限一致。'
const OVERLAY_FOCUS_ADVICE = '按一次助手唤回热键让侧栏重新获取焦点；如果文字还进游戏聊天框，先点一下侧栏输入框再试。'
const STRATAGEM_ADVICE = '确认 HD2 在前台且聊天框没打开；如果方向没反应，在战备页把方向按键改成与你游戏设置一致的 WASD 或 ↑↓←→。'
const PROCESS_ADVICE = '如果退出后双击没反应，等 10 秒；仍不行就在任务管理器结束“Helldivers 2 中文助手”，重新打开后导出诊断日志反馈。'

const ADVICE_BY_CODE: Partial<Record<IpcErrorCode, string>> = {
  UNSUPPORTED_PLATFORM: '这个功能只能在 Windows 桌面版助手里使用；浏览器预览不能控制游戏。',
  TARGET_NOT_MATCHED: TARGET_ADVICE,
  TARGET_CHANGED: TARGET_ADVICE,
  WINDOW_UNAVAILABLE: TARGET_ADVICE,
  WINDOW_NOT_VISIBLE: WINDOW_VISIBILITY_ADVICE,
  WINDOW_MINIMIZED: WINDOW_VISIBILITY_ADVICE,
  WINDOW_CLOAKED: WINDOW_VISIBILITY_ADVICE,
  INTEGRITY_INCOMPATIBLE: PERMISSION_ADVICE,
  TEXT_EMPTY: '先输入要发送的内容。',
  TEXT_TOO_LONG: '缩短内容后再发送，或拆成多条发送。',
  TEXT_ENCODING_UNSUPPORTED: '切到 Unicode 输入方式后再试；GBK 路径不适合这段内容。',
  KEYBOARD_LAYOUT_UNAVAILABLE: '切回中文或英文键盘布局后再试；如果刚切过输入法，等半秒。',
  SEND_INPUT_PARTIAL: SEND_ADVICE,
  FINAL_SUBMIT_FAILED: FINAL_SUBMIT_ADVICE,
  API_CONFIGURATION: API_CONFIG_ADVICE,
  API_REQUEST_FAILED: API_REQUEST_ADVICE,
  API_RESPONSE_INVALID: API_RESPONSE_ADVICE,
  SETTINGS_STORAGE_FAILED: '确认助手目录可写，关闭杀毒或同步软件占用后再保存。',
  CAPTURE_FAILED: '切回 HD2，确认无边框或窗口化且没有最小化，关闭遮挡画面的覆盖层后重新捕获。',
  OCR_UNAVAILABLE: OCR_SETUP_ADVICE,
  OCR_EMPTY: OCR_REGION_ADVICE,
  INVALID_CAPTURE_REGION: '先到聊天翻译页重新校准聊天区域，再截图翻译。',
  INVALID_SESSION: '重新捕获 HD2 后直接在助手里发送，不用先手动打开聊天框。',
  SUBMIT_KEY_STILL_DOWN: INPUT_STATE_ADVICE,
  INPUT_STATE_UNCERTAIN: CAPS_ADVICE,
  INTERNAL_STATE: '先重启助手并重新捕获；仍失败就导出诊断日志发给作者。',
}

const ACTIONS_BY_CODE: Partial<Record<IpcErrorCode, ErrorRecoveryAction[]>> = {
  TARGET_NOT_MATCHED: ['recaptureTarget', 'exportLogs'],
  TARGET_CHANGED: ['recaptureTarget', 'exportLogs'],
  WINDOW_UNAVAILABLE: ['recaptureTarget', 'exportLogs'],
  WINDOW_NOT_VISIBLE: ['recaptureTarget', 'exportLogs'],
  WINDOW_MINIMIZED: ['recaptureTarget', 'exportLogs'],
  WINDOW_CLOAKED: ['recaptureTarget', 'exportLogs'],
  INTEGRITY_INCOMPATIBLE: ['exportLogs'],
  SEND_INPUT_PARTIAL: ['recaptureTarget', 'exportLogs'],
  FINAL_SUBMIT_FAILED: ['focusComposer', 'exportLogs'],
  API_CONFIGURATION: ['openSettings', 'testApi'],
  API_REQUEST_FAILED: ['openSettings', 'testApi', 'exportLogs'],
  API_RESPONSE_INVALID: ['openSettings', 'testApi', 'exportLogs'],
  SETTINGS_STORAGE_FAILED: ['openSettings', 'exportLogs'],
  CAPTURE_FAILED: ['recaptureTarget', 'exportLogs'],
  OCR_UNAVAILABLE: ['calibrateRegion', 'exportLogs'],
  OCR_EMPTY: ['calibrateRegion', 'exportLogs'],
  INVALID_CAPTURE_REGION: ['calibrateRegion'],
  INVALID_SESSION: ['recaptureTarget'],
  SUBMIT_KEY_STILL_DOWN: ['focusComposer'],
  INPUT_STATE_UNCERTAIN: ['focusComposer', 'exportLogs'],
  INTERNAL_STATE: ['exportLogs'],
}

export function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    return String((error as { message: unknown }).message)
  }
  return String(error)
}

export function getErrorAdvice(error: unknown, context: ErrorAdviceContext = {}): string | null {
  const ipcError = getIpcError(error)
  if (ipcError) return adviceFromIpcError(ipcError) ?? null

  return adviceFromText(`${context.title ?? ''} ${getErrorMessage(error)}`)
}

export function getErrorActions(error: unknown, context: ErrorAdviceContext = {}): ErrorRecoveryAction[] {
  const ipcError = getIpcError(error)
  if (ipcError) return uniqueActions(ACTIONS_BY_CODE[ipcError.code] ?? [])

  const actions = actionsFromText(`${context.title ?? ''} ${getErrorMessage(error)}`)
  if (actions.length > 0) return actions
  if (error instanceof Error || (typeof error === 'object' && error !== null && 'message' in error)) {
    return ['exportLogs']
  }
  return []
}

export function errorMessageWithAdvice(error: unknown, context: ErrorAdviceContext = {}): string {
  return appendAdvice(getErrorMessage(error), getErrorAdvice(error, context) ?? DEFAULT_ERROR_ADVICE)
}

export function errorTextWithAdvice(message: string, context: ErrorAdviceContext = {}): string {
  return appendAdvice(message, getErrorAdvice(message, context))
}

function appendAdvice(message: string, advice: string | null): string {
  if (!advice || message.includes('建议：')) return message
  return `${message}\n建议：${advice}`
}

function getIpcError(error: unknown): IpcError | null {
  return isIpcError(error) ? error : null
}

function isIpcError(error: unknown): error is IpcError {
  if (typeof error !== 'object' || error === null) return false
  const candidate = error as Partial<IpcError>
  return typeof candidate.code === 'string' && typeof candidate.message === 'string'
}

function adviceFromText(text: string): string | null {
  if (/退出.*后台|后台.*进程|进程.*残留|双击.*没反应|无法关闭程序|任务管理器/.test(text)) {
    return PROCESS_ADVICE
  }
  if (/HotKey already registered|already registered|热键.*注册失败|热键.*占用|快捷键.*占用/.test(text)) {
    return HOTKEY_ADVICE
  }
  if (/权限|管理员|完整性|integrity/i.test(text)) return PERMISSION_ADVICE
  if (/CapsLock|大写|输入法保护|键盘.*锁|INPUT_STATE_UNCERTAIN/.test(text)) return CAPS_ADVICE
  if (/接口|API|Key|模型|代理|网络/.test(text)) return API_REQUEST_ADVICE
  if (/战备|搓球|Stratagem/i.test(text)) return STRATAGEM_ADVICE
  if (/悬浮输入栏|中文侧栏|侧栏|输入框|焦点|focus/i.test(text)) return OVERLAY_FOCUS_ADVICE
  if (/目标|捕获|窗口|前台|最小化|HD2|HELLDIVERS/i.test(text)) return TARGET_ADVICE
  if (/OCR|聊天翻译|截图翻译|读取聊天区域|校准|框选|区域/.test(text)) return OCR_REGION_ADVICE
  if (/发送|发言|中译英|中文发送|快捷喊话|提交|注入|SendInput/i.test(text)) return SEND_ADVICE
  return null
}

function adviceFromIpcError(error: IpcError): string | null {
  if (error.code === 'SEND_INPUT_PARTIAL') {
    if (/聊天框|open_chat|OpenChat|准备游戏聊天框|打开游戏聊天框/.test(error.message)) {
      return OPEN_CHAT_ADVICE
    }
    if (error.partialPrefixPossible || error.report?.partialPrefixPossible) {
      return PARTIAL_SEND_ADVICE
    }
  }
  if (error.code === 'INPUT_STATE_UNCERTAIN') return CAPS_ADVICE
  return ADVICE_BY_CODE[error.code] ?? null
}

function actionsFromText(text: string): ErrorRecoveryAction[] {
  if (/退出.*后台|后台.*进程|进程.*残留|双击.*没反应|无法关闭程序|任务管理器/.test(text)) {
    return ['exportLogs']
  }
  if (/HotKey already registered|already registered|热键.*注册失败|热键.*占用|快捷键.*占用|热键没有用|快捷键没有用/.test(text)) {
    return ['openSettings', 'exportLogs']
  }
  if (/权限|管理员|完整性|integrity/i.test(text)) return ['exportLogs']
  if (/CapsLock|大写|输入法保护|键盘.*锁|INPUT_STATE_UNCERTAIN/.test(text)) return ['focusComposer', 'exportLogs']
  if (/接口|API|Key|模型|代理|网络/.test(text)) return ['openSettings', 'testApi', 'exportLogs']
  if (/战备|搓球|Stratagem|方向键|WASD/i.test(text)) return ['openStratagem', 'exportLogs']
  if (/悬浮输入栏|中文侧栏|侧栏|输入框|焦点|focus/i.test(text)) return ['focusComposer', 'exportLogs']
  if (/目标|捕获|窗口|前台|最小化|HD2|HELLDIVERS/i.test(text)) return ['recaptureTarget', 'exportLogs']
  if (/OCR|聊天翻译|截图翻译|读取聊天区域|校准|框选|区域/.test(text)) return ['calibrateRegion', 'exportLogs']
  if (/发送|发言|中译英|中文发送|快捷喊话|提交|注入|SendInput/i.test(text)) {
    return ['recaptureTarget', 'exportLogs']
  }
  return []
}

function uniqueActions(actions: ErrorRecoveryAction[]): ErrorRecoveryAction[] {
  return Array.from(new Set(actions))
}
