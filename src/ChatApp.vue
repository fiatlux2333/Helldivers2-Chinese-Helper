<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'

import TargetDiagnosticPanel from '@/components/TargetDiagnosticPanel.vue'
import { useCaptureHotkey } from '@/composables/useCaptureHotkey'
import { useCompositionLatch } from '@/composables/useCompositionLatch'
import { useGameOverlayWindow } from '@/composables/useGameOverlayWindow'
import { useInputHistory } from '@/composables/useInputHistory'
import { useRestoreHotkey } from '@/composables/useRestoreHotkey'
import { useQuickShoutHotkeys } from '@/composables/useQuickShoutHotkeys'
import {
  beginProbeSession,
  cancelSession,
  cancelOverlayChat,
  captureChatCalibrationPreview,
  checkForUpdates,
  getTargetDiagnostic,
  getTranslationSettings,
  injectProbeText,
  isTauriRuntime,
  listOcrLanguages,
  normalizeTarget,
  previewText,
  saveTranslationSettings,
  sendQuickShout,
  testTranslationApi,
  translateChatCapture,
  translateOutgoingText,
} from '@/services/tauriApi'
import type {
  CalibrationPreview,
  ChatTranslationLine,
  NormalizedRegion,
  OcrLanguage,
  QuickShout,
  TargetDiagnostic,
  TranslationSettingsUpdate,
  TranslationSettingsView,
  UpdateCheckView,
} from '@/types/ipc'
import { submitIntentFromKeydown, type SubmitIntent } from '@/utils/submit'
import { acceleratorFromKeyboardEvent, formatHotkeyLabel } from '@/utils/hotkey'

const CHARACTER_LIMIT = 100
const DEFAULT_CAPTURE_HOTKEY = 'CommandOrControl+Shift+T'
const MIN_QUICK_SHOUT_FOCUS_DELAY_MS = 300
const MAX_QUICK_SHOUT_FOCUS_DELAY_MS = 1_200
const QUICK_SHOUT_FOCUS_DELAY_STEP_MS = 50
const DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS = 500
const desktopRuntime = isTauriRuntime()
const previewParams = new URLSearchParams(window.location.search)
const overlayPreview = import.meta.env.DEV && previewParams.has('overlay-preview')
const updatePreview = import.meta.env.DEV && previewParams.has('update-preview')

type NoticeTone = 'idle' | 'working' | 'success' | 'error'
type AppView = 'compose' | 'translate' | 'settings'
type OutgoingMode = 'direct' | 'translate'

interface TranslationHistoryItem {
  id: number
  lines: ChatTranslationLine[]
  messageOcrLanguage: string
  speakerOcrLanguage: string
  translatedAt: number
}

const inputRef = ref<HTMLInputElement | null>(null)
const overlayInputRef = ref<HTMLInputElement | null>(null)
const activeView = ref<AppView>('compose')
const outgoingMode = ref<OutgoingMode>('direct')
const text = ref('')
const target = ref<TargetDiagnostic | null>(null)
const activeGeneration = ref<string | null>(null)
const chatInputLikelyOpen = ref(false)
const isRefreshing = ref(false)
const isCapturing = ref(false)
const isSending = ref(false)
const isTranslatingChat = ref(false)
const isSavingSettings = ref(false)
const isTestingApi = ref(false)
const isCheckingUpdates = ref(false)
const isCalibrating = ref(false)
const isQuickShouting = ref(false)
const quickHotkeyRecordingIndex = ref<number | null>(null)
const overlayChatKeyRecording = ref(false)
const overlayArmed = ref(false)
const captureToken = ref(0)
const submitOnEnterRelease = ref<SubmitIntent | null>(null)
const cancelOnEscapeRelease = ref(false)
const noticeTone = ref<NoticeTone>('idle')
const noticeTitle = ref('等待目标')
const noticeMessage = ref('先捕获游戏窗口，再输入消息或配置聊天翻译。')
const lastOutgoingTranslation = ref<string | null>(null)
const translationHistory = ref<TranslationHistoryItem[]>([])
const ocrLanguages = ref<OcrLanguage[]>([])
const calibrationPreview = ref<CalibrationPreview | null>(null)
const calibrationSelection = ref<NormalizedRegion | null>(null)
const selectionStart = ref<{ x: number; y: number } | null>(null)
const previewSurfaceRef = ref<HTMLDivElement | null>(null)
const updateInfo = ref<UpdateCheckView | null>(
  updatePreview
    ? {
      currentVersion: '0.3.0',
      latestVersion: '0.5.0',
      updateAvailable: true,
      releaseUrl: 'https://github.com/fiatlux2333/Helldivers2-Chinese-Helper/releases/tag/v0.5.0',
    }
    : null,
)

const savedSettings = reactive<TranslationSettingsView>({
  apiUrl: '',
  proxyUrl: '',
  apiKeyConfigured: false,
  model: '',
  ocrLanguage: 'auto',
  captureHotkey: DEFAULT_CAPTURE_HOTKEY,
  chatRegion: null,
  incomingPrompt: '',
  outgoingPrompt: '',
  gameOverlayEnabled: true,
  overlayChatKey: 'Enter',
  autoLockCaps: true,
  gameInputMethod: 'gbkAltCode',
  quickShoutFocusDelayMs: DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS,
  quickShouts: [],
})
const settingsDraft = reactive({
  apiUrl: '',
  proxyUrl: '',
  apiKey: '',
  model: '',
  ocrLanguage: 'auto',
  chatRegion: null as NormalizedRegion | null,
  incomingPrompt: '',
  outgoingPrompt: '',
  gameOverlayEnabled: true,
  overlayChatKey: 'Enter',
  autoLockCaps: true,
  gameInputMethod: 'gbkAltCode' as TranslationSettingsUpdate['gameInputMethod'],
  quickShoutFocusDelayMs: DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS,
  quickShouts: [] as QuickShout[],
})

const composition = useCompositionLatch()
const history = useInputHistory(50)
const captureHotkeyValue = ref(DEFAULT_CAPTURE_HOTKEY)
const anyBusy = computed(
  () =>
    isSending.value ||
    isCapturing.value ||
    isTranslatingChat.value ||
    isSavingSettings.value ||
    isTestingApi.value ||
    isCalibrating.value ||
    isQuickShouting.value,
)
const gameOverlayEnabled = computed(() => savedSettings.gameOverlayEnabled && overlayArmed.value)
const gameOverlay = useGameOverlayWindow({
  enabled: gameOverlayEnabled,
  busy: anyBusy,
  onGameForeground: async () => {
    try {
      const diagnostic = await getTargetDiagnostic()
      target.value = diagnostic
    } catch {
      target.value = null
    }
  },
  onComposerFocused: () => {
    overlayInputRef.value?.focus()
  },
  onError: (message) => setNotice('error', '悬浮输入栏异常', message),
})
const showGameOverlay = computed(() => gameOverlay.isCompact.value || overlayPreview)

const restoreHotkey = useRestoreHotkey({
  isSending: anyBusy,
  isCapturing,
  conflictsWith: () => captureHotkeyValue.value,
  onRestored: () => restoreAssistantWindow({ focus: true }),
  onYielded: yieldAssistantWindow,
  onError: (title, message) => setNotice('error', title, message),
  onHotkeyChanged: (label) => setNotice('success', '唤回热键已更新', `当前唤回热键：${label}`),
})

const captureHotkey = useCaptureHotkey(DEFAULT_CAPTURE_HOTKEY, {
  disabled: anyBusy,
  conflictsWith: () => restoreHotkey.accelerator.value,
  onTriggered: () => runChatTranslation(true),
  onChanged: persistCaptureHotkey,
  onError: (title, message) => setNotice('error', title, message),
})

const quickShoutHotkeys = useQuickShoutHotkeys({
  disabled: computed(() => anyBusy.value || quickHotkeyRecordingIndex.value !== null),
  conflictsWith: () => [restoreHotkey.accelerator.value, captureHotkey.accelerator.value],
  onTriggered: (shout) => runQuickShout(shout, 'hotkey'),
  onError: (title, message) => setNotice('error', title, message),
})

const characterCount = computed(() => Array.from(text.value).length)
const hasText = computed(() => text.value.trim().length > 0)
const isOverLimit = computed(() => characterCount.value > CHARACTER_LIMIT)
const canCapture = computed(
  () => desktopRuntime && !anyBusy.value && !composition.isComposing.value && !composition.isLatched.value,
)
const canSubmit = computed(
  () =>
    hasText.value &&
    !isOverLimit.value &&
    !anyBusy.value &&
    !composition.isComposing.value &&
    !composition.isLatched.value &&
    activeGeneration.value !== null &&
    target.value?.valid === true,
)
const canOverlaySubmit = computed(
  () =>
    desktopRuntime &&
    gameOverlay.isCompact.value &&
    hasText.value &&
    !isOverLimit.value &&
    !anyBusy.value &&
    !composition.isComposing.value &&
    !composition.isLatched.value &&
    target.value?.valid === true,
)
const counterTone = computed(() => {
  if (isOverLimit.value) return 'error'
  if (characterCount.value >= CHARACTER_LIMIT * 0.8) return 'warning'
  return 'normal'
})
const calibrationStyle = computed(() => {
  const selection = calibrationSelection.value
  if (!selection) return undefined
  return {
    left: `${selection.x * 100}%`,
    top: `${selection.y * 100}%`,
    width: `${selection.width * 100}%`,
    height: `${selection.height * 100}%`,
  }
})

function setNotice(tone: NoticeTone, title: string, message: string): void {
  noticeTone.value = tone
  noticeTitle.value = title
  noticeMessage.value = message
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    return String((error as { message: unknown }).message)
  }
  return String(error)
}

function focusInput(): void {
  if (showGameOverlay.value) {
    void nextTick(() => overlayInputRef.value?.focus())
  } else if (activeView.value === 'compose') {
    void nextTick(() => inputRef.value?.focus())
  }
}

async function restoreAssistantWindow(options?: { focus?: boolean }): Promise<void> {
  if (!desktopRuntime) return
  await gameOverlay.restoreWindow(options?.focus !== false)
  if (options?.focus !== false) focusInput()
}

async function yieldAssistantWindow(): Promise<void> {
  if (desktopRuntime) await gameOverlay.yieldWindow()
}

function applySettingsView(view: TranslationSettingsView): void {
  Object.assign(savedSettings, view)
  settingsDraft.apiUrl = view.apiUrl
  settingsDraft.proxyUrl = view.proxyUrl
  settingsDraft.apiKey = ''
  settingsDraft.model = view.model
  settingsDraft.ocrLanguage = view.ocrLanguage
  settingsDraft.chatRegion = view.chatRegion
  settingsDraft.incomingPrompt = view.incomingPrompt
  settingsDraft.outgoingPrompt = view.outgoingPrompt
  settingsDraft.gameOverlayEnabled = view.gameOverlayEnabled
  settingsDraft.overlayChatKey = view.overlayChatKey
  settingsDraft.autoLockCaps = view.autoLockCaps
  settingsDraft.gameInputMethod = view.gameInputMethod
  settingsDraft.quickShoutFocusDelayMs = view.quickShoutFocusDelayMs
  settingsDraft.quickShouts = view.quickShouts.map((shout) => ({ ...shout }))
  captureHotkeyValue.value = view.captureHotkey
}

function settingsPayload(apiKey?: string): TranslationSettingsUpdate {
  const payload: TranslationSettingsUpdate = {
    apiUrl: settingsDraft.apiUrl,
    proxyUrl: settingsDraft.proxyUrl,
    model: settingsDraft.model,
    ocrLanguage: settingsDraft.ocrLanguage,
    captureHotkey: captureHotkey.accelerator.value,
    chatRegion: settingsDraft.chatRegion,
    incomingPrompt: settingsDraft.incomingPrompt,
    outgoingPrompt: settingsDraft.outgoingPrompt,
    gameOverlayEnabled: settingsDraft.gameOverlayEnabled,
    overlayChatKey: settingsDraft.overlayChatKey,
    autoLockCaps: settingsDraft.autoLockCaps,
    gameInputMethod: settingsDraft.gameInputMethod,
    quickShoutFocusDelayMs: settingsDraft.quickShoutFocusDelayMs,
    quickShouts: settingsDraft.quickShouts.map((shout) => ({ ...shout })),
  }
  if (apiKey !== undefined) payload.apiKey = apiKey
  return payload
}

async function saveSettings(options?: { clearKey?: boolean; quiet?: boolean }): Promise<boolean> {
  if (isSavingSettings.value) return false
  isSavingSettings.value = true
  try {
    const apiKey = options?.clearKey ? '' : settingsDraft.apiKey.trim() || undefined
    const view = await saveTranslationSettings(settingsPayload(apiKey))
    applySettingsView(view)
    if (desktopRuntime) await quickShoutHotkeys.sync(view.quickShouts)
    if (!options?.quiet) setNotice('success', '设置已保存', '输入栏、注入方式、翻译与 OCR 配置已更新。')
    return true
  } catch (error) {
    setNotice('error', '保存设置失败', errorMessage(error))
    return false
  } finally {
    isSavingSettings.value = false
  }
}

async function setGameOverlayEnabled(enabled: boolean): Promise<void> {
  if (isSavingSettings.value || enabled === savedSettings.gameOverlayEnabled) return
  isSavingSettings.value = true
  try {
    const view = await saveTranslationSettings({
      apiUrl: savedSettings.apiUrl,
      proxyUrl: savedSettings.proxyUrl,
      model: savedSettings.model,
      ocrLanguage: savedSettings.ocrLanguage,
      captureHotkey: savedSettings.captureHotkey,
      chatRegion: savedSettings.chatRegion,
      incomingPrompt: savedSettings.incomingPrompt,
      outgoingPrompt: savedSettings.outgoingPrompt,
      gameOverlayEnabled: enabled,
      overlayChatKey: savedSettings.overlayChatKey,
      autoLockCaps: savedSettings.autoLockCaps,
      gameInputMethod: savedSettings.gameInputMethod,
      quickShoutFocusDelayMs: savedSettings.quickShoutFocusDelayMs,
      quickShouts: savedSettings.quickShouts.map((shout) => ({ ...shout })),
    })
    savedSettings.gameOverlayEnabled = view.gameOverlayEnabled
    settingsDraft.gameOverlayEnabled = view.gameOverlayEnabled
    setNotice(
      'success',
      view.gameOverlayEnabled ? '中文侧栏已开启' : '中文侧栏已关闭',
      view.gameOverlayEnabled ? '游戏聊天键将唤出右侧输入栏。' : '后续消息在助手主界面输入。',
    )
  } catch (error) {
    setNotice('error', '切换输入模式失败', errorMessage(error))
  } finally {
    isSavingSettings.value = false
  }
}

function onGameOverlayToggle(event: Event): void {
  void setGameOverlayEnabled((event.target as HTMLInputElement).checked)
}

async function persistCaptureHotkey(accelerator: string, label: string): Promise<void> {
  captureHotkeyValue.value = accelerator
  const view = await saveTranslationSettings({
    apiUrl: savedSettings.apiUrl,
    proxyUrl: savedSettings.proxyUrl,
    model: savedSettings.model,
    ocrLanguage: savedSettings.ocrLanguage,
    captureHotkey: accelerator,
    chatRegion: savedSettings.chatRegion,
    incomingPrompt: savedSettings.incomingPrompt,
    outgoingPrompt: savedSettings.outgoingPrompt,
    gameOverlayEnabled: savedSettings.gameOverlayEnabled,
    overlayChatKey: savedSettings.overlayChatKey,
    autoLockCaps: savedSettings.autoLockCaps,
    gameInputMethod: savedSettings.gameInputMethod,
    quickShoutFocusDelayMs: savedSettings.quickShoutFocusDelayMs,
    quickShouts: savedSettings.quickShouts.map((shout) => ({ ...shout })),
  })
  applySettingsView(view)
  setNotice('success', '截图热键已更新', `当前截图翻译热键：${label}`)
}

async function testApi(): Promise<void> {
  if (!(await saveSettings({ quiet: true }))) return
  isTestingApi.value = true
  try {
    const response = await testTranslationApi()
    setNotice('success', '接口连接正常', `接口返回：${response}`)
  } catch (error) {
    setNotice('error', '接口测试失败', errorMessage(error))
  } finally {
    isTestingApi.value = false
  }
}

async function runUpdateCheck(manual: boolean): Promise<void> {
  if (!desktopRuntime || isCheckingUpdates.value) return
  isCheckingUpdates.value = true
  try {
    const result = await checkForUpdates()
    updateInfo.value = result
    if (result.updateAvailable) {
      setNotice(
        'working',
        `发现新版本 v${result.latestVersion}`,
        `当前版本 v${result.currentVersion}，请前往 GitHub 下载最新版。`,
      )
    } else if (manual) {
      setNotice('success', '已经是最新版', `当前版本 v${result.currentVersion}。`)
    }
  } catch (error) {
    if (manual) setNotice('error', '检查更新失败', errorMessage(error))
  } finally {
    isCheckingUpdates.value = false
  }
}

async function openLatestRelease(): Promise<void> {
  const releaseUrl = updateInfo.value?.releaseUrl
  if (!releaseUrl) return
  try {
    await openUrl(releaseUrl)
  } catch (error) {
    setNotice('error', '无法打开下载页', errorMessage(error))
  }
}

async function refreshTarget(): Promise<TargetDiagnostic> {
  isRefreshing.value = true
  try {
    const diagnostic = await getTargetDiagnostic()
    target.value = diagnostic
    if (!diagnostic.valid) setNotice('error', '目标不可用', diagnostic.message)
    return diagnostic
  } finally {
    isRefreshing.value = false
  }
}

async function captureTarget(): Promise<void> {
  if (!canCapture.value) return
  if (!desktopRuntime) {
    await refreshTarget()
    return
  }
  const token = captureToken.value + 1
  const previousGeneration = activeGeneration.value
  captureToken.value = token
  isCapturing.value = true
  activeGeneration.value = null
  target.value = null
  try {
    await cancelSession(previousGeneration ?? undefined).catch(() => undefined)
    setNotice('working', '等待游戏窗口', '助手即将最小化，请切换到已打开聊天框的 HD2。')
    await new Promise((resolve) => window.setTimeout(resolve, 700))
    await yieldAssistantWindow()
    await new Promise((resolve) => window.setTimeout(resolve, 2_000))
    if (captureToken.value !== token) return
    const session = await beginProbeSession()
    activeGeneration.value = session.generation
    target.value = normalizeTarget(session.diagnostic, session.integrity)
    if (savedSettings.gameOverlayEnabled) {
      await cancelOverlayChat()
      chatInputLikelyOpen.value = false
      overlayArmed.value = true
    } else {
      chatInputLikelyOpen.value = true
      overlayArmed.value = false
    }
    await restoreAssistantWindow()
    setNotice('success', '目标已锁定', 'Enter 直接发送，Ctrl+Enter 只填入。')
  } catch (error) {
    activeGeneration.value = null
    chatInputLikelyOpen.value = false
    target.value = null
    overlayArmed.value = false
    await restoreAssistantWindow().catch(() => undefined)
    setNotice('error', '目标捕获失败', errorMessage(error))
  } finally {
    isCapturing.value = false
  }
}

async function clearDraft(): Promise<void> {
  if (isSending.value) return
  const generation = activeGeneration.value
  captureToken.value += 1
  text.value = ''
  submitOnEnterRelease.value = null
  cancelOnEscapeRelease.value = false
  activeGeneration.value = null
  chatInputLikelyOpen.value = false
  target.value = null
  overlayArmed.value = false
  history.resetBrowsing()
  composition.reset()
  await cancelSession(generation ?? undefined).catch(() => undefined)
  setNotice('idle', '已清空', '草稿和目标会话已清除。')
  focusInput()
}

async function submit(intent: SubmitIntent): Promise<void> {
  if (!canSubmit.value) return
  const generation = activeGeneration.value
  if (!generation) return
  const sourceText = text.value
  isSending.value = true
  setNotice('working', outgoingMode.value === 'translate' ? '正在翻译并准备发送' : '正在准备发送', intent === 'send' ? '目标验证通过后将填入文字并发送最终 Enter。' : '本次仅填入文字。')
  try {
    const outgoingText = outgoingMode.value === 'translate' ? await translateOutgoingText(sourceText) : sourceText
    if (outgoingMode.value === 'translate') lastOutgoingTranslation.value = outgoingText
    const preview = await previewText(outgoingText)
    if (!preview.cleanedText.trim()) throw new Error('没有可发送的文字')
    if (preview.scalarCount > CHARACTER_LIMIT) throw new Error(`最终文本共 ${preview.scalarCount} 字符，超过 ${CHARACTER_LIMIT} 字符限制`)
    await yieldAssistantWindow()
    await new Promise((resolve) => window.setTimeout(resolve, 180))
    const result = await injectProbeText(generation, preview.cleanedText, intent === 'send')
    if (!result.ok) {
      activeGeneration.value = null
      target.value = null
      chatInputLikelyOpen.value = false
      await restoreAssistantWindow().catch(() => undefined)
      throw new Error(result.message)
    }
    history.add(sourceText)
    text.value = ''
    chatInputLikelyOpen.value = intent === 'fill'
    setNotice('success', intent === 'send' ? '消息已提交' : '文字已填入', intent === 'send' ? '请在游戏中确认发送结果；下一条消息前先重新打开聊天框。' : '游戏保持前台，请检查内容后手动按 Enter。')
  } catch (error) {
    await restoreAssistantWindow().catch(() => undefined)
    setNotice('error', '发送失败', errorMessage(error))
  } finally {
    isSending.value = false
    if (desktopRuntime) await gameOverlay.resumeCompactIfGame()
  }
}

async function submitOverlay(): Promise<void> {
  if (!canOverlaySubmit.value) return
  const sourceText = text.value
  isSending.value = true
  setNotice(
    'working',
    outgoingMode.value === 'translate' ? '正在中译英' : '正在发送中文',
    outgoingMode.value === 'translate' ? '翻译完成后将英文写入游戏聊天框。' : '正在恢复 HD2 并写入游戏聊天框。',
  )
  let sent = false
  try {
    const outgoingText = outgoingMode.value === 'translate' ? await translateOutgoingText(sourceText) : sourceText
    if (outgoingMode.value === 'translate') lastOutgoingTranslation.value = outgoingText
    const preview = await previewText(outgoingText)
    if (!preview.cleanedText.trim()) throw new Error('没有可发送的文字')
    if (preview.scalarCount > CHARACTER_LIMIT) throw new Error(`最终文本共 ${preview.scalarCount} 字符，超过 ${CHARACTER_LIMIT} 字符限制`)
    await gameOverlay.deactivateComposer()
    const result = await sendQuickShout(preview.cleanedText, undefined, false)
    if (!result.ok) throw new Error(result.message)
    history.add(sourceText)
    text.value = ''
    chatInputLikelyOpen.value = false
    sent = true
    setNotice(
      'success',
      outgoingMode.value === 'translate' ? '英文译文已提交' : '中文消息已提交',
      '请在游戏中确认文字显示和发送结果。',
    )
  } catch (error) {
    chatInputLikelyOpen.value = true
    setNotice('error', outgoingMode.value === 'translate' ? '中译英发送失败' : '中文发送失败', errorMessage(error))
  } finally {
    isSending.value = false
    if (sent) {
      await gameOverlay.dismissCompact()
    } else {
      await gameOverlay.resumeCompactIfGame()
      await gameOverlay.focusComposer()
    }
  }
}

async function cancelOverlayComposer(): Promise<void> {
  if (!gameOverlay.isCompact.value || anyBusy.value) return
  text.value = ''
  submitOnEnterRelease.value = null
  cancelOnEscapeRelease.value = false
  history.resetBrowsing()
  composition.reset()
  isSending.value = true
  let cancelled = false
  try {
    await gameOverlay.deactivateComposer()
    await cancelOverlayChat()
    chatInputLikelyOpen.value = false
    cancelled = true
    setNotice('idle', '已取消输入', '游戏聊天框已关闭。')
  } catch (error) {
    setNotice('error', '取消输入失败', errorMessage(error))
  } finally {
    isSending.value = false
    if (cancelled) {
      await gameOverlay.dismissCompact()
    } else {
      await gameOverlay.resumeCompactIfGame()
      await gameOverlay.focusComposer()
    }
  }
}

async function expandOverlayToFull(): Promise<void> {
  activeView.value = 'compose'
  await gameOverlay.expandFull(true)
  focusInput()
}

async function runQuickShout(shout: QuickShout, source: 'button' | 'hotkey'): Promise<void> {
  if (isQuickShouting.value || anyBusy.value) return
  // Prefer a captured generation for button clicks, but still allow last-target /
  // foreground fallback so one-key shout works after the assistant is reopened.
  const generation = source === 'button' ? activeGeneration.value ?? undefined : undefined

  isQuickShouting.value = true
  setNotice('working', '正在快捷喊话', `${shout.label}：${shout.message}`)
  try {
    if (source === 'button') {
      await yieldAssistantWindow()
      await new Promise((resolve) => window.setTimeout(resolve, 380))
    } else {
      // Give the game a beat after global hotkey release before injecting Enter.
      await new Promise((resolve) => window.setTimeout(resolve, 160))
    }
    // Capturing a target and fill-only submissions leave chat open. Sending an
    // extra Enter in that state would close chat before the text is injected.
    const result = await sendQuickShout(shout.message, generation, !chatInputLikelyOpen.value)
    if (!result.ok) throw new Error(result.message)
    chatInputLikelyOpen.value = false
    setNotice('success', '快捷喊话已发送', `${shout.label}：${shout.message}`)
  } catch (error) {
    chatInputLikelyOpen.value = false
    if (source === 'button') await restoreAssistantWindow().catch(() => undefined)
    setNotice('error', '快捷喊话失败', errorMessage(error))
  } finally {
    isQuickShouting.value = false
    if (source === 'button' && desktopRuntime) await gameOverlay.resumeCompactIfGame()
  }
}

function addQuickShout(): void {
  if (settingsDraft.quickShouts.length >= 12) {
    setNotice('error', '喊话数量已达上限', '最多保存 12 条快捷喊话。')
    return
  }
  settingsDraft.quickShouts.push({ label: '新喊话', message: 'message', hotkey: '' })
}

function removeQuickShout(index: number): void {
  if (quickHotkeyRecordingIndex.value === index) stopQuickHotkeyRecording()
  settingsDraft.quickShouts.splice(index, 1)
}

function startQuickHotkeyRecording(index: number): void {
  stopQuickHotkeyRecording()
  quickHotkeyRecordingIndex.value = index
  window.addEventListener('keydown', onQuickHotkeyKeydown, true)
}

function stopQuickHotkeyRecording(): void {
  quickHotkeyRecordingIndex.value = null
  window.removeEventListener('keydown', onQuickHotkeyKeydown, true)
}

function onQuickHotkeyKeydown(event: KeyboardEvent): void {
  event.preventDefault()
  event.stopPropagation()
  if (event.code === 'Escape') {
    stopQuickHotkeyRecording()
    return
  }
  const accelerator = acceleratorFromKeyboardEvent(event)
  if (!accelerator || quickHotkeyRecordingIndex.value === null) return
  const shout = settingsDraft.quickShouts[quickHotkeyRecordingIndex.value]
  if (!shout) {
    stopQuickHotkeyRecording()
    return
  }
  shout.hotkey = accelerator
  stopQuickHotkeyRecording()
}

function isSupportedOverlayChatKey(code: string): boolean {
  return (
    ['Enter', 'Space', 'Tab', 'Backquote', 'Minus', 'Equal', 'BracketLeft', 'BracketRight', 'Backslash', 'Semicolon', 'Quote', 'Comma', 'Period', 'Slash'].includes(code) ||
    /^Key[A-Z]$/.test(code) ||
    /^Digit[0-9]$/.test(code) ||
    /^F(?:[1-9]|1[0-2])$/.test(code)
  )
}

function overlayChatKeyLabel(code: string): string {
  if (code.startsWith('Key')) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  const labels: Record<string, string> = {
    Space: 'Space',
    Backquote: '`',
    Minus: '-',
    Equal: '=',
    BracketLeft: '[',
    BracketRight: ']',
    Backslash: '\\',
    Semicolon: ';',
    Quote: "'",
    Comma: ',',
    Period: '.',
    Slash: '/',
  }
  return labels[code] ?? code
}

function startOverlayChatKeyRecording(): void {
  stopOverlayChatKeyRecording()
  overlayChatKeyRecording.value = true
  window.addEventListener('keydown', onOverlayChatKeydown, true)
}

function stopOverlayChatKeyRecording(): void {
  overlayChatKeyRecording.value = false
  window.removeEventListener('keydown', onOverlayChatKeydown, true)
}

function onOverlayChatKeydown(event: KeyboardEvent): void {
  event.preventDefault()
  event.stopPropagation()
  if (event.code === 'Escape') {
    stopOverlayChatKeyRecording()
    return
  }
  if (!isSupportedOverlayChatKey(event.code) || event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) {
    setNotice('error', '聊天键无效', '请只按一个普通按键，不要带 Ctrl、Alt、Shift 或 Win。')
    return
  }
  settingsDraft.overlayChatKey = event.code
  stopOverlayChatKeyRecording()
  setNotice('idle', '聊天键待保存', `当前选择：${overlayChatKeyLabel(event.code)}`)
}

async function runChatTranslation(restoreWhenDone: boolean): Promise<void> {
  if (isTranslatingChat.value) return
  const generation = activeGeneration.value
  if (!generation) {
    if (restoreWhenDone) await restoreAssistantWindow().catch(() => undefined)
    activeView.value = 'translate'
    setNotice('error', '没有目标会话', '请先捕获 HD2 窗口并校准聊天区域。')
    return
  }
  isTranslatingChat.value = true
  try {
    const result = await translateChatCapture(generation)
    const hasNewLines = result.lines.length > 0
    if (hasNewLines) {
      translationHistory.value.unshift({ id: Date.now(), ...result, translatedAt: Date.now() })
      translationHistory.value = translationHistory.value.slice(0, 20)
    }
    activeView.value = 'translate'
    await restoreAssistantWindow({ focus: true })
    setNotice(
      'success',
      hasNewLines ? '新聊天翻译完成' : '没有发现新聊天',
      hasNewLines
        ? `离线 OCR：${result.messageOcrLanguage}；识别到 ${result.lines.length} 条新消息，旧聊天行未重复发送。`
        : `离线 OCR：${result.messageOcrLanguage}；当前聊天行均已处理，未调用翻译接口。`,
    )
  } catch (error) {
    if (restoreWhenDone) await restoreAssistantWindow().catch(() => undefined)
    activeView.value = 'translate'
    setNotice('error', '聊天翻译失败', errorMessage(error))
  } finally {
    isTranslatingChat.value = false
  }
}

async function startManualChatTranslation(): Promise<void> {
  if (!activeGeneration.value || anyBusy.value) return
  setNotice('working', '正在读取聊天区域', '助手将最小化并读取已校准区域。')
  await yieldAssistantWindow()
  await new Promise((resolve) => window.setTimeout(resolve, 350))
  await runChatTranslation(true)
}

async function startCalibration(): Promise<void> {
  const generation = activeGeneration.value
  if (!generation || anyBusy.value) {
    setNotice('error', '无法校准', '请先捕获 HD2 窗口。')
    return
  }
  isCalibrating.value = true
  try {
    setNotice('working', '正在截取游戏画面', '助手将最小化以获取校准预览。')
    await yieldAssistantWindow()
    await new Promise((resolve) => window.setTimeout(resolve, 350))
    calibrationPreview.value = await captureChatCalibrationPreview(generation)
    calibrationSelection.value = savedSettings.chatRegion ?? { x: 0.02, y: 0.55, width: 0.55, height: 0.4 }
    await restoreAssistantWindow({ focus: true })
    activeView.value = 'translate'
    setNotice('idle', '选择聊天区域', '在预览中拖拽矩形，然后保存区域。')
  } catch (error) {
    await restoreAssistantWindow().catch(() => undefined)
    setNotice('error', '校准截图失败', errorMessage(error))
  } finally {
    isCalibrating.value = false
  }
}

function previewPoint(event: PointerEvent): { x: number; y: number } | null {
  const surface = previewSurfaceRef.value
  if (!surface) return null
  const rect = surface.getBoundingClientRect()
  if (rect.width <= 0 || rect.height <= 0) return null
  return { x: Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)), y: Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height)) }
}

function onSelectionStart(event: PointerEvent): void {
  const point = previewPoint(event)
  if (!point) return
  selectionStart.value = point
  calibrationSelection.value = { x: point.x, y: point.y, width: 0.001, height: 0.001 }
  previewSurfaceRef.value?.setPointerCapture(event.pointerId)
}

function onSelectionMove(event: PointerEvent): void {
  const start = selectionStart.value
  const point = previewPoint(event)
  if (!start || !point) return
  calibrationSelection.value = { x: Math.min(start.x, point.x), y: Math.min(start.y, point.y), width: Math.max(0.001, Math.abs(point.x - start.x)), height: Math.max(0.001, Math.abs(point.y - start.y)) }
}

function onSelectionEnd(event: PointerEvent): void {
  onSelectionMove(event)
  selectionStart.value = null
}

async function saveCalibration(): Promise<void> {
  const selection = calibrationSelection.value
  if (!selection || selection.width < 0.01 || selection.height < 0.01) {
    setNotice('error', '区域太小', '请重新拖拽并覆盖完整的聊天消息区域。')
    return
  }
  settingsDraft.chatRegion = selection
  if (await saveSettings({ quiet: true })) {
    calibrationPreview.value = null
    setNotice('success', '聊天区域已保存', '现在可使用截图热键翻译游戏聊天。')
  }
}

function onInput(event: Event): void {
  text.value = (event.target as HTMLInputElement).value
  history.resetBrowsing()
}

function onKeydown(event: KeyboardEvent): void {
  if (restoreHotkey.isRecording.value || captureHotkey.isRecording.value || overlayChatKeyRecording.value) return
  if (anyBusy.value) { event.preventDefault(); return }
  if (composition.shouldBlockKeydown(event)) return
  if (event.key === 'Escape') {
    event.preventDefault()
    if (gameOverlay.isCompact.value) cancelOnEscapeRelease.value = true
    else void clearDraft()
    return
  }
  if (event.key === 'ArrowUp') { event.preventDefault(); text.value = history.browseOlder(text.value); return }
  if (event.key === 'ArrowDown') { event.preventDefault(); text.value = history.browseNewer(text.value); return }
  if (event.key === 'Enter' && !event.repeat) {
    event.preventDefault()
    submitOnEnterRelease.value = gameOverlay.isCompact.value ? 'send' : submitIntentFromKeydown(event)
  }
}

function onKeyup(event: KeyboardEvent): void {
  const blocked = composition.shouldBlockKeyup(event)
  if (event.key === 'Escape') {
    const shouldCancel = cancelOnEscapeRelease.value
    cancelOnEscapeRelease.value = false
    if (!blocked && shouldCancel) void cancelOverlayComposer()
    return
  }
  if (event.key !== 'Enter') return
  const intent = submitOnEnterRelease.value
  submitOnEnterRelease.value = null
  if (!blocked && intent) {
    if (gameOverlay.isCompact.value) void submitOverlay()
    else void submit(intent)
  }
}

function showView(view: AppView): void {
  activeView.value = view
  if (view === 'compose') focusInput()
}

onMounted(async () => {
  focusInput()
  try {
    const [settings, languages] = await Promise.all([getTranslationSettings(), listOcrLanguages()])
    applySettingsView(settings)
    ocrLanguages.value = languages
    if (desktopRuntime) {
      await restoreHotkey.ensureRegistered()
      await captureHotkey.applyAccelerator(settings.captureHotkey)
      await quickShoutHotkeys.sync(settings.quickShouts)
      await gameOverlay.start()
    }
  } catch (error) {
    setNotice('error', '初始化失败', errorMessage(error))
  }
  if (!desktopRuntime) await refreshTarget()
  else void runUpdateCheck(false)
})

onUnmounted(() => {
  stopQuickHotkeyRecording()
  stopOverlayChatKeyRecording()
  void gameOverlay.dispose()
})
</script>

<template>
  <main class="app-shell" :class="{ 'is-game-overlay': showGameOverlay }">
    <section v-if="showGameOverlay" class="game-overlay-shell" aria-label="游戏内中文输入栏">
      <button class="overlay-drag-handle" type="button" title="拖动输入栏" aria-label="拖动输入栏" @pointerdown="gameOverlay.startDragging">⋮</button>
      <div class="overlay-mode-segmented" aria-label="侧栏发言模式">
        <button type="button" :aria-pressed="outgoingMode === 'direct'" :disabled="anyBusy" @click="outgoingMode = 'direct'">直发</button>
        <button type="button" :aria-pressed="outgoingMode === 'translate'" :disabled="anyBusy" @click="outgoingMode = 'translate'">中译英</button>
      </div>
      <div class="overlay-input-frame" :class="{ 'is-composing': composition.isComposing.value, 'has-error': isOverLimit }">
        <input
          ref="overlayInputRef"
          :value="text"
          type="text"
          autocomplete="off"
          spellcheck="false"
          :readonly="anyBusy || !gameOverlay.isComposerFocused.value"
          :placeholder="outgoingMode === 'translate' ? '输入中文并翻译' : '输入中文消息'"
          @input="onInput"
          @keydown="onKeydown"
          @keyup="onKeyup"
          @compositionstart="composition.onCompositionStart"
          @compositionupdate="composition.onCompositionUpdate"
          @compositionend="composition.onCompositionEnd"
          @blur="composition.onBlur"
        />
        <span v-if="composition.isComposing.value" class="composition-badge">候选中</span>
        <span class="overlay-counter" :data-tone="counterTone">{{ characterCount }} / {{ CHARACTER_LIMIT }}</span>
      </div>
      <button class="overlay-icon-button" type="button" :title="outgoingMode === 'translate' ? '翻译并发送' : '发送'" :aria-label="outgoingMode === 'translate' ? '翻译并发送' : '发送'" :disabled="!canOverlaySubmit" @click="submitOverlay">↑</button>
      <button class="overlay-icon-button" type="button" title="取消" aria-label="取消" :disabled="anyBusy || !gameOverlay.isComposerFocused.value" @click="cancelOverlayComposer">×</button>
      <button class="overlay-icon-button" type="button" title="展开助手" aria-label="展开助手" :disabled="anyBusy" @click="expandOverlayToFull">□</button>
    </section>
    <section v-else class="workspace" aria-labelledby="app-title">
      <header class="app-header">
        <div class="brand-mark" aria-hidden="true"><span>H2</span></div>
        <div class="brand-copy"><p class="eyebrow">HELLDIVERS 2 / CHAT CONSOLE</p><h1 id="app-title">中文输入与聊天翻译</h1></div>
        <nav class="view-tabs" aria-label="工作视图">
          <button :class="{ active: activeView === 'compose' }" type="button" @click="showView('compose')">发言</button>
          <button :class="{ active: activeView === 'translate' }" type="button" @click="showView('translate')">聊天翻译</button>
          <button :class="{ active: activeView === 'settings' }" type="button" @click="showView('settings')">设置</button>
        </nav>
        <label class="overlay-mode-toggle" title="切换中文输入位置">
          <input type="checkbox" :checked="savedSettings.gameOverlayEnabled" :disabled="anyBusy" @change="onGameOverlayToggle" />
          <span class="overlay-mode-track" aria-hidden="true"><span></span></span>
          <span>中文侧栏</span>
        </label>
      </header>

      <div v-if="updateInfo?.updateAvailable" class="update-banner" role="status">
        <div><strong>发现新版本 v{{ updateInfo.latestVersion }}</strong><span>当前版本 v{{ updateInfo.currentVersion }}</span></div>
        <button class="primary-button compact" type="button" @click="openLatestRelease">下载最新版</button>
      </div>

      <div class="content-grid">
        <section class="composer-panel work-panel">
          <template v-if="activeView === 'compose'">
            <div class="section-heading composer-heading">
              <div><p class="eyebrow">MESSAGE BUFFER</p><h2>准备发言</h2></div>
              <div class="segmented-control" aria-label="发言模式">
                <button type="button" :aria-pressed="outgoingMode === 'direct'" @click="outgoingMode = 'direct'">中文直发</button>
                <button type="button" :aria-pressed="outgoingMode === 'translate'" @click="outgoingMode = 'translate'">中译英</button>
              </div>
            </div>
            <label class="input-label" for="message-input">消息内容</label>
            <div class="input-frame" :class="{ 'is-composing': composition.isComposing.value, 'has-error': isOverLimit }">
              <input id="message-input" ref="inputRef" :value="text" type="text" autocomplete="off" spellcheck="false" :readonly="anyBusy" placeholder="输入要发送到游戏聊天框的内容" @input="onInput" @keydown="onKeydown" @keyup="onKeyup" @compositionstart="composition.onCompositionStart" @compositionupdate="composition.onCompositionUpdate" @compositionend="composition.onCompositionEnd" @blur="composition.onBlur" />
              <span v-if="composition.isComposing.value" class="composition-badge">候选中</span>
              <span class="character-counter" :data-tone="counterTone">{{ characterCount }} / {{ CHARACTER_LIMIT }}</span>
            </div>
            <div class="input-help"><span><kbd>Enter</kbd> 发送</span><span><kbd>Ctrl</kbd> + <kbd>Enter</kbd> 仅填入</span><span><kbd>↑</kbd> <kbd>↓</kbd> 历史</span></div>
            <div v-if="lastOutgoingTranslation" class="translation-preview"><span>最近英文译文</span><p>{{ lastOutgoingTranslation }}</p></div>
            <section class="quick-shout-panel" aria-labelledby="quick-shout-heading">
              <div class="subsection-heading">
                <div><p class="eyebrow">QUICK COMMS</p><h3 id="quick-shout-heading">一键喊话</h3></div>
                <span>自动打开聊天并发送</span>
              </div>
              <div v-if="savedSettings.quickShouts.length" class="quick-shout-grid">
                <button
                  v-for="(shout, index) in savedSettings.quickShouts"
                  :key="`${shout.label}-${index}`"
                  class="quick-shout-button"
                  type="button"
                  :disabled="anyBusy"
                  :title="shout.message"
                  @click="runQuickShout(shout, 'button')"
                >
                  <span>{{ shout.label }}</span>
                  <kbd v-if="shout.hotkey">{{ formatHotkeyLabel(shout.hotkey) }}</kbd>
                </button>
              </div>
              <div v-else class="empty-inline">在设置中添加快捷喊话</div>
            </section>
            <div class="notice" :data-tone="noticeTone" role="status" aria-live="polite"><span class="notice-signal" aria-hidden="true"></span><div><strong>{{ noticeTitle }}</strong><p>{{ noticeMessage }}</p></div></div>
            <div class="send-actions"><button class="primary-button" type="button" :disabled="!canSubmit" @click="submit('send')">{{ isSending ? '处理中…' : outgoingMode === 'translate' ? '翻译并发送' : '发送到游戏' }}</button><button class="secondary-button" type="button" :disabled="!canSubmit" @click="submit('fill')">仅填入</button></div>
            <p v-if="isOverLimit" class="field-error" role="alert">已超出 {{ CHARACTER_LIMIT }} 字符限制。</p>
          </template>

          <template v-else-if="activeView === 'translate'">
            <div class="section-heading composer-heading"><div><p class="eyebrow">CHAT OCR</p><h2>游戏聊天翻译</h2></div><span class="hotkey-chip">{{ captureHotkey.hotkeyLabel.value }}</span></div>
            <div class="translation-toolbar"><button class="primary-button compact" type="button" :disabled="!activeGeneration || anyBusy || !savedSettings.chatRegion" @click="startManualChatTranslation">{{ isTranslatingChat ? '识别翻译中…' : '读取并翻译' }}</button><button class="secondary-button" type="button" :disabled="!activeGeneration || anyBusy" @click="startCalibration">校准区域</button></div>
            <div v-if="calibrationPreview" class="calibration-workspace">
              <div ref="previewSurfaceRef" class="preview-surface" @pointerdown="onSelectionStart" @pointermove="onSelectionMove" @pointerup="onSelectionEnd" @pointercancel="selectionStart = null"><img :src="calibrationPreview.dataUrl" alt="游戏客户区校准预览" draggable="false" /><span v-if="calibrationSelection" class="selection-box" :style="calibrationStyle"></span></div>
              <div class="calibration-actions"><button class="primary-button compact" type="button" @click="saveCalibration">保存区域</button><button class="ghost-button" type="button" @click="calibrationPreview = null">取消</button></div>
            </div>
            <div class="notice translation-notice" :data-tone="noticeTone" role="status" aria-live="polite"><span class="notice-signal" aria-hidden="true"></span><div><strong>{{ noticeTitle }}</strong><p>{{ noticeMessage }}</p></div></div>
            <div v-if="translationHistory.length" class="translation-history">
              <section v-for="item in translationHistory" :key="item.id" class="translation-capture">
                <header class="capture-meta"><span>{{ item.messageOcrLanguage }} · {{ item.lines.length }} 条</span><time>{{ new Date(item.translatedAt).toLocaleTimeString() }}</time></header>
                <article v-for="(line, lineIndex) in item.lines" :key="`${item.id}-${lineIndex}`" class="translation-item">
                  <div class="message-copy">
                    <strong v-if="line.speaker" class="message-speaker">{{ line.speaker }}</strong>
                    <p class="translated-text">{{ line.translatedMessage }}</p>
                    <p class="source-text"><span>原文</span>{{ line.originalMessage }}</p>
                  </div>
                </article>
              </section>
            </div>
            <div v-else class="empty-state">尚无聊天翻译记录</div>
          </template>

          <template v-else>
            <div class="section-heading composer-heading"><div><p class="eyebrow">TRANSLATION API</p><h2>翻译与热键设置</h2></div><span class="key-status">{{ savedSettings.apiKeyConfigured ? 'Key 已保存' : '未配置 Key' }}</span></div>
            <form class="settings-form" @submit.prevent="saveSettings()">
              <label>API 地址<input v-model="settingsDraft.apiUrl" type="url" placeholder="https://example.com/v1" /></label>
              <label>网络代理（可选）<input v-model="settingsDraft.proxyUrl" type="url" placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:7891" /></label>
              <label>模型名<input v-model="settingsDraft.model" type="text" placeholder="gpt-4.1-mini" /></label>
              <label>API Key<input v-model="settingsDraft.apiKey" type="password" :placeholder="savedSettings.apiKeyConfigured ? '留空以保留已保存 Key' : '可选，本地接口可留空'" /></label>
              <label>离线 OCR<select v-model="settingsDraft.ocrLanguage"><option value="auto">自动中英混合（推荐）</option><option v-for="language in ocrLanguages" :key="language.tag" :value="language.tag">回退：{{ language.nativeName }} · {{ language.tag }}</option></select></label>
              <label>游戏聊天英译中提示词<textarea v-model="settingsDraft.incomingPrompt" rows="8" placeholder="留空时使用内置《绝地潜兵2》玩家黑话提示词"></textarea></label>
              <label>输入消息中译英提示词<textarea v-model="settingsDraft.outgoingPrompt" rows="8" placeholder="留空时使用内置《绝地潜兵2》Gamer Slang 提示词"></textarea></label>
              <label class="overlay-toggle-setting"><input v-model="settingsDraft.gameOverlayEnabled" type="checkbox" />游戏前台时启用右侧中文输入栏</label>
              <div class="settings-row">
                <div><span class="field-label">游戏聊天键</span><strong>{{ overlayChatKeyLabel(settingsDraft.overlayChatKey) }}</strong></div>
                <button class="secondary-button" type="button" :disabled="anyBusy || !settingsDraft.gameOverlayEnabled" @click="overlayChatKeyRecording ? stopOverlayChatKeyRecording() : startOverlayChatKeyRecording()">{{ overlayChatKeyRecording ? '按下单个按键…' : '重新绑定' }}</button>
              </div>
              <label>游戏文字注入方式<select v-model="settingsDraft.gameInputMethod"><option value="gbkAltCode">GBK Alt 数字码（HD2 推荐）</option><option value="unicodeSendInput">旧版 Unicode SendInput（排障）</option></select></label>
              <label class="delay-setting">
                <span class="delay-setting-label">聊天框文字焦点等待 <output>{{ settingsDraft.quickShoutFocusDelayMs }} ms</output></span>
                <input v-model.number="settingsDraft.quickShoutFocusDelayMs" type="range" :min="MIN_QUICK_SHOUT_FOCUS_DELAY_MS" :max="MAX_QUICK_SHOUT_FOCUS_DELAY_MS" :step="QUICK_SHOUT_FOCUS_DELAY_STEP_MS" />
              </label>
              <section class="quick-shout-settings" aria-labelledby="quick-shout-settings-heading">
                <div class="subsection-heading">
                  <div><p class="eyebrow">QUICK COMMS</p><h3 id="quick-shout-settings-heading">快捷喊话预设</h3></div>
                  <button class="secondary-button" type="button" :disabled="anyBusy || settingsDraft.quickShouts.length >= 12" @click="addQuickShout">添加</button>
                </div>
                <div class="quick-shout-editor-list">
                  <div v-for="(shout, index) in settingsDraft.quickShouts" :key="index" class="quick-shout-editor">
                    <label>名称<input v-model="shout.label" maxlength="24" type="text" /></label>
                    <label>实际发送文本<input v-model="shout.message" maxlength="100" type="text" /></label>
                    <div class="quick-hotkey-field">
                      <span class="field-label">全局热键</span>
                      <strong>{{ shout.hotkey ? formatHotkeyLabel(shout.hotkey) : '未绑定' }}</strong>
                      <button class="secondary-button" type="button" :disabled="anyBusy" @click="quickHotkeyRecordingIndex === index ? stopQuickHotkeyRecording() : startQuickHotkeyRecording(index)">{{ quickHotkeyRecordingIndex === index ? '按组合键…' : '录制' }}</button>
                      <button class="ghost-button" type="button" :disabled="anyBusy || !shout.hotkey" @click="shout.hotkey = ''">清除</button>
                      <button class="ghost-button danger-action" type="button" :disabled="anyBusy" @click="removeQuickShout(index)">删除</button>
                    </div>
                  </div>
                </div>
              </section>
              <div class="settings-row"><div><span class="field-label">截图翻译热键</span><strong>{{ captureHotkey.hotkeyLabel.value }}</strong></div><button class="secondary-button" type="button" :disabled="anyBusy" @click="captureHotkey.isRecording.value ? captureHotkey.cancelRecording() : captureHotkey.startRecording()">{{ captureHotkey.isRecording.value ? '按下新组合键…' : '重新绑定' }}</button></div>
              <div class="settings-row"><div><span class="field-label">助手唤回热键</span><strong>{{ restoreHotkey.hotkeyLabel.value }}</strong></div><button class="secondary-button" type="button" :disabled="anyBusy" @click="restoreHotkey.isRecording.value ? restoreHotkey.cancelRecording() : restoreHotkey.startRecording()">{{ restoreHotkey.isRecording.value ? '按下新组合键…' : '重新绑定' }}</button></div>
              <p class="security-note">API Key 以明文 UTF-8 JSON 保存在当前 Windows 用户的应用配置目录。</p>
              <div class="settings-actions"><button class="primary-button compact" type="submit" :disabled="anyBusy">{{ isSavingSettings ? '保存中…' : '保存设置' }}</button><button class="secondary-button" type="button" :disabled="anyBusy" @click="testApi">{{ isTestingApi ? '测试中…' : '测试接口' }}</button><button class="secondary-button" type="button" :disabled="anyBusy || isCheckingUpdates" @click="runUpdateCheck(true)">{{ isCheckingUpdates ? '检查中…' : '检查更新' }}</button><button v-if="savedSettings.apiKeyConfigured" class="ghost-button" type="button" :disabled="anyBusy" @click="saveSettings({ clearKey: true })">清除 Key</button></div>
            </form>
            <div class="notice settings-notice" :data-tone="noticeTone" role="status" aria-live="polite"><span class="notice-signal" aria-hidden="true"></span><div><strong>{{ noticeTitle }}</strong><p>{{ noticeMessage }}</p></div></div>
          </template>
        </section>
        <TargetDiagnosticPanel :diagnostic="target" :loading="isCapturing || isRefreshing" :available="canCapture" @capture="captureTarget" />
      </div>
      <footer class="safety-note"><span class="safety-line" aria-hidden="true"></span><p>仅向已锁定且重新验证的前台 HD2 窗口注入；截图使用 Windows 本地 OCR，不上传游戏画面。</p></footer>
    </section>
  </main>
</template>

<style scoped>
.app-shell.is-game-overlay { display: block; min-width: 0; min-height: 100vh; overflow: hidden; padding: 0; background: #090b09; }
.game-overlay-shell { display: grid; width: 100vw; height: 100vh; grid-template-columns: 14px 88px minmax(0, 1fr) repeat(3, 38px); align-items: center; gap: 6px; padding: 6px; overflow: hidden; border: 1px solid #59614e; border-radius: 6px; background: #10130f; }
.overlay-drag-handle { width: 14px; height: 38px; padding: 0; border: 0; border-radius: 3px; background: var(--text-muted); color: #10130f; cursor: move; font-size: 15px; line-height: 1; }
.overlay-drag-handle:hover { background: var(--accent); }
.overlay-mode-segmented { display: grid; width: 88px; height: 38px; grid-template-columns: repeat(2, minmax(0, 1fr)); padding: 3px; border: 1px solid var(--line-strong); border-radius: 5px; background: #171a15; }
.overlay-mode-segmented button { min-width: 0; padding: 0; border: 0; border-radius: 3px; background: transparent; color: var(--text-muted); cursor: pointer; font-size: 10px; font-weight: 700; letter-spacing: 0; }
.overlay-mode-segmented button[aria-pressed='true'] { background: var(--surface-raised); color: var(--accent); }
.overlay-mode-segmented button:disabled { cursor: default; opacity: .45; }
.overlay-input-frame { position: relative; min-width: 0; height: 44px; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 5px; background: #171a15; }
.overlay-input-frame:focus-within { border-color: var(--accent); box-shadow: inset 0 0 0 1px rgba(230, 200, 76, .28); }
.overlay-input-frame.is-composing { border-color: var(--info); }
.overlay-input-frame.has-error { border-color: var(--danger); }
.overlay-input-frame input { width: 100%; height: 100%; min-width: 0; padding: 0 104px 0 12px; border: 0; outline: 0; background: transparent; color: var(--text); caret-color: var(--accent); font-size: 15px; }
.overlay-input-frame input[readonly] { color: var(--text-secondary); }
.overlay-input-frame .composition-badge { top: 11px; right: 55px; padding: 2px 5px; border-radius: 4px; font-size: 9px; }
.overlay-counter { position: absolute; top: 15px; right: 8px; color: var(--text-muted); font-family: ui-monospace, Consolas, monospace; font-size: 9px; font-variant-numeric: tabular-nums; }
.overlay-counter[data-tone='warning'] { color: var(--accent); }
.overlay-counter[data-tone='error'] { color: var(--danger); }
.overlay-icon-button { display: grid; width: 38px; height: 38px; place-items: center; padding: 0; border: 1px solid var(--line-strong); border-radius: 5px; background: #1b1f18; color: var(--text-secondary); cursor: pointer; font-size: 17px; line-height: 1; }
.overlay-icon-button:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }
.overlay-icon-button:disabled { opacity: .38; }
.workspace { width: min(100%, 1180px); }
.app-header { flex-wrap: wrap; }
.view-tabs { display: flex; gap: 4px; margin-left: auto; padding: 4px; border: 1px solid var(--line); border-radius: 8px; background: #121510; }
.view-tabs button, .segmented-control button { min-height: 34px; padding: 0 13px; border: 0; border-radius: 5px; background: transparent; color: var(--text-muted); cursor: pointer; font-weight: 700; font-size: 12px; }
.view-tabs button.active, .segmented-control button[aria-pressed='true'] { background: var(--surface-raised); color: var(--accent); }
.update-banner { display: flex; width: 100%; align-items: center; justify-content: space-between; gap: 16px; padding: 10px 20px; border-top: 1px solid rgba(230, 200, 76, .35); border-bottom: 1px solid rgba(230, 200, 76, .35); background: rgba(230, 200, 76, .08); }
.update-banner div { display: flex; min-width: 0; align-items: baseline; gap: 10px; }
.update-banner strong { color: var(--accent); font-size: 13px; }
.update-banner span { color: var(--text-muted); font-size: 11px; }
.update-banner .compact { min-width: 120px; min-height: 36px; }
.overlay-mode-toggle { display: flex; min-height: 42px; align-items: center; gap: 8px; color: var(--text-secondary); cursor: pointer; font-size: 12px; font-weight: 700; }
.overlay-mode-toggle input { position: absolute; width: 1px; height: 1px; opacity: 0; pointer-events: none; }
.overlay-mode-track { position: relative; width: 36px; height: 20px; flex: 0 0 36px; border: 1px solid var(--line-strong); border-radius: 10px; background: #171a15; transition: border-color 120ms ease, background 120ms ease; }
.overlay-mode-track span { position: absolute; top: 3px; left: 3px; width: 12px; height: 12px; border-radius: 50%; background: var(--text-muted); transition: transform 120ms ease, background 120ms ease; }
.overlay-mode-toggle input:checked + .overlay-mode-track { border-color: var(--accent); background: rgba(230, 200, 76, .16); }
.overlay-mode-toggle input:checked + .overlay-mode-track span { transform: translateX(16px); background: var(--accent); }
.overlay-mode-toggle:focus-within .overlay-mode-track { box-shadow: 0 0 0 3px var(--focus); }
.overlay-mode-toggle:has(input:disabled) { cursor: default; opacity: .45; }
.content-grid { grid-template-columns: minmax(0, 1.65fr) minmax(280px, .75fr); }
.work-panel { min-height: 590px; }
.segmented-control { display: flex; padding: 3px; border: 1px solid var(--line); border-radius: 7px; background: #121510; }
.send-actions, .translation-toolbar, .settings-actions, .calibration-actions { display: flex; gap: 10px; align-items: center; }
.send-actions .primary-button { flex: 1; }
.compact { width: auto; min-width: 150px; padding: 0 18px; }
.translation-preview { margin: 0 0 16px; padding: 12px 14px; border-left: 3px solid var(--info); background: #141913; }
.translation-preview span, .field-label { display: block; margin-bottom: 5px; color: var(--text-muted); font-size: 11px; }
.translation-preview p { margin: 0; color: var(--text-secondary); font-size: 13px; line-height: 1.5; }
.quick-shout-panel { margin: 0 0 16px; padding: 14px 0; border-top: 1px solid var(--line); border-bottom: 1px solid var(--line); }
.subsection-heading { display: flex; align-items: center; justify-content: space-between; gap: 14px; margin-bottom: 11px; }
.subsection-heading h3 { margin: 2px 0 0; color: var(--text); font-size: 15px; letter-spacing: 0; }
.subsection-heading > span { color: var(--text-muted); font-size: 10px; }
.quick-shout-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; }
.quick-shout-button { display: grid; grid-template-rows: 20px 17px; min-width: 0; min-height: 52px; align-content: center; justify-items: start; padding: 7px 10px; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 6px; background: var(--surface-soft); color: var(--text-secondary); cursor: pointer; text-align: left; }
.quick-shout-button:hover:not(:disabled) { border-color: var(--accent); color: var(--text); }
.quick-shout-button span { max-width: 100%; overflow: hidden; font-size: 12px; font-weight: 700; text-overflow: ellipsis; white-space: nowrap; }
.quick-shout-button kbd { max-width: 100%; overflow: hidden; color: var(--text-muted); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.quick-shout-button:disabled { cursor: not-allowed; opacity: .45; }
.empty-inline { padding: 8px 0; color: var(--text-muted); font-size: 12px; }
.hotkey-chip, .key-status { padding: 5px 9px; border: 1px solid var(--line-strong); border-radius: 6px; color: var(--text-secondary); font-family: ui-monospace, Consolas, monospace; font-size: 11px; }
.translation-toolbar { margin-bottom: 18px; }
.translation-notice, .settings-notice { margin-top: 18px; }
.translation-history { display: grid; gap: 10px; max-height: 460px; overflow-y: auto; scrollbar-gutter: stable; }
.translation-capture { overflow: hidden; border: 1px solid var(--line); border-radius: 7px; background: var(--surface-soft); }
.capture-meta { display: flex; justify-content: space-between; padding: 8px 14px; border-bottom: 1px solid var(--line); color: var(--text-muted); font-size: 10px; }
.translation-item { display: grid; grid-template-columns: minmax(0, 1fr); gap: 6px; align-items: start; padding: 12px 14px; border-bottom: 1px solid rgba(53, 59, 48, .7); }
.translation-item:last-child { border-bottom: 0; }
.translation-item p { white-space: pre-wrap; overflow-wrap: anywhere; word-break: break-word; }
.message-speaker { display: inline-block; min-width: 0; max-width: 100%; margin-bottom: 2px; color: var(--accent); font-size: 12px; font-weight: 700; line-height: 1.45; overflow-wrap: anywhere; word-break: break-word; }
.message-speaker.is-empty { min-height: 1px; }
.message-copy { min-width: 0; display: grid; gap: 4px; }
.translated-text { margin: 0; color: var(--text); font-size: 14px; font-weight: 650; line-height: 1.5; }
.source-text { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 7px; align-items: start; margin: 0; color: var(--text-muted); font-size: 11px; line-height: 1.45; }
.source-text span { color: #9a8253; font-size: 9px; font-weight: 700; line-height: 1.6; }
.empty-state { display: grid; min-height: 220px; place-items: center; border: 1px dashed var(--line); color: var(--text-muted); font-size: 13px; }
.calibration-workspace { margin-bottom: 18px; }
.preview-surface { position: relative; width: 100%; overflow: hidden; border: 1px solid var(--line-strong); background: #050605; cursor: crosshair; touch-action: none; user-select: none; }
.preview-surface img { display: block; width: 100%; height: auto; pointer-events: none; }
.selection-box { position: absolute; border: 2px solid var(--accent); background: rgba(230, 200, 76, .12); box-shadow: 0 0 0 9999px rgba(0, 0, 0, .48); pointer-events: none; }
.calibration-actions { margin-top: 10px; }
.settings-form { display: grid; gap: 15px; }
.settings-form label { display: grid; gap: 7px; color: var(--text-secondary); font-size: 12px; font-weight: 650; }
.settings-form input, .settings-form select, .settings-form textarea { width: 100%; min-height: 42px; padding: 0 12px; border: 1px solid var(--line-strong); border-radius: 6px; outline: none; background: #11140f; color: var(--text); }
.settings-form input[type='range'] { min-height: 24px; padding: 0; border: 0; background: transparent; accent-color: var(--accent); }
.settings-form .overlay-toggle-setting { grid-template-columns: 20px minmax(0, 1fr); align-items: center; }
.settings-form .overlay-toggle-setting input { width: 18px; height: 18px; min-height: 0; padding: 0; accent-color: var(--accent); }
.settings-form textarea { min-height: 150px; padding: 11px 12px; resize: vertical; font-family: ui-monospace, Consolas, monospace; font-size: 11px; line-height: 1.55; }
.settings-form input:focus, .settings-form select:focus, .settings-form textarea:focus { border-color: var(--accent); box-shadow: 0 0 0 3px var(--focus); }
.delay-setting-label { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.delay-setting-label output { color: var(--accent); font-family: ui-monospace, Consolas, monospace; font-size: 12px; }
.quick-shout-settings { padding-top: 4px; border-top: 1px solid var(--line); }
.quick-shout-editor-list { display: grid; gap: 9px; }
.quick-shout-editor { display: grid; grid-template-columns: minmax(100px, .55fr) minmax(180px, 1.45fr); gap: 10px; padding: 11px; border: 1px solid var(--line); border-radius: 7px; background: var(--surface-soft); }
.quick-hotkey-field { display: grid; grid-column: 1 / -1; grid-template-columns: minmax(72px, auto) minmax(120px, 1fr) auto auto auto; gap: 8px; align-items: center; }
.quick-hotkey-field .field-label { margin: 0; }
.quick-hotkey-field strong { min-width: 0; overflow: hidden; color: var(--text-secondary); font-family: ui-monospace, Consolas, monospace; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.quick-hotkey-field button { min-height: 34px; padding: 0 10px; }
.danger-action { color: var(--danger); }
.settings-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 0; border-top: 1px solid var(--line); }
.settings-row strong { color: var(--text-secondary); font-family: ui-monospace, Consolas, monospace; font-size: 12px; }
.security-note { margin: 0; color: var(--danger); font-size: 11px; line-height: 1.55; }
@media (max-width: 820px) { .content-grid { grid-template-columns: 1fr; } .work-panel { min-height: 0; } .quick-shout-grid { grid-template-columns: repeat(3, minmax(0, 1fr)); } }
@media (max-width: 600px) { .view-tabs { width: 100%; margin-left: 0; } .view-tabs button { flex: 1; } .update-banner, .update-banner div { align-items: stretch; flex-direction: column; } .update-banner .compact { width: 100%; } .section-heading, .subsection-heading { align-items: stretch; flex-direction: column; } .segmented-control button { flex: 1; } .send-actions, .translation-toolbar, .settings-actions { align-items: stretch; flex-direction: column; } .send-actions button, .translation-toolbar button, .settings-actions button { width: 100%; } .quick-shout-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } .quick-shout-editor { grid-template-columns: 1fr; } .quick-hotkey-field { grid-column: auto; grid-template-columns: 1fr 1fr; } .quick-hotkey-field strong { grid-column: 1 / -1; } .translation-item { grid-template-columns: 1fr; gap: 4px; } .message-speaker.is-empty { display: none; } }
</style>
