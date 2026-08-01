import { invoke } from '@tauri-apps/api/core'

import type {
  CalibrationPreview,
  ChatTranslationResult,
  DiagnosticLogsView,
  InjectionReport,
  InjectionResult,
  IntegrityDiagnostic,
  IpcError,
  OcrLanguage,
  ProbeSession,
  RustTargetDiagnostic,
  SessionSnapshot,
  TargetDiagnostic,
  TextPreview,
  TranslationSettingsUpdate,
  TranslationSettingsView,
} from '@/types/ipc'

export const IPC_COMMANDS = {
  getTargetDiagnostic: 'get_target_diagnostic',
  getDiagnosticLogs: 'get_diagnostic_logs',
  clearDiagnosticLogs: 'clear_diagnostic_logs',
  beginProbeSession: 'begin_probe_session',
  previewText: 'preview_text',
  injectProbeText: 'inject_probe_text',
  getIntegrityDiagnostic: 'get_integrity_diagnostic',
  getSessionState: 'get_session_state',
  cancelSession: 'cancel_session',
  getTranslationSettings: 'get_translation_settings',
  saveTranslationSettings: 'save_translation_settings',
  testTranslationApi: 'test_translation_api',
  translateOutgoingText: 'translate_outgoing_text',
  sendQuickShout: 'send_quick_shout',
  cancelOverlayChat: 'cancel_overlay_chat',
  listOcrLanguages: 'list_ocr_languages',
  captureChatCalibrationPreview: 'capture_chat_calibration_preview',
  translateChatCapture: 'translate_chat_capture',
} as const

const browserMessage = '当前为浏览器预览环境。目标检测与文字填入仅在 Windows Tauri 应用中可用。'

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__ || window.__TAURI__)
}

function browserDiagnostic(): TargetDiagnostic {
  return {
    platform: 'browser',
    status: 'unsupported_platform',
    valid: false,
    matched: false,
    title: null,
    visible: false,
    minimized: false,
    cloaked: false,
    identity: null,
    integrity: null,
    message: browserMessage,
    checkedAt: Date.now(),
  }
}

function normalizeError(error: unknown): IpcError {
  if (typeof error === 'object' && error !== null && 'code' in error && 'message' in error) {
    const candidate = error as Partial<IpcError>
    return {
      code: candidate.code ?? 'INTERNAL_STATE',
      message: candidate.message ?? '与本地核心通信失败。',
      partialPrefixPossible: candidate.partialPrefixPossible ?? false,
      report: candidate.report ?? null,
    }
  }
  return {
    code: 'INTERNAL_STATE',
    message: error instanceof Error ? error.message : String(error),
    partialPrefixPossible: false,
    report: null,
  }
}

export function normalizeTarget(
  raw: RustTargetDiagnostic,
  integrity: IntegrityDiagnostic | null,
): TargetDiagnostic {
  const status = !raw.supported
    ? 'unsupported_platform'
    : !raw.identity || !raw.isWindow
      ? 'not_found'
      : !raw.titleMatches
        ? 'title_mismatch'
        : !raw.visible
          ? 'not_visible'
          : raw.minimized
            ? 'minimized'
            : raw.cloaked
              ? 'cloaked'
              : 'ready'
  const effectiveStatus =
    status !== 'ready'
      ? status
      : integrity?.compatible === false
        ? 'permission_mismatch'
        : integrity?.compatible === true
          ? 'ready'
          : 'permission_unknown'
  const valid = effectiveStatus === 'ready'
  const messages: Record<TargetDiagnostic['status'], string> = {
    ready: integrity?.compatible === null ? '已锁定游戏窗口；权限级别尚未确认。' : '已锁定可用的游戏前台窗口。',
    not_found: '未找到可读取的前台窗口，请先切回游戏。',
    title_mismatch: '当前前台窗口标题不匹配 HELLDIVERS。',
    not_visible: '目标窗口当前不可见。',
    minimized: '目标窗口已最小化，请恢复后重试。',
    cloaked: '目标窗口被系统隐藏，请切换到可见窗口。',
    permission_mismatch: '工具权限低于游戏，Windows 会阻止输入注入。',
    permission_unknown: '已识别游戏窗口，但无法确认权限级别。',
    unsupported_platform: browserMessage,
    error: '目标检测失败。',
  }

  return {
    platform: 'windows',
    status: effectiveStatus,
    valid,
    matched: raw.titleMatches,
    title: raw.title || null,
    visible: raw.visible,
    minimized: raw.minimized,
    cloaked: raw.cloaked,
    identity: raw.identity,
    integrity,
    message: messages[effectiveStatus],
    checkedAt: Date.now(),
  }
}

export async function getTargetDiagnostic(): Promise<TargetDiagnostic> {
  if (!isTauriRuntime()) return browserDiagnostic()

  try {
    const raw = await invoke<RustTargetDiagnostic>(IPC_COMMANDS.getTargetDiagnostic)
    let integrity: IntegrityDiagnostic | null = null
    if (raw.titleMatches && raw.identity && raw.visible && !raw.minimized && !raw.cloaked) {
      try {
        integrity = await invoke<IntegrityDiagnostic>(IPC_COMMANDS.getIntegrityDiagnostic)
      } catch {
        integrity = null
      }
    }
    return normalizeTarget(raw, integrity)
  } catch (error) {
    const ipcError = normalizeError(error)
    return {
      ...browserDiagnostic(),
      platform: 'windows',
      status: 'error',
      message: ipcError.message,
    }
  }
}

export async function beginProbeSession(): Promise<ProbeSession> {
  if (!isTauriRuntime()) throw normalizeError({ code: 'UNSUPPORTED_PLATFORM', message: browserMessage })
  try {
    return await invoke<ProbeSession>(IPC_COMMANDS.beginProbeSession)
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function previewText(text: string): Promise<TextPreview> {
  if (!isTauriRuntime()) {
    const characters = Array.from(text)
    const cleanedText = characters.filter((character) => !isDisallowed(character)).join('')
    const cleanedCharacters = Array.from(cleanedText)
    const batches = Array.from({ length: Math.ceil(cleanedCharacters.length / 5) }, (_, index) =>
      cleanedCharacters.slice(index * 5, index * 5 + 5).join(''),
    )
    return {
      cleanedText,
      scalarCount: cleanedCharacters.length,
      utf16Batches: batches.map((batch) => Array.from(batch).flatMap(toUtf16Units)),
    }
  }
  try {
    return await invoke<TextPreview>(IPC_COMMANDS.previewText, { text })
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function injectProbeText(
  generation: string,
  text: string,
  submit: boolean,
): Promise<InjectionResult> {
  if (!isTauriRuntime()) {
    const error = normalizeError({ code: 'UNSUPPORTED_PLATFORM', message: browserMessage })
    return { ok: false, message: error.message, report: null, error }
  }

  try {
    const report = await invoke<InjectionReport>(IPC_COMMANDS.injectProbeText, {
      generation,
      text,
      submit,
    })
    return {
      ok: true,
      message: submit ? '文字已发送到游戏聊天。' : '文字已填入游戏聊天框。',
      report,
      error: null,
    }
  } catch (error) {
    const ipcError = normalizeError(error)
    return { ok: false, message: ipcError.message, report: ipcError.report, error: ipcError }
  }
}

const browserTranslationSettings: TranslationSettingsView = {
  apiUrl: '',
  proxyUrl: '',
  apiKeyConfigured: false,
  model: '',
  ocrLanguage: 'auto',
  captureHotkey: 'CommandOrControl+Shift+T',
  chatRegion: null,
  incomingPrompt: '',
  outgoingPrompt: '',
  gameOverlayEnabled: true,
  overlayChatKey: 'Enter',
  autoLockCaps: true,
  gameInputMethod: 'gbkAltCode',
  quickShoutFocusDelayMs: 500,
  quickShouts: [],
}

export async function getTranslationSettings(): Promise<TranslationSettingsView> {
  if (!isTauriRuntime()) return { ...browserTranslationSettings }
  try {
    return await invoke<TranslationSettingsView>(IPC_COMMANDS.getTranslationSettings)
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function saveTranslationSettings(
  settings: TranslationSettingsUpdate,
): Promise<TranslationSettingsView> {
  if (!isTauriRuntime()) {
    return {
      ...browserTranslationSettings,
      ...settings,
      apiKeyConfigured: settings.apiKey !== undefined && settings.apiKey.length > 0,
    }
  }
  try {
    return await invoke<TranslationSettingsView>(IPC_COMMANDS.saveTranslationSettings, { settings })
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function testTranslationApi(): Promise<string> {
  if (!isTauriRuntime()) {
    throw normalizeError({ code: 'UNSUPPORTED_PLATFORM', message: browserMessage })
  }
  try {
    return await invoke<string>(IPC_COMMANDS.testTranslationApi)
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function translateOutgoingText(text: string): Promise<string> {
  if (!isTauriRuntime()) {
    throw normalizeError({ code: 'UNSUPPORTED_PLATFORM', message: browserMessage })
  }
  try {
    return await invoke<string>(IPC_COMMANDS.translateOutgoingText, { text })
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function sendQuickShout(
  text: string,
  generation?: string,
  openChat = true,
): Promise<InjectionResult> {
  if (!isTauriRuntime()) {
    const error = normalizeError({ code: 'UNSUPPORTED_PLATFORM', message: browserMessage })
    return { ok: false, message: error.message, report: null, error }
  }
  try {
    const report = await invoke<InjectionReport>(IPC_COMMANDS.sendQuickShout, {
      text,
      generation: generation ?? null,
      openChat,
    })
    return { ok: true, message: '快捷喊话已发送。', report, error: null }
  } catch (error) {
    const ipcError = normalizeError(error)
    return { ok: false, message: ipcError.message, report: ipcError.report, error: ipcError }
  }
}

export async function cancelOverlayChat(): Promise<void> {
  if (!isTauriRuntime()) {
    throw normalizeError({ code: 'UNSUPPORTED_PLATFORM', message: browserMessage })
  }
  try {
    await invoke<void>(IPC_COMMANDS.cancelOverlayChat)
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function getDiagnosticLogs(): Promise<DiagnosticLogsView> {
  if (!isTauriRuntime()) return { path: '', content: '' }
  try {
    return await invoke<DiagnosticLogsView>(IPC_COMMANDS.getDiagnosticLogs)
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function clearDiagnosticLogs(): Promise<DiagnosticLogsView> {
  if (!isTauriRuntime()) return { path: '', content: '' }
  try {
    return await invoke<DiagnosticLogsView>(IPC_COMMANDS.clearDiagnosticLogs)
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function listOcrLanguages(): Promise<OcrLanguage[]> {
  if (!isTauriRuntime()) {
    return [
      {
        tag: 'en-US',
        displayName: 'English (United States)',
        nativeName: 'English (United States)',
      },
    ]
  }
  try {
    return await invoke<OcrLanguage[]>(IPC_COMMANDS.listOcrLanguages)
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function captureChatCalibrationPreview(
  generation: string,
): Promise<CalibrationPreview> {
  if (!isTauriRuntime()) {
    throw normalizeError({ code: 'UNSUPPORTED_PLATFORM', message: browserMessage })
  }
  try {
    return await invoke<CalibrationPreview>(IPC_COMMANDS.captureChatCalibrationPreview, {
      generation,
    })
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function translateChatCapture(generation: string): Promise<ChatTranslationResult> {
  if (!isTauriRuntime()) {
    throw normalizeError({ code: 'UNSUPPORTED_PLATFORM', message: browserMessage })
  }
  try {
    return await invoke<ChatTranslationResult>(IPC_COMMANDS.translateChatCapture, { generation })
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function getSessionState(): Promise<SessionSnapshot> {
  if (!isTauriRuntime()) {
    return { generation: '0', phase: 'idle', target: null, draft: '', lastError: browserMessage }
  }
  try {
    return await invoke<SessionSnapshot>(IPC_COMMANDS.getSessionState)
  } catch (error) {
    throw normalizeError(error)
  }
}

export async function cancelSession(generation?: string): Promise<SessionSnapshot> {
  if (!isTauriRuntime()) {
    return { generation: '0', phase: 'idle', target: null, draft: '', lastError: null }
  }
  try {
    return await invoke<SessionSnapshot>(IPC_COMMANDS.cancelSession, { generation })
  } catch (error) {
    throw normalizeError(error)
  }
}

function isDisallowed(character: string): boolean {
  const codePoint = character.codePointAt(0) ?? 0
  return (
    character === '\r' ||
    character === '\n' ||
    character === '\0' ||
    character === '\u2028' ||
    character === '\u2029' ||
    (codePoint >= 0x0001 && codePoint <= 0x001f) ||
    (codePoint >= 0x007f && codePoint <= 0x009f)
  )
}

function toUtf16Units(character: string): number[] {
  if (character.length === 1) return [character.charCodeAt(0)]
  return [character.charCodeAt(0), character.charCodeAt(1)]
}
