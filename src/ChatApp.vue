<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { openUrl } from '@tauri-apps/plugin-opener'

import TargetDiagnosticPanel from '@/components/TargetDiagnosticPanel.vue'
import { useCaptureHotkey } from '@/composables/useCaptureHotkey'
import { useChatOverlayWindow } from '@/composables/useChatOverlayWindow'
import { useCompositionLatch } from '@/composables/useCompositionLatch'
import type { GameForegroundEvent } from '@/composables/useGameOverlayWindow'
import { useInputHistory } from '@/composables/useInputHistory'
import { useRestoreHotkey } from '@/composables/useRestoreHotkey'
import { useQuickShoutHotkeys } from '@/composables/useQuickShoutHotkeys'
import { useStratagemHotkeys } from '@/composables/useStratagemHotkeys'
import { useTranslationHudWindow } from '@/composables/useTranslationHudWindow'
import {
  STRATAGEM_GROUP_LABELS,
  STRATAGEM_PRESETS,
  type StratagemPreset,
  type StratagemPresetGroup,
} from '@/data/stratagemPresets'
import {
  beginProbeSession,
  cancelSession,
  cancelOverlayChat,
  captureChatCalibrationPreview,
  checkForUpdates,
  exportDiagnosticLogs,
  getSessionState,
  getTargetDiagnostic,
  getTranslationSettings,
  handoffGameplayInput,
  injectProbeText,
  isTauriRuntime,
  listOcrLanguages,
  normalizeTarget,
  previewText,
  recordClientDiagnostic,
  resetInputState,
  saveTranslationSettings,
  sendQuickShout,
  sendStratagemMacro,
  testTranslationApi,
  translateChatCapture,
  translateOutgoingText,
} from '@/services/tauriApi'
import type {
  CalibrationPreview,
  ChatTranslationLine,
  NormalizedPosition,
  NormalizedRegion,
  OcrLanguage,
  QuickShout,
  StratagemDirectionInputMode,
  StratagemMacro,
  TargetDiagnostic,
  TranslationSettingsUpdate,
  TranslationSettingsView,
  UpdateCheckView,
} from '@/types/ipc'
import {
  SINGLE_INSTANCE_RESTORE_EVENT,
  type ChatOverlayAction,
  type ChatOverlayStatePayload,
} from '@/types/chatOverlay'
import {
  errorMessageWithAdvice,
  errorTextWithAdvice,
  getErrorActions,
  getErrorMessage,
  type ErrorRecoveryAction,
} from '@/utils/errorAdvice'
import { submitIntentFromKeydown, type SubmitIntent } from '@/utils/submit'
import { acceleratorFromKeyboardEvent, formatHotkeyLabel, stratagemAcceleratorFromKeyboardEvent } from '@/utils/hotkey'

const CHARACTER_LIMIT = 100
const DEFAULT_CAPTURE_HOTKEY = 'CommandOrControl+Shift+T'
const MIN_QUICK_SHOUT_FOCUS_DELAY_MS = 300
const MAX_QUICK_SHOUT_FOCUS_DELAY_MS = 1_200
const QUICK_SHOUT_FOCUS_DELAY_STEP_MS = 50
const DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS = 500
const MIN_GAME_INPUT_DELAY_MS = 10
const MAX_GAME_INPUT_DELAY_MS = 30
const GAME_INPUT_DELAY_STEP_MS = 5
const DEFAULT_GAME_INPUT_DELAY_MS = 15
const MIN_STRATAGEM_DELAY_MS = 10
const MAX_STRATAGEM_DELAY_MS = 250
const STRATAGEM_DELAY_STEP_MS = 5
const DEFAULT_STRATAGEM_MENU_OPEN_DELAY_MS = 100
const DEFAULT_STRATAGEM_PRESS_DELAY_MS = 50
const DEFAULT_STRATAGEM_INTERVAL_DELAY_MS = 35
const COMPACT_OVERLAY_HEIGHT = 58
const CHAT_CAPTURE_FOREGROUND_SETTLE_MS = 350
const FORCE_INPUT_RECOVERY_DELAY_MS = 8_000
const desktopRuntime = isTauriRuntime()
const previewParams = new URLSearchParams(window.location.search)
const updatePreview = import.meta.env.DEV && previewParams.has('update-preview')
const NOTICE_ACTION_LABELS: Record<ErrorRecoveryAction, string> = {
  recaptureTarget: '重新捕获',
  calibrateRegion: '重新校准',
  exportLogs: '导出日志',
  openSettings: '打开设置',
  openStratagem: '打开战备',
  testApi: '测试接口',
  focusComposer: '回到输入框',
  restoreInputState: '恢复输入状态',
}

type NoticeTone = 'idle' | 'working' | 'success' | 'error'
type AppView = 'compose' | 'translate' | 'stratagem' | 'settings'
type OutgoingMode = 'direct' | 'translate'
type GameChatInputState = 'open' | 'closed' | 'unknown'

interface TranslationHistoryItem {
  id: number
  lines: ChatTranslationLine[]
  messageOcrLanguage: string
  speakerOcrLanguage: string
  translatedAt: number
}

const inputRef = ref<HTMLInputElement | null>(null)
const activeView = ref<AppView>('compose')
const outgoingMode = ref<OutgoingMode>('direct')
const text = ref('')
const target = ref<TargetDiagnostic | null>(null)
const activeGeneration = ref<string | null>(null)
const gameChatInputState = ref<GameChatInputState>('closed')
const isRefreshing = ref(false)
const isCapturing = ref(false)
const isSending = ref(false)
const isTranslatingChat = ref(false)
const isSavingSettings = ref(false)
const isTestingApi = ref(false)
const isCheckingUpdates = ref(false)
const isExportingLogs = ref(false)
const isCalibrating = ref(false)
const isQuickShouting = ref(false)
const isStratagemRunning = ref(false)
const isRecoveringInput = ref(false)
const quickHotkeyRecordingIndex = ref<number | null>(null)
const stratagemHotkeyRecordingIndex = ref<number | null>(null)
const stratagemSearch = ref('')
const stratagemGroupFilter = ref<StratagemPresetGroup | 'all'>('all')
const overlayChatKeyRecording = ref(false)
const overlayArmed = ref(false)
const captureToken = ref(0)
const submitOnEnterRelease = ref<SubmitIntent | null>(null)
const noticeTone = ref<NoticeTone>('idle')
const noticeTitle = ref('等待目标')
const noticeMessage = ref('先捕获游戏窗口，再输入消息或配置聊天翻译。')
const noticeActions = ref<ErrorRecoveryAction[]>([])
const lastOutgoingTranslation = ref<string | null>(null)
const translationHistory = ref<TranslationHistoryItem[]>([])
const ocrLanguages = ref<OcrLanguage[]>([])
const calibrationPreview = ref<CalibrationPreview | null>(null)
const calibrationSelection = ref<NormalizedRegion | null>(null)
const selectionStart = ref<{ x: number; y: number } | null>(null)
const previewSurfaceRef = ref<HTMLDivElement | null>(null)
const latestGameForeground = ref<GameForegroundEvent | null>(null)
const clientOperationSeq = ref(0)
const unlisteners: UnlistenFn[] = []
const updateInfo = ref<UpdateCheckView | null>(
  updatePreview
    ? {
      currentVersion: '0.3.0',
      latestVersion: '0.5.2',
      updateAvailable: true,
      releaseUrl: 'https://github.com/fiatlux2333/Helldivers2-Chinese-Helper/releases/tag/v0.5.2',
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
  incomingTranslationDisplayMode: 'chatTranslationPage',
  translationHudPosition: null,
  gameOverlayEnabled: true,
  overlayChatKey: 'Enter',
  autoLockCaps: true,
  autoRestoreGameplayInput: true,
  gameInputMethod: 'unicodeSendInput',
  gameInputDelayMs: DEFAULT_GAME_INPUT_DELAY_MS,
  quickShoutFocusDelayMs: DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS,
  quickShouts: [],
  stratagemMacros: [],
  stratagemDirectionInputMode: 'wasd',
  stratagemAllowBareNumberHotkeys: false,
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
  incomingTranslationDisplayMode: 'chatTranslationPage' as TranslationSettingsUpdate['incomingTranslationDisplayMode'],
  translationHudPosition: null as NormalizedPosition | null,
  gameOverlayEnabled: true,
  overlayChatKey: 'Enter',
  autoLockCaps: true,
  autoRestoreGameplayInput: true,
  gameInputMethod: 'unicodeSendInput' as TranslationSettingsUpdate['gameInputMethod'],
  gameInputDelayMs: DEFAULT_GAME_INPUT_DELAY_MS,
  quickShoutFocusDelayMs: DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS,
  quickShouts: [] as QuickShout[],
  stratagemMacros: [] as StratagemMacro[],
  stratagemDirectionInputMode: 'wasd' as StratagemDirectionInputMode,
  stratagemAllowBareNumberHotkeys: false,
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
    isExportingLogs.value ||
    isCalibrating.value ||
    isQuickShouting.value ||
    isStratagemRunning.value ||
    isRecoveringInput.value,
)
const gameOverlayEnabled = computed(() => savedSettings.gameOverlayEnabled && overlayArmed.value)
const chatInputLikelyOpen = computed(() => gameChatInputState.value === 'open')
const wantsTypingOverlayTranslation = computed(
  () => savedSettings.incomingTranslationDisplayMode === 'typingOverlay',
)
const compactOverlayHeight = computed(() => COMPACT_OVERLAY_HEIGHT)
const translationHud = useTranslationHudWindow({
  enabled: computed(() => desktopRuntime && wantsTypingOverlayTranslation.value),
  onError: (message) => setErrorTextNotice('聊天译文 HUD 异常', message),
})
const gameOverlay = useChatOverlayWindow({
  enabled: gameOverlayEnabled,
  busy: anyBusy,
  compactHeight: compactOverlayHeight,
  getComposerState: (): ChatOverlayStatePayload => {
    const count = Array.from(text.value).length
    return {
      text: text.value,
      mode: outgoingMode.value,
      busy: anyBusy.value,
      canSubmit:
        desktopRuntime &&
        text.value.trim().length > 0 &&
        count <= CHARACTER_LIMIT &&
        !anyBusy.value &&
        target.value?.valid === true,
      characterCount: count,
      characterLimit: CHARACTER_LIMIT,
      counterTone: count > CHARACTER_LIMIT ? 'error' : count >= CHARACTER_LIMIT * 0.8 ? 'warning' : 'normal',
    }
  },
  onComposerInput: (value) => {
    text.value = value
    history.resetBrowsing()
  },
  onComposerModeChanged: (mode) => {
    outgoingMode.value = mode
  },
  onComposerAction: (action: ChatOverlayAction) => {
    if (action === 'submit') return submitOverlay()
    if (action === 'cancel') return cancelOverlayComposer()
    if (action === 'expand') return expandOverlayToFull()
    if (action === 'historyOlder') {
      text.value = history.browseOlder(text.value)
      return
    }
    text.value = history.browseNewer(text.value)
  },
  onForegroundChanged: (event) => {
    if (event.state === 'game') latestGameForeground.value = event
  },
  onGameForeground: async () => {
    try {
      const diagnostic = await getTargetDiagnostic()
      target.value = diagnostic
    } catch {
      target.value = null
    }
  },
  onComposerFocused: async () => {
    openGameChatInput('overlay_composer_focused')
  },
  onDiagnostic: (stage, message) => recordClientDiagnostic(stage, message),
  onCapsProtectionFailure: (message) => setErrorTextNotice('输入法保护已暂停', message),
  onError: (message) => setErrorTextNotice('悬浮输入栏异常', message),
})

const restoreHotkey = useRestoreHotkey({
  isSending: anyBusy,
  isCapturing,
  conflictsWith: () => [
    { accelerator: captureHotkeyValue.value, label: '聊天截图翻译' },
    ...quickShoutHotkeyConflicts(),
    ...stratagemHotkeyConflicts(),
  ],
  onRestored: () => restoreAssistantWindow({ focus: true }),
  onYielded: yieldAssistantWindow,
  onError: (title, message) => setErrorTextNotice(title, message),
  onHotkeyChanged: (label) => setNotice('success', '唤回热键已更新', `当前唤回热键：${label}`),
})

const captureHotkey = useCaptureHotkey(DEFAULT_CAPTURE_HOTKEY, {
  disabled: anyBusy,
  conflictsWith: () => [
    { accelerator: restoreHotkey.accelerator.value, label: '助手唤回' },
    ...quickShoutHotkeyConflicts(),
    ...stratagemHotkeyConflicts(),
  ],
  onTriggered: () => runChatTranslation(true),
  onChanged: persistCaptureHotkey,
  onError: (title, message) => setErrorTextNotice(title, message),
})

const quickShoutHotkeys = useQuickShoutHotkeys({
  disabled: computed(() => anyBusy.value || quickHotkeyRecordingIndex.value !== null || stratagemHotkeyRecordingIndex.value !== null),
  conflictsWith: () => [
    restoreHotkey.accelerator.value,
    captureHotkey.accelerator.value,
    ...settingsDraft.stratagemMacros.map((macroConfig) => macroConfig.hotkey.trim()),
  ],
  onTriggered: (shout) => runQuickShout(shout, 'hotkey'),
  onError: (title, message) => setErrorTextNotice(title, message),
})

const stratagemHotkeys = useStratagemHotkeys({
  disabled: computed(() => anyBusy.value || quickHotkeyRecordingIndex.value !== null || stratagemHotkeyRecordingIndex.value !== null),
  allowBareNumberKeys: computed(() => savedSettings.stratagemAllowBareNumberHotkeys),
  conflictsWith: () => [
    restoreHotkey.accelerator.value,
    captureHotkey.accelerator.value,
    ...settingsDraft.quickShouts.map((shout) => shout.hotkey.trim()),
  ],
  onTriggered: (macroConfig) => runStratagemMacro(macroConfig, 'hotkey'),
  onError: (title, message) => setErrorTextNotice(title, message),
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
watch([text, outgoingMode, anyBusy, target], () => {
  void gameOverlay.updateComposerState()
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
const stratagemGroupOptions = computed(() =>
  (['all', 'support', 'orbital', 'eagle', 'emplacement', 'sentry', 'backpack', 'vehicle', 'mission'] as Array<StratagemPresetGroup | 'all'>)
    .map((value) => ({ value, label: STRATAGEM_GROUP_LABELS[value] })),
)
const filteredStratagemPresets = computed(() => {
  const query = stratagemSearch.value.trim().toLowerCase()
  return STRATAGEM_PRESETS.filter((preset) => {
    if (stratagemGroupFilter.value !== 'all' && preset.group !== stratagemGroupFilter.value) return false
    if (!query) return true
    const haystack = [
      preset.zhName,
      preset.enName,
      STRATAGEM_GROUP_LABELS[preset.group],
      formatStratagemSequence(preset.sequence),
    ]
      .join(' ')
      .toLowerCase()
    return haystack.includes(query)
  })
})

function setNotice(tone: NoticeTone, title: string, message: string, actions: ErrorRecoveryAction[] = []): void {
  noticeTone.value = tone
  noticeTitle.value = title
  noticeMessage.value = message
  noticeActions.value = tone === 'error' ? actions : []
}

function errorMessage(error: unknown): string {
  return getErrorMessage(error)
}

function recordErrorNotice(title: string, message: string): void {
  const normalizedMessage = message.replace(/\s+/g, ' ').trim()
  void recordClientDiagnostic('ui_error_notice', `title=${title} message=${normalizedMessage}`).catch(() => undefined)
}

function nextClientOperationId(prefix: string): string {
  clientOperationSeq.value += 1
  return `${prefix}-${Date.now().toString(36)}-${clientOperationSeq.value}`
}

function recordStage(scope: string, opId: string, stage: string, detail = ''): void {
  const suffix = detail.trim() ? ` ${detail.trim().replace(/\s+/g, ' ')}` : ''
  const normalizedScope = scope.replace(/^client\./, '')
  void recordClientDiagnostic(normalizedScope, `op=${opId} stage=${stage}${suffix}`).catch(() => undefined)
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms))
}

function isInputRecoveryBusyError(error: unknown): boolean {
  if (!error || typeof error !== 'object') return false
  const candidate = error as { code?: unknown; message?: unknown }
  return (
    candidate.code === 'INVALID_SESSION' &&
    typeof candidate.message === 'string' &&
    /输入事务|正在发送|安全恢复/.test(candidate.message)
  )
}

function setErrorNotice(title: string, error: unknown): void {
  const context = { title }
  const message = errorMessageWithAdvice(error, context)
  const actions = getErrorActions(error, context)
  recordErrorNotice(title, message)
  setNotice('error', title, message, actions)
}

function setErrorTextNotice(title: string, message: string): void {
  const context = { title }
  const messageWithAdvice = errorTextWithAdvice(message, context)
  const actions = getErrorActions(message, context)
  recordErrorNotice(title, messageWithAdvice)
  setNotice('error', title, messageWithAdvice, actions)
}

function noticeActionLabel(action: ErrorRecoveryAction): string {
  return NOTICE_ACTION_LABELS[action]
}

function noticeActionDisabled(action: ErrorRecoveryAction): boolean {
  if (action === 'restoreInputState') return isRecoveringInput.value
  if (
    action === 'openSettings' ||
    action === 'openStratagem' ||
    action === 'focusComposer'
  ) {
    return false
  }
  return anyBusy.value
}

async function runNoticeAction(action: ErrorRecoveryAction): Promise<void> {
  if (noticeActionDisabled(action)) return
  try {
    if (action === 'recaptureTarget') {
      await captureTarget()
      return
    }
    if (action === 'calibrateRegion') {
      activeView.value = 'translate'
      await startCalibration()
      return
    }
    if (action === 'exportLogs') {
      await exportLogs()
      return
    }
    if (action === 'openSettings') {
      showView('settings')
      return
    }
    if (action === 'openStratagem') {
      showView('stratagem')
      return
    }
    if (action === 'testApi') {
      showView('settings')
      await testApi()
      return
    }
    if (action === 'restoreInputState') {
      await recoverInputState()
      return
    }
    showView('compose')
    await restoreAssistantWindow({ focus: true }).catch(() => undefined)
    focusInput()
  } catch (error) {
    setErrorNotice('处理建议失败', error)
  }
}

async function recoverInputState(): Promise<void> {
  if (isRecoveringInput.value) return
  isRecoveringInput.value = true
  const opId = nextClientOperationId('recovery')
  const draft = text.value
  recordStage('client.recovery', opId, 'start', `draft_chars=${Array.from(draft).length}`)
  try {
    let forced = false
    const snapshot = await resetInputState(false).catch(async (error) => {
      if (!isInputRecoveryBusyError(error)) throw error
      recordStage('client.recovery', opId, 'busy_wait', `delay_ms=${FORCE_INPUT_RECOVERY_DELAY_MS}`)
      setNotice(
        'working',
        '正在恢复输入状态',
        '检测到上一条发送还没结束，正在等待安全窗口；不会打断正在发送的事务，原文字会保留。',
      )
      await delay(FORCE_INPUT_RECOVERY_DELAY_MS)
      forced = true
      recordStage('client.recovery', opId, 'force_reset')
      return resetInputState(true)
    })
    recordStage(
      'client.recovery',
      opId,
      'core_reset',
      `generation=${snapshot.generation} phase=${snapshot.phase} target=${snapshot.target !== null} forced=${forced}`,
    )
    submitOnEnterRelease.value = null
    composition.reset()
    unknownGameChatInput('manual_recovery')
    if (snapshot.target) {
      activeGeneration.value = snapshot.generation
    } else {
      activeGeneration.value = null
      target.value = null
    }
    if (desktopRuntime) {
      await gameOverlay.dispose().catch((error) => {
        recordStage('client.recovery', opId, 'overlay_dispose_failed', errorMessage(error))
      })
      await gameOverlay.start()
      await gameOverlay.restoreWindow(true).catch((error) => {
        recordStage('client.recovery', opId, 'restore_window_failed', errorMessage(error))
      })
    }
    activeView.value = 'compose'
    text.value = draft
    await nextTick()
    focusInput()
    recordStage('client.recovery', opId, 'done')
    if (!snapshot.keysRecovered || !snapshot.capsRecovered) {
      setNotice(
        'error',
        '输入状态仍需处理',
        !snapshot.keysRecovered && !snapshot.capsRecovered
          ? '按键释放和 CapsLock 输入法保护都没有通过自检。请先松开功能键、按一次 Esc，再手动调整 CapsLock；不要直接重试发送。'
          : !snapshot.keysRecovered
            ? '部分功能键没有通过释放自检。请先松开功能键、按一次 Esc，再点击恢复输入状态；不要直接重试发送。'
            : '已释放可能残留的按键，但 CapsLock 输入法保护没有通过自检。请手动调整 CapsLock，或在设置中关闭后重新启用输入法保护。',
        ['openSettings', 'exportLogs'],
      )
      return
    }
    setNotice(
      'success',
      '输入状态已恢复',
      snapshot.target
        ? forced
          ? '已强制释放可能残留的按键并恢复输入框，原文字已保留；请确认游戏聊天框状态后再发送。'
          : '已释放可能残留的按键并恢复输入框，原文字已保留。'
        : '已释放可能残留的按键，原文字已保留；请重新捕获 HD2 后再发送。',
    )
  } catch (error) {
    recordStage('client.recovery', opId, 'failed', errorMessage(error))
    throw error
  } finally {
    isRecoveringInput.value = false
  }
}

function setGameChatInputState(state: GameChatInputState, reason: string): void {
  if (gameChatInputState.value === state) return
  gameChatInputState.value = state
  void recordClientDiagnostic(
    'game_chat_input_state',
    `state=${state} reason=${reason}`,
  ).catch(() => undefined)
}

function openGameChatInput(reason: string): void {
  setGameChatInputState('open', reason)
}

function closeGameChatInput(reason: string): void {
  setGameChatInputState('closed', reason)
}

function unknownGameChatInput(reason: string): void {
  setGameChatInputState('unknown', reason)
}

function toggleGameChatInput(reason: string): void {
  setGameChatInputState(gameChatInputState.value === 'open' ? 'closed' : 'open', reason)
}

async function reconcileSessionAfterInjectionFailure(generation: string): Promise<boolean> {
  try {
    const snapshot = await getSessionState()
    const sameGeneration = snapshot.generation === generation
    const retryable = sameGeneration && snapshot.phase === 'editing' && snapshot.target !== null
    activeGeneration.value = retryable ? generation : null
    await recordClientDiagnostic(
      'session_reconcile',
      `generation_match=${sameGeneration} phase=${snapshot.phase} target=${snapshot.target !== null} retryable=${retryable}`,
    ).catch(() => undefined)
    return retryable
  } catch (error) {
    await recordClientDiagnostic(
      'session_reconcile',
      `stage=failed error=${errorMessage(error)}`,
    ).catch(() => undefined)
    return false
  }
}

function focusInput(): void {
  if (activeView.value === 'compose') {
    void nextTick(() => inputRef.value?.focus())
  }
}

function quickShoutHotkeyConflicts(): Array<{ accelerator: string; label: string }> {
  return settingsDraft.quickShouts
    .map((shout, index) => ({
      accelerator: shout.hotkey.trim(),
      label: `快捷喊话 ${shout.label.trim() || `#${index + 1}`}`,
    }))
    .filter((item) => item.accelerator.length > 0)
}

function stratagemHotkeyConflicts(): Array<{ accelerator: string; label: string }> {
  return settingsDraft.stratagemMacros
    .map((macroConfig, index) => ({
      accelerator: macroConfig.hotkey.trim(),
      label: `战备 ${macroConfig.label.trim() || `#${index + 1}`}`,
    }))
    .filter((item) => item.accelerator.length > 0)
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
  const normalizedView = {
    ...view,
    stratagemMacros: view.stratagemMacros.map(normalizeStratagemMacro),
  }
  Object.assign(savedSettings, normalizedView)
  settingsDraft.apiUrl = view.apiUrl
  settingsDraft.proxyUrl = view.proxyUrl
  settingsDraft.apiKey = ''
  settingsDraft.model = view.model
  settingsDraft.ocrLanguage = view.ocrLanguage
  settingsDraft.chatRegion = view.chatRegion
  settingsDraft.incomingPrompt = view.incomingPrompt
  settingsDraft.outgoingPrompt = view.outgoingPrompt
  settingsDraft.incomingTranslationDisplayMode = view.incomingTranslationDisplayMode
  settingsDraft.translationHudPosition = view.translationHudPosition ? { ...view.translationHudPosition } : null
  settingsDraft.gameOverlayEnabled = view.gameOverlayEnabled
  settingsDraft.overlayChatKey = view.overlayChatKey
  settingsDraft.autoLockCaps = view.autoLockCaps
  settingsDraft.autoRestoreGameplayInput = view.autoRestoreGameplayInput
  settingsDraft.gameInputMethod = view.gameInputMethod
  settingsDraft.gameInputDelayMs = view.gameInputDelayMs
  settingsDraft.quickShoutFocusDelayMs = view.quickShoutFocusDelayMs
  settingsDraft.quickShouts = view.quickShouts.map((shout) => ({ ...shout }))
  settingsDraft.stratagemMacros = normalizedView.stratagemMacros.map((macroConfig) => ({ ...macroConfig, sequence: [...macroConfig.sequence] }))
  settingsDraft.stratagemDirectionInputMode = view.stratagemDirectionInputMode
  settingsDraft.stratagemAllowBareNumberHotkeys = view.stratagemAllowBareNumberHotkeys
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
    incomingTranslationDisplayMode: settingsDraft.incomingTranslationDisplayMode,
    translationHudPosition: settingsDraft.translationHudPosition ? { ...settingsDraft.translationHudPosition } : null,
    gameOverlayEnabled: settingsDraft.gameOverlayEnabled,
    overlayChatKey: settingsDraft.overlayChatKey,
    autoLockCaps: settingsDraft.autoLockCaps,
    autoRestoreGameplayInput: settingsDraft.autoRestoreGameplayInput,
    gameInputMethod: settingsDraft.gameInputMethod,
    gameInputDelayMs: settingsDraft.gameInputDelayMs,
    quickShoutFocusDelayMs: settingsDraft.quickShoutFocusDelayMs,
    quickShouts: settingsDraft.quickShouts.map((shout) => ({ ...shout })),
    stratagemMacros: settingsDraft.stratagemMacros.map(normalizeStratagemMacro),
    stratagemDirectionInputMode: settingsDraft.stratagemDirectionInputMode,
    stratagemAllowBareNumberHotkeys: settingsDraft.stratagemAllowBareNumberHotkeys,
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
    if (desktopRuntime) {
      await quickShoutHotkeys.sync(view.quickShouts)
      await stratagemHotkeys.sync(view.stratagemMacros)
    }
    if (desktopRuntime && gameOverlay.isCompact.value) await gameOverlay.resumeCompactIfGame()
    if (!options?.quiet) setNotice('success', '设置已保存', '输入栏、注入方式、翻译、OCR 与战备配置已更新。')
    return true
  } catch (error) {
    setErrorNotice('保存设置失败', error)
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
      incomingTranslationDisplayMode: savedSettings.incomingTranslationDisplayMode,
      translationHudPosition: savedSettings.translationHudPosition,
      gameOverlayEnabled: enabled,
      overlayChatKey: savedSettings.overlayChatKey,
      autoLockCaps: savedSettings.autoLockCaps,
      autoRestoreGameplayInput: savedSettings.autoRestoreGameplayInput,
      gameInputMethod: savedSettings.gameInputMethod,
      gameInputDelayMs: savedSettings.gameInputDelayMs,
      quickShoutFocusDelayMs: savedSettings.quickShoutFocusDelayMs,
      quickShouts: savedSettings.quickShouts.map((shout) => ({ ...shout })),
      stratagemMacros: savedSettings.stratagemMacros.map((macroConfig) => ({ ...macroConfig, sequence: [...macroConfig.sequence] })),
      stratagemDirectionInputMode: savedSettings.stratagemDirectionInputMode,
      stratagemAllowBareNumberHotkeys: savedSettings.stratagemAllowBareNumberHotkeys,
    })
    savedSettings.gameOverlayEnabled = view.gameOverlayEnabled
    settingsDraft.gameOverlayEnabled = view.gameOverlayEnabled
    setNotice(
      'success',
      view.gameOverlayEnabled ? '中文侧栏已开启' : '中文侧栏已关闭',
      view.gameOverlayEnabled ? '游戏聊天键将唤出右侧输入栏。' : '后续消息在助手主界面输入。',
    )
  } catch (error) {
    setErrorNotice('切换输入模式失败', error)
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
    incomingTranslationDisplayMode: savedSettings.incomingTranslationDisplayMode,
    translationHudPosition: savedSettings.translationHudPosition,
    gameOverlayEnabled: savedSettings.gameOverlayEnabled,
    overlayChatKey: savedSettings.overlayChatKey,
    autoLockCaps: savedSettings.autoLockCaps,
    autoRestoreGameplayInput: savedSettings.autoRestoreGameplayInput,
    gameInputMethod: savedSettings.gameInputMethod,
    gameInputDelayMs: savedSettings.gameInputDelayMs,
    quickShoutFocusDelayMs: savedSettings.quickShoutFocusDelayMs,
    quickShouts: savedSettings.quickShouts.map((shout) => ({ ...shout })),
    stratagemMacros: savedSettings.stratagemMacros.map((macroConfig) => ({ ...macroConfig, sequence: [...macroConfig.sequence] })),
    stratagemDirectionInputMode: savedSettings.stratagemDirectionInputMode,
    stratagemAllowBareNumberHotkeys: savedSettings.stratagemAllowBareNumberHotkeys,
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
    setErrorNotice('接口测试失败', error)
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
    if (manual) setErrorNotice('检查更新失败', error)
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
    setErrorNotice('无法打开下载页', error)
  }
}

async function exportLogs(): Promise<void> {
  if (!desktopRuntime || isExportingLogs.value) return
  isExportingLogs.value = true
  try {
    const path = await exportDiagnosticLogs()
    setNotice('success', '诊断日志已导出', `文件已保存到：${path}`)
  } catch (error) {
    setErrorNotice('导出诊断日志失败', error)
  } finally {
    isExportingLogs.value = false
  }
}

async function refreshTarget(): Promise<TargetDiagnostic> {
  isRefreshing.value = true
  try {
    const diagnostic = await getTargetDiagnostic()
    target.value = diagnostic
    if (!diagnostic.valid) setErrorTextNotice('目标不可用', diagnostic.message)
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
    setNotice('working', '等待游戏窗口', '助手即将最小化，请切回 HD2；不用先打开游戏聊天框。')
    await new Promise((resolve) => window.setTimeout(resolve, 700))
    await yieldAssistantWindow()
    await new Promise((resolve) => window.setTimeout(resolve, 2_000))
    if (captureToken.value !== token) return
    const session = await beginProbeSession()
    activeGeneration.value = session.generation
    target.value = normalizeTarget(session.diagnostic, session.integrity)
    let overlayCleanupError: unknown = null
    const shouldCloseObservedGameChat = savedSettings.gameOverlayEnabled && chatInputLikelyOpen.value
    if (shouldCloseObservedGameChat) {
      overlayArmed.value = true
      try {
        await cancelOverlayChat()
        closeGameChatInput('capture_cancel_overlay_chat_success')
      } catch (error) {
        openGameChatInput('capture_cancel_overlay_chat_failed')
        overlayCleanupError = error
      }
    } else {
      overlayArmed.value = savedSettings.gameOverlayEnabled
    }
    await restoreAssistantWindow()
    if (overlayCleanupError) {
      setNotice(
        'success',
        '目标已锁定',
        `未能自动关闭游戏聊天框，请回到游戏手动按 Esc 后再开始输入。${errorMessage(overlayCleanupError)}`,
      )
    } else {
      setNotice('success', '目标已锁定', '在发言页输入后按 Enter 会自动打开游戏聊天框并发送；Ctrl+Enter 只填入。')
    }
  } catch (error) {
    activeGeneration.value = null
    closeGameChatInput('capture_failed')
    target.value = null
    overlayArmed.value = false
    await restoreAssistantWindow().catch(() => undefined)
    setErrorNotice('目标捕获失败', error)
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
  activeGeneration.value = null
  closeGameChatInput('clear_draft')
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
  const opId = nextClientOperationId('submit')
  recordStage(
    'client.submit',
    opId,
    'start',
    `intent=${intent} mode=${outgoingMode.value} generation=${generation} source_chars=${Array.from(sourceText).length}`,
  )
  isSending.value = true
  setNotice('working', outgoingMode.value === 'translate' ? '正在翻译并准备发送' : '正在准备发送', intent === 'send' ? '目标验证通过后将填入文字并发送最终 Enter。' : '本次仅填入文字。')
  try {
    const outgoingText = outgoingMode.value === 'translate' ? await translateOutgoingText(sourceText) : sourceText
    if (outgoingMode.value === 'translate') recordStage('client.submit', opId, 'translated', `chars=${Array.from(outgoingText).length}`)
    if (outgoingMode.value === 'translate') lastOutgoingTranslation.value = outgoingText
    const preview = await previewText(outgoingText)
    if (!preview.cleanedText.trim()) throw new Error('没有可发送的文字')
    if (preview.scalarCount > CHARACTER_LIMIT) throw new Error(`最终文本共 ${preview.scalarCount} 字符，超过 ${CHARACTER_LIMIT} 字符限制`)
    recordStage('client.submit', opId, 'preview', `chars=${preview.scalarCount} chat_input_likely_open=${chatInputLikelyOpen.value}`)
    recordStage('client.submit', opId, 'yield_assistant')
    await yieldAssistantWindow()
    await new Promise((resolve) => window.setTimeout(resolve, 180))
    recordStage('client.submit', opId, 'inject_start', `chat_preparation=${chatInputLikelyOpen.value ? 'keepOpen' : 'open'}`)
    const result = await injectProbeText(
      generation,
      preview.cleanedText,
      intent === 'send',
      chatInputLikelyOpen.value ? 'keepOpen' : 'open',
    )
    if (!result.ok) {
      recordStage('client.submit', opId, 'inject_failed', `code=${result.error?.code ?? 'unknown'}`)
      const retryableBeforeInjection = await reconcileSessionAfterInjectionFailure(generation)
      if (!retryableBeforeInjection) {
        activeGeneration.value = null
        unknownGameChatInput('compose_submit_failure_not_retryable')
      }
      if (
        result.error?.code === 'WINDOW_UNAVAILABLE' ||
        result.error?.code === 'WINDOW_NOT_VISIBLE' ||
        result.error?.code === 'WINDOW_MINIMIZED' ||
        result.error?.code === 'WINDOW_CLOAKED'
      ) {
        target.value = null
      }
      await restoreAssistantWindow().catch(() => undefined)
      throw result.error ?? new Error(result.message)
    }
    recordStage('client.submit', opId, 'inject_done')
    history.add(sourceText)
    text.value = ''
    if (intent === 'fill') openGameChatInput('compose_fill_success')
    else closeGameChatInput('compose_send_success')
    const handoffOk = intent === 'send'
      ? await bestEffortGameplayHandoff('client.submit', opId)
      : true
    if (handoffOk) {
      setNotice(
        'success',
        intent === 'send' ? '消息已提交' : '文字已填入',
        intent === 'send'
          ? '请在游戏中确认发送结果；下一条可直接回助手输入并按 Enter。'
          : '游戏保持前台，请检查内容后手动按 Enter。',
      )
    }
  } catch (error) {
    recordStage('client.submit', opId, 'failed', errorMessage(error))
    await restoreAssistantWindow().catch(() => undefined)
    setErrorNotice('发送失败', error)
  } finally {
    isSending.value = false
    if (desktopRuntime) await gameOverlay.resumeCompactIfGame()
    recordStage('client.submit', opId, 'finish')
  }
}

function observePhysicalGameChatKey(): void {
  toggleGameChatInput('physical_chat_key')
}

function observePhysicalGameEscapeKey(): void {
  closeGameChatInput('physical_escape_key')
}

async function submitOverlay(): Promise<void> {
  if (!canOverlaySubmit.value) return
  const sourceText = text.value
  const opId = nextClientOperationId('overlay-submit')
  recordStage(
    'client.overlay_submit',
    opId,
    'start',
    `mode=${outgoingMode.value} source_chars=${Array.from(sourceText).length}`,
  )
  isSending.value = true
  setNotice(
    'working',
    outgoingMode.value === 'translate' ? '正在中译英' : '正在发送中文',
    outgoingMode.value === 'translate' ? '翻译完成后将英文写入游戏聊天框。' : '正在恢复 HD2 并写入游戏聊天框。',
  )
  let sent = false
  try {
    const outgoingText = outgoingMode.value === 'translate' ? await translateOutgoingText(sourceText) : sourceText
    if (outgoingMode.value === 'translate') recordStage('client.overlay_submit', opId, 'translated', `chars=${Array.from(outgoingText).length}`)
    if (outgoingMode.value === 'translate') lastOutgoingTranslation.value = outgoingText
    const preview = await previewText(outgoingText)
    if (!preview.cleanedText.trim()) throw new Error('没有可发送的文字')
    if (preview.scalarCount > CHARACTER_LIMIT) throw new Error(`最终文本共 ${preview.scalarCount} 字符，超过 ${CHARACTER_LIMIT} 字符限制`)
    recordStage('client.overlay_submit', opId, 'preview', `chars=${preview.scalarCount}`)
    recordStage('client.overlay_submit', opId, 'dismiss_overlay')
    await gameOverlay.dismissCompact()
    recordStage('client.overlay_submit', opId, 'inject_start', 'chat_preparation=keepOpen')
    const result = await sendQuickShout(preview.cleanedText, undefined, 'keepOpen')
    if (!result.ok) {
      recordStage('client.overlay_submit', opId, 'inject_failed', `code=${result.error?.code ?? 'unknown'}`)
      throw result.error ?? new Error(result.message)
    }
    recordStage('client.overlay_submit', opId, 'inject_done')
    history.add(sourceText)
    text.value = ''
    closeGameChatInput('overlay_submit_success')
    const handoffOk = await bestEffortGameplayHandoff('client.overlay_submit', opId)
    sent = true
    if (handoffOk) {
      setNotice(
        'success',
        outgoingMode.value === 'translate' ? '英文译文已发出' : '中文消息已发出',
        '输入事件已发往游戏，请确认聊天框中的文字和发送结果。',
      )
    }
  } catch (error) {
    recordStage('client.overlay_submit', opId, 'failed', errorMessage(error))
    unknownGameChatInput('overlay_submit_failure')
    setErrorNotice(outgoingMode.value === 'translate' ? '中译英发送失败' : '中文发送失败', error)
  } finally {
    isSending.value = false
    if (!sent) {
      await gameOverlay.resumeCompactIfGame()
      await gameOverlay.focusComposer()
    }
    recordStage('client.overlay_submit', opId, 'finish', `sent=${sent}`)
  }
}

async function cancelOverlayComposer(): Promise<void> {
  if (!gameOverlay.isCompact.value || anyBusy.value) return
  const opId = nextClientOperationId('overlay-cancel')
  recordStage('client.overlay_cancel', opId, 'start')
  text.value = ''
  submitOnEnterRelease.value = null
  history.resetBrowsing()
  composition.reset()
  isSending.value = true
  let cancelled = false
  try {
    recordStage('client.overlay_cancel', opId, 'dismiss_overlay')
    await gameOverlay.dismissCompact()
    recordStage('client.overlay_cancel', opId, 'send_escape')
    await cancelOverlayChat()
    const handoffOk = await bestEffortGameplayHandoff('client.overlay_cancel', opId)
    closeGameChatInput('overlay_cancel_success')
    cancelled = true
    if (handoffOk) setNotice('idle', '已取消输入', '游戏聊天框已关闭。')
  } catch (error) {
    recordStage('client.overlay_cancel', opId, 'failed', errorMessage(error))
    setErrorNotice('取消输入失败', error)
  } finally {
    isSending.value = false
    if (!cancelled) {
      await gameOverlay.resumeCompactIfGame()
      await gameOverlay.focusComposer()
    }
    recordStage('client.overlay_cancel', opId, 'finish', `cancelled=${cancelled}`)
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
  const compactComposerWasFocused = source === 'hotkey' && gameOverlay.isComposerFocused.value
  const opId = nextClientOperationId('quick-shout')
  recordStage(
    'client.quick_shout',
    opId,
    'start',
    `source=${source} generation=${generation ?? 'none'} compact_focused=${compactComposerWasFocused}`,
  )
  let succeeded = false

  isQuickShouting.value = true
  setNotice('working', '正在快捷喊话', `${shout.label}：${shout.message}`)
  try {
    if (source === 'button') {
      recordStage('client.quick_shout', opId, 'yield_assistant')
      await yieldAssistantWindow()
      await new Promise((resolve) => window.setTimeout(resolve, 380))
    } else if (compactComposerWasFocused) {
      recordStage('client.quick_shout', opId, 'dismiss_overlay')
      await gameOverlay.dismissCompact()
    } else {
      // Give the game a beat after global hotkey release before injecting Enter.
      await new Promise((resolve) => window.setTimeout(resolve, 160))
    }
    // Capturing a target and fill-only submissions leave chat open. Sending an
    // extra Enter in that state would close chat before the text is injected.
    recordStage('client.quick_shout', opId, 'inject_start', `chat_preparation=${chatInputLikelyOpen.value ? 'keepOpen' : 'open'}`)
    const result = await sendQuickShout(
      shout.message,
      generation,
      chatInputLikelyOpen.value ? 'keepOpen' : 'open',
    )
    if (!result.ok) {
      recordStage('client.quick_shout', opId, 'inject_failed', `code=${result.error?.code ?? 'unknown'}`)
      throw result.error ?? new Error(result.message)
    }
    recordStage('client.quick_shout', opId, 'inject_done')
    closeGameChatInput('quick_shout_success')
    const handoffOk = await bestEffortGameplayHandoff('client.quick_shout', opId)
    succeeded = true
    if (handoffOk) setNotice('success', '快捷喊话已发出', `${shout.label}：${shout.message}，请在游戏中确认。`)
  } catch (error) {
    recordStage('client.quick_shout', opId, 'failed', errorMessage(error))
    unknownGameChatInput('quick_shout_failure')
    if (source === 'button') await restoreAssistantWindow().catch(() => undefined)
    setErrorNotice('快捷喊话失败', error)
  } finally {
    isQuickShouting.value = false
    if (source === 'button' && desktopRuntime) await gameOverlay.resumeCompactIfGame()
    if (!succeeded && compactComposerWasFocused) {
      await gameOverlay.resumeCompactIfGame()
      await gameOverlay.focusComposer()
    }
    recordStage('client.quick_shout', opId, 'finish', `succeeded=${succeeded}`)
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

async function runStratagemMacro(
  macroConfig: StratagemMacro,
  source: 'button' | 'hotkey',
  directionInputMode: StratagemDirectionInputMode = savedSettings.stratagemDirectionInputMode,
): Promise<void> {
  if (isStratagemRunning.value || anyBusy.value) return
  const normalizedMacro = normalizeStratagemMacro(macroConfig)
  const generation = source === 'button' ? activeGeneration.value ?? undefined : undefined
  const compactComposerWasFocused = source === 'hotkey' && gameOverlay.isComposerFocused.value
  const directionModeLabel = stratagemDirectionInputModeLabel(directionInputMode)
  const opId = nextClientOperationId('stratagem')
  recordStage(
    'client.stratagem',
    opId,
    'start',
    `source=${source} generation=${generation ?? 'none'} compact_focused=${compactComposerWasFocused} direction_mode=${directionInputMode}`,
  )
  let succeeded = false

  isStratagemRunning.value = true
  setNotice('working', '正在触发战备', `${normalizedMacro.label}：${formatStratagemSequence(normalizedMacro.sequence)} · ${directionModeLabel}`)
  try {
    if (source === 'button') {
      recordStage('client.stratagem', opId, 'yield_assistant')
      await yieldAssistantWindow()
      await new Promise((resolve) => window.setTimeout(resolve, 260))
    } else if (compactComposerWasFocused) {
      recordStage('client.stratagem', opId, 'dismiss_overlay')
      await gameOverlay.dismissCompact()
    } else {
      await new Promise((resolve) => window.setTimeout(resolve, 160))
    }
    recordStage('client.stratagem', opId, 'inject_start', `sequence_len=${normalizedMacro.sequence.length}`)
    const result = await sendStratagemMacro(normalizedMacro, directionInputMode, generation)
    if (!result.ok) {
      recordStage('client.stratagem', opId, 'inject_failed', `code=${result.error?.code ?? 'unknown'}`)
      throw result.error ?? new Error(result.message)
    }
    recordStage('client.stratagem', opId, 'inject_done')
    const handoffOk = await bestEffortGameplayHandoff('client.stratagem', opId)
    succeeded = true
    if (handoffOk) setNotice('success', '战备已触发', `${normalizedMacro.label}：${formatStratagemSequence(normalizedMacro.sequence)} · ${directionModeLabel}`)
  } catch (error) {
    recordStage('client.stratagem', opId, 'failed', errorMessage(error))
    if (source === 'button') await restoreAssistantWindow().catch(() => undefined)
    setErrorNotice('战备触发失败', error)
  } finally {
    isStratagemRunning.value = false
    if (source === 'button' && desktopRuntime) await gameOverlay.resumeCompactIfGame()
    if (!succeeded && compactComposerWasFocused) {
      await gameOverlay.resumeCompactIfGame()
      await gameOverlay.focusComposer()
    }
    recordStage('client.stratagem', opId, 'finish', `succeeded=${succeeded}`)
  }
}

async function bestEffortGameplayHandoff(scope: string, opId: string): Promise<boolean> {
  if (!desktopRuntime || !savedSettings.autoRestoreGameplayInput) {
    if (desktopRuntime) recordStage(scope, opId, 'gameplay_handoff_skipped', 'reason=setting_disabled')
    return true
  }
  recordStage(scope, opId, 'gameplay_handoff_start')
  try {
    const result = await handoffGameplayInput()
    recordStage(
      scope,
      opId,
      'gameplay_handoff_verified',
      `changed=${result.changed} before=0x${result.beforeLayout.toString(16)} active=0x${result.activeLayout.toString(16)}`,
    )
    return true
  } catch (error) {
    const message = errorMessage(error)
    recordStage(scope, opId, 'gameplay_handoff_fallback', message)
    const code =
      typeof error === 'object' && error !== null && 'code' in error
        ? String((error as { code?: unknown }).code ?? '')
        : ''
    if (code === 'KEYBOARD_LAYOUT_UNAVAILABLE') {
      setNotice(
        'error',
        '消息已发送，但游戏状态未完全恢复',
        `${message}\n程序不会再强制切换 Windows 键盘布局。请点击“恢复输入状态”后再继续。`,
        ['openSettings', 'exportLogs'],
      )
      return false
    }
    setNotice(
      'error',
      '消息已发送，但输入状态需要恢复',
      `${message}\n点击“恢复输入状态”后再继续游戏操作。`,
      ['restoreInputState', 'exportLogs'],
    )
    return false
  }
}

function addStratagemMacro(): void {
  if (settingsDraft.stratagemMacros.length >= 12) {
    setNotice('error', '战备数量已达上限', '最多保存 12 个战备预设。')
    return
  }
  settingsDraft.stratagemMacros.push({
    label: '新战备',
    hotkey: '',
    menuKey: 'ControlLeft',
    menuMode: 'hold',
    sequence: ['KeyW', 'KeyD'],
    menuOpenDelayMs: DEFAULT_STRATAGEM_MENU_OPEN_DELAY_MS,
    pressDelayMs: DEFAULT_STRATAGEM_PRESS_DELAY_MS,
    intervalDelayMs: DEFAULT_STRATAGEM_INTERVAL_DELAY_MS,
  })
}

function addStratagemPreset(preset: StratagemPreset): void {
  if (settingsDraft.stratagemMacros.length >= 12) {
    setNotice('error', '战备数量已达上限', '最多保存 12 个战备预设。')
    return
  }
  settingsDraft.stratagemMacros.push({
    label: preset.zhName,
    hotkey: '',
    menuKey: 'ControlLeft',
    menuMode: 'hold',
    sequence: [...preset.sequence],
    menuOpenDelayMs: DEFAULT_STRATAGEM_MENU_OPEN_DELAY_MS,
    pressDelayMs: DEFAULT_STRATAGEM_PRESS_DELAY_MS,
    intervalDelayMs: DEFAULT_STRATAGEM_INTERVAL_DELAY_MS,
  })
  setNotice('idle', '战备已加入待保存', `${preset.zhName}：${formatStratagemSequence(preset.sequence)}`)
}

function isStratagemPresetAdded(preset: StratagemPreset): boolean {
  return settingsDraft.stratagemMacros.some(
    (macroConfig) =>
      macroConfig.label.trim() === preset.zhName &&
      macroConfig.sequence.join(',') === preset.sequence.join(','),
  )
}

function removeStratagemMacro(index: number): void {
  if (stratagemHotkeyRecordingIndex.value === index) stopStratagemHotkeyRecording()
  settingsDraft.stratagemMacros.splice(index, 1)
}

function startStratagemHotkeyRecording(index: number): void {
  stopQuickHotkeyRecording()
  stopStratagemHotkeyRecording()
  stratagemHotkeyRecordingIndex.value = index
  window.addEventListener('keydown', onStratagemHotkeyKeydown, true)
}

function stopStratagemHotkeyRecording(): void {
  stratagemHotkeyRecordingIndex.value = null
  window.removeEventListener('keydown', onStratagemHotkeyKeydown, true)
}

function onStratagemHotkeyKeydown(event: KeyboardEvent): void {
  event.preventDefault()
  event.stopPropagation()
  if (event.code === 'Escape') {
    stopStratagemHotkeyRecording()
    return
  }
  const accelerator = stratagemAcceleratorFromKeyboardEvent(
    event,
    settingsDraft.stratagemAllowBareNumberHotkeys,
  )
  if (!accelerator || stratagemHotkeyRecordingIndex.value === null) return
  const macroConfig = settingsDraft.stratagemMacros[stratagemHotkeyRecordingIndex.value]
  if (!macroConfig) {
    stopStratagemHotkeyRecording()
    return
  }
  macroConfig.hotkey = accelerator
  stopStratagemHotkeyRecording()
}

function sequenceText(macroConfig: StratagemMacro): string {
  return macroConfig.sequence.map(directionLabel).join(' ')
}

function normalizeStratagemMacro(macroConfig: StratagemMacro): StratagemMacro {
  return {
    ...macroConfig,
    sequence: macroConfig.sequence.map(normalizeStratagemDirectionCode).filter(Boolean),
  }
}

function normalizeStratagemDirectionCode(code: string): string {
  return directionCode(code) ?? code.trim()
}

function updateStratagemSequence(macroConfig: StratagemMacro, event: Event): void {
  macroConfig.sequence = parseStratagemSequence((event.target as HTMLInputElement).value)
}

function parseStratagemSequence(value: string): string[] {
  const compact = value.trim()
  if (!compact) return []
  const tokens = compact.includes(' ')
    ? compact.split(/[\s,，、]+/)
    : Array.from(compact)
  return tokens.map(directionCode).filter((code): code is string => Boolean(code)).slice(0, 16)
}

function directionCode(token: string): string | null {
  const normalized = token.trim()
  const upper = normalized.toUpperCase()
  const map: Record<string, string> = {
    W: 'KeyW',
    A: 'KeyA',
    S: 'KeyS',
    D: 'KeyD',
    KEYW: 'KeyW',
    KEYA: 'KeyA',
    KEYS: 'KeyS',
    KEYD: 'KeyD',
    UP: 'KeyW',
    LEFT: 'KeyA',
    DOWN: 'KeyS',
    RIGHT: 'KeyD',
    ARROWUP: 'KeyW',
    ARROWLEFT: 'KeyA',
    ARROWDOWN: 'KeyS',
    ARROWRIGHT: 'KeyD',
    '↑': 'KeyW',
    '←': 'KeyA',
    '↓': 'KeyS',
    '→': 'KeyD',
    上: 'KeyW',
    左: 'KeyA',
    下: 'KeyS',
    右: 'KeyD',
  }
  return map[upper] ?? map[normalized] ?? null
}

function directionLabel(code: string): string {
  const labels: Record<string, string> = {
    ArrowUp: '↑',
    ArrowLeft: '←',
    ArrowDown: '↓',
    ArrowRight: '→',
    KeyW: '↑',
    KeyA: '←',
    KeyS: '↓',
    KeyD: '→',
  }
  return labels[code] ?? code
}

function formatStratagemSequence(sequence: string[]): string {
  return sequence.map(directionLabel).join(' ')
}

function stratagemDirectionInputModeLabel(mode: StratagemDirectionInputMode): string {
  return mode === 'arrowKeys' ? '方向键' : 'WASD'
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

function onTranslationHudPositionToggle(event: Event): void {
  settingsDraft.translationHudPosition = (event.target as HTMLInputElement).checked
    ? settingsDraft.translationHudPosition ?? { x: 0.03, y: 0.45 }
    : null
}

function shouldStayInTypingOverlay(): boolean {
  return wantsTypingOverlayTranslation.value && gameOverlay.isCompact.value
}

async function restoreAfterChatTranslation(stayInTypingOverlay: boolean): Promise<void> {
  if (!desktopRuntime) return
  if (stayInTypingOverlay) {
    await gameOverlay.resumeCompactIfGame(true)
    return
  }
  await restoreAssistantWindow({ focus: true })
}

async function runChatTranslation(restoreWhenDone: boolean): Promise<void> {
  if (isTranslatingChat.value) return
  const generation = activeGeneration.value
  const stayInTypingOverlay = shouldStayInTypingOverlay()
  const opId = nextClientOperationId('chat-translation')
  recordStage(
    'client.chat_translation',
    opId,
    'start',
    `generation=${generation ?? 'none'} restore=${restoreWhenDone} stay_overlay=${stayInTypingOverlay}`,
  )
  if (!generation) {
    let recoveryError: unknown = null
    if (restoreWhenDone) {
      try {
        await restoreAfterChatTranslation(stayInTypingOverlay)
      } catch (error) {
        recoveryError = error
      }
    }
    activeView.value = stayInTypingOverlay ? 'compose' : 'translate'
    setNotice(
      'error',
      '没有目标会话',
      recoveryError
        ? errorTextWithAdvice(
          `请先捕获 HD2 窗口并校准聊天区域；窗口恢复失败：${errorMessage(recoveryError)}`,
          { title: '没有目标会话' },
        )
        : errorTextWithAdvice('请先捕获 HD2 窗口并校准聊天区域。', { title: '没有目标会话' }),
    )
    recordStage('client.chat_translation', opId, 'failed', 'missing_generation')
    return
  }
  isTranslatingChat.value = true
  try {
    if (desktopRuntime && restoreWhenDone) {
      if (gameOverlay.isCompact.value) {
        recordStage('client.chat_translation', opId, 'dismiss_overlay')
        await gameOverlay.dismissCompact()
      } else {
        recordStage('client.chat_translation', opId, 'yield_assistant')
        await yieldAssistantWindow()
      }
      await new Promise((resolve) => window.setTimeout(resolve, CHAT_CAPTURE_FOREGROUND_SETTLE_MS))
    }
    recordStage('client.chat_translation', opId, 'capture_start')
    const result = await translateChatCapture(generation)
    recordStage('client.chat_translation', opId, 'capture_done', `lines=${result.lines.length}`)
    const hasNewLines = result.lines.length > 0
    if (hasNewLines) {
      translationHistory.value.unshift({ id: Date.now(), ...result, translatedAt: Date.now() })
      translationHistory.value = translationHistory.value.slice(0, 20)
    }
    if (stayInTypingOverlay) {
      activeView.value = 'compose'
      await restoreAfterChatTranslation(true)
      if (hasNewLines) {
        await translationHud.show(translationHistory.value.slice(0, 3), {
          foreground: latestGameForeground.value,
          chatRegion: savedSettings.chatRegion,
          position: savedSettings.translationHudPosition,
        })
      }
    } else {
      await translationHud.hide()
      activeView.value = 'translate'
      await restoreAssistantWindow({ focus: true })
    }
    setNotice(
      'success',
      hasNewLines ? '新聊天翻译完成' : '没有发现新聊天',
      hasNewLines
        ? `离线 OCR：${result.messageOcrLanguage}；识别到 ${result.lines.length} 条新消息，旧聊天行未重复发送。`
        : `离线 OCR：${result.messageOcrLanguage}；当前聊天行均已处理，未调用翻译接口。`,
    )
    recordStage('client.chat_translation', opId, 'done', `lines=${result.lines.length}`)
  } catch (error) {
    recordStage('client.chat_translation', opId, 'failed', errorMessage(error))
    let recoveryError: unknown = null
    if (restoreWhenDone) {
      try {
        await restoreAfterChatTranslation(stayInTypingOverlay)
      } catch (restoreError) {
        recoveryError = restoreError
      }
    }
    activeView.value = stayInTypingOverlay ? 'compose' : 'translate'
    setNotice(
      'error',
      '聊天翻译失败',
      recoveryError
        ? errorTextWithAdvice(
          `${errorMessage(error)}；窗口恢复失败：${errorMessage(recoveryError)}`,
          { title: '聊天翻译失败' },
        )
        : errorMessageWithAdvice(error, { title: '聊天翻译失败' }),
    )
  } finally {
    isTranslatingChat.value = false
    recordStage('client.chat_translation', opId, 'finish')
  }
}

async function startManualChatTranslation(): Promise<void> {
  if (!activeGeneration.value || anyBusy.value) return
  setNotice('working', '正在读取聊天区域', '助手将最小化并读取已校准区域。')
  await runChatTranslation(true)
}

async function startCalibration(): Promise<void> {
  const generation = activeGeneration.value
  if (!generation || anyBusy.value) {
    setErrorTextNotice('无法校准', '请先捕获 HD2 窗口。')
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
    setErrorNotice('校准截图失败', error)
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
  if (restoreHotkey.isRecording.value || captureHotkey.isRecording.value || overlayChatKeyRecording.value || stratagemHotkeyRecordingIndex.value !== null) return
  if (anyBusy.value) { event.preventDefault(); return }
  if (composition.shouldBlockKeydown(event)) return
  if (event.key === 'Escape') {
    event.preventDefault()
    void clearDraft()
    return
  }
  if (event.key === 'ArrowUp') { event.preventDefault(); text.value = history.browseOlder(text.value); return }
  if (event.key === 'ArrowDown') { event.preventDefault(); text.value = history.browseNewer(text.value); return }
  if (event.key === 'Enter') {
    if (event.repeat) return
    const intent = submitIntentFromKeydown(event)
    if (!intent) {
      submitOnEnterRelease.value = null
      return
    }
    event.preventDefault()
    submitOnEnterRelease.value = intent
  }
}

function onKeyup(event: KeyboardEvent): void {
  const blocked = composition.shouldBlockKeyup(event)
  if (event.key !== 'Enter') return
  const intent = submitOnEnterRelease.value
  submitOnEnterRelease.value = null
  if (!blocked && intent) void submit(intent)
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
      unlisteners.push(
        await listen<GameForegroundEvent>('game-chat-key-physical', (event) => {
          if (event.payload.state === 'game') observePhysicalGameChatKey()
        }),
        await listen<GameForegroundEvent>('game-chat-escape-physical', (event) => {
          if (event.payload.state === 'game') observePhysicalGameEscapeKey()
        }),
        await listen<void>(SINGLE_INSTANCE_RESTORE_EVENT, () => {
          void gameOverlay.restoreWindow(false)
            .then(() => {
              activeView.value = 'compose'
              focusInput()
              void recordClientDiagnostic('single_instance_restore', 'stage=synced').catch(() => undefined)
            })
            .catch((error) => setErrorNotice('窗口恢复失败', error))
        }),
      )
      await restoreHotkey.ensureRegistered()
      await captureHotkey.applyAccelerator(settings.captureHotkey)
      await quickShoutHotkeys.sync(settings.quickShouts)
      await stratagemHotkeys.sync(settings.stratagemMacros)
      await gameOverlay.start()
    }
  } catch (error) {
    setErrorNotice('初始化失败', error)
  }
  if (!desktopRuntime) await refreshTarget()
  else void runUpdateCheck(false)
})

onUnmounted(() => {
  for (const unlisten of unlisteners.splice(0)) unlisten()
  stopQuickHotkeyRecording()
  stopStratagemHotkeyRecording()
  stopOverlayChatKeyRecording()
  void gameOverlay.dispose()
  translationHud.dispose()
})
</script>

<template>
  <main class="app-shell">
    <section class="workspace" aria-labelledby="app-title">
      <header class="app-header">
        <div class="brand-mark" aria-hidden="true"><span>H2</span></div>
        <div class="brand-copy"><p class="eyebrow">HELLDIVERS 2 / CHAT CONSOLE</p><h1 id="app-title">中文输入与聊天翻译</h1></div>
        <nav class="view-tabs" aria-label="工作视图">
          <button :class="{ active: activeView === 'compose' }" type="button" @click="showView('compose')">发言</button>
          <button :class="{ active: activeView === 'translate' }" type="button" @click="showView('translate')">聊天翻译</button>
          <button :class="{ active: activeView === 'stratagem' }" type="button" @click="showView('stratagem')">战备</button>
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
            <div class="notice" :data-tone="noticeTone" role="status" aria-live="polite">
              <span class="notice-signal" aria-hidden="true"></span>
              <div>
                <strong>{{ noticeTitle }}</strong>
                <p>{{ noticeMessage }}</p>
                <div v-if="noticeActions.length" class="notice-actions">
                  <button
                    v-for="action in noticeActions"
                    :key="action"
                    class="secondary-button"
                    type="button"
                    :disabled="noticeActionDisabled(action)"
                    @click="runNoticeAction(action)"
                  >
                    {{ noticeActionLabel(action) }}
                  </button>
                </div>
              </div>
            </div>
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
            <div class="notice translation-notice" :data-tone="noticeTone" role="status" aria-live="polite">
              <span class="notice-signal" aria-hidden="true"></span>
              <div>
                <strong>{{ noticeTitle }}</strong>
                <p>{{ noticeMessage }}</p>
                <div v-if="noticeActions.length" class="notice-actions">
                  <button
                    v-for="action in noticeActions"
                    :key="action"
                    class="secondary-button"
                    type="button"
                    :disabled="noticeActionDisabled(action)"
                    @click="runNoticeAction(action)"
                  >
                    {{ noticeActionLabel(action) }}
                  </button>
                </div>
              </div>
            </div>
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

          <template v-else-if="activeView === 'stratagem'">
            <div class="section-heading composer-heading">
              <div><p class="eyebrow">STRATAGEMS</p><h2>战备模块</h2></div>
              <div class="stratagem-toolbar">
                <label class="stratagem-mode-control">
                  <span>方向按键</span>
                  <select v-model="settingsDraft.stratagemDirectionInputMode" :disabled="anyBusy">
                    <option value="wasd">WASD</option>
                    <option value="arrowKeys">方向键 ↑↓←→</option>
                  </select>
                </label>
                <label class="stratagem-number-hotkey-toggle" title="开启后可把主键盘数字或小键盘数字录为战备单键热键">
                  <input v-model="settingsDraft.stratagemAllowBareNumberHotkeys" type="checkbox" :disabled="anyBusy" />
                  <span>允许数字单键热键</span>
                </label>
                <button class="secondary-button" type="button" :disabled="anyBusy || settingsDraft.stratagemMacros.length >= 12" @click="addStratagemMacro">自定义战备</button>
                <button class="primary-button compact" type="button" :disabled="anyBusy" @click="saveSettings()">{{ isSavingSettings ? '保存中…' : '保存战备' }}</button>
              </div>
            </div>
            <section class="stratagem-library-panel" aria-labelledby="stratagem-library-heading">
              <div class="subsection-heading">
                <div><p class="eyebrow">LIBRARY</p><h3 id="stratagem-library-heading">战备库</h3></div>
                <span>{{ filteredStratagemPresets.length }} / {{ STRATAGEM_PRESETS.length }}</span>
              </div>
              <div class="stratagem-library-tools">
                <input v-model="stratagemSearch" type="search" placeholder="搜索战备名称、英文名或方向" />
                <div class="stratagem-group-tabs" aria-label="战备分类">
                  <button
                    v-for="group in stratagemGroupOptions"
                    :key="group.value"
                    type="button"
                    :aria-pressed="stratagemGroupFilter === group.value"
                    @click="stratagemGroupFilter = group.value"
                  >
                    {{ group.label }}
                  </button>
                </div>
              </div>
              <div class="stratagem-preset-grid">
                <article v-for="preset in filteredStratagemPresets" :key="preset.id" class="stratagem-preset-card">
                  <div class="stratagem-preset-copy">
                    <strong>{{ preset.zhName }}</strong>
                    <span>{{ STRATAGEM_GROUP_LABELS[preset.group] }} · {{ preset.enName }}</span>
                    <kbd>{{ formatStratagemSequence(preset.sequence) }}</kbd>
                  </div>
                  <button class="secondary-button" type="button" :disabled="anyBusy || settingsDraft.stratagemMacros.length >= 12 || isStratagemPresetAdded(preset)" @click="addStratagemPreset(preset)">
                    {{ isStratagemPresetAdded(preset) ? '已加入' : '加入' }}
                  </button>
                </article>
              </div>
            </section>
            <section class="quick-shout-panel stratagem-panel" aria-labelledby="stratagem-launch-heading">
              <div class="subsection-heading">
                <div><p class="eyebrow">LAUNCH</p><h3 id="stratagem-launch-heading">一键触发</h3></div>
                <span>按钮使用已保存预设</span>
              </div>
              <div v-if="savedSettings.stratagemMacros.length" class="quick-shout-grid">
                <button
                  v-for="(macroConfig, index) in savedSettings.stratagemMacros"
                  :key="`${macroConfig.label}-${index}`"
                  class="quick-shout-button stratagem-button"
                  type="button"
                  :disabled="anyBusy"
                  :title="formatStratagemSequence(macroConfig.sequence)"
                  @click="runStratagemMacro(macroConfig, 'button')"
                >
                  <span>{{ macroConfig.label }}</span>
                  <kbd>{{ macroConfig.hotkey ? formatHotkeyLabel(macroConfig.hotkey) : formatStratagemSequence(macroConfig.sequence) }}</kbd>
                </button>
              </div>
              <div v-else class="empty-inline">先添加并保存一个战备预设</div>
            </section>
            <section class="stratagem-editor-panel" aria-labelledby="stratagem-editor-heading">
              <div class="subsection-heading">
                <div><p class="eyebrow">LOADOUT</p><h3 id="stratagem-editor-heading">手动配置</h3></div>
                <span>{{ settingsDraft.stratagemMacros.length }} / 12</span>
              </div>
              <div v-if="settingsDraft.stratagemMacros.length" class="quick-shout-editor-list">
                <div v-for="(macroConfig, index) in settingsDraft.stratagemMacros" :key="index" class="quick-shout-editor stratagem-editor">
                  <label>名称<input v-model="macroConfig.label" maxlength="24" type="text" /></label>
                  <label>方向序列<input :value="sequenceText(macroConfig)" maxlength="48" type="text" placeholder="WASD / ↑ → ↓ ← / 上右下左" @input="updateStratagemSequence(macroConfig, $event)" /></label>
                  <label>战备菜单键<select v-model="macroConfig.menuKey"><option value="ControlLeft">左 Ctrl</option><option value="AltLeft">左 Alt</option><option value="Tab">Tab</option><option value="CapsLock">CapsLock</option></select></label>
                  <label>菜单模式<select v-model="macroConfig.menuMode"><option value="hold">按住菜单键</option><option value="toggle">点按切换</option></select></label>
                  <label class="delay-setting">
                    <span class="delay-setting-label">开菜单等待 <output>{{ macroConfig.menuOpenDelayMs }} ms</output></span>
                    <input v-model.number="macroConfig.menuOpenDelayMs" type="range" :min="MIN_STRATAGEM_DELAY_MS" :max="MAX_STRATAGEM_DELAY_MS" :step="STRATAGEM_DELAY_STEP_MS" />
                  </label>
                  <label class="delay-setting">
                    <span class="delay-setting-label">按键按住 <output>{{ macroConfig.pressDelayMs }} ms</output></span>
                    <input v-model.number="macroConfig.pressDelayMs" type="range" :min="MIN_STRATAGEM_DELAY_MS" :max="MAX_STRATAGEM_DELAY_MS" :step="STRATAGEM_DELAY_STEP_MS" />
                  </label>
                  <label class="delay-setting">
                    <span class="delay-setting-label">方向间隔 <output>{{ macroConfig.intervalDelayMs }} ms</output></span>
                    <input v-model.number="macroConfig.intervalDelayMs" type="range" :min="MIN_STRATAGEM_DELAY_MS" :max="MAX_STRATAGEM_DELAY_MS" :step="STRATAGEM_DELAY_STEP_MS" />
                  </label>
                  <div class="quick-hotkey-field stratagem-hotkey-field">
                    <span class="field-label">全局热键</span>
                    <strong>{{ macroConfig.hotkey ? formatHotkeyLabel(macroConfig.hotkey) : '未绑定' }}</strong>
                    <button class="secondary-button" type="button" :disabled="anyBusy" @click="stratagemHotkeyRecordingIndex === index ? stopStratagemHotkeyRecording() : startStratagemHotkeyRecording(index)">{{ stratagemHotkeyRecordingIndex === index ? (settingsDraft.stratagemAllowBareNumberHotkeys ? '按热键…' : '按组合键…') : '录制' }}</button>
                    <button class="ghost-button" type="button" :disabled="anyBusy || !macroConfig.hotkey" @click="macroConfig.hotkey = ''">清除</button>
                    <button class="ghost-button" type="button" :disabled="anyBusy || macroConfig.sequence.length === 0" @click="runStratagemMacro(macroConfig, 'button', settingsDraft.stratagemDirectionInputMode)">测试</button>
                    <button class="ghost-button danger-action" type="button" :disabled="anyBusy" @click="removeStratagemMacro(index)">删除</button>
                  </div>
                </div>
              </div>
              <div v-else class="empty-state">点击“添加战备”开始手动配置</div>
            </section>
            <div class="notice settings-notice" :data-tone="noticeTone" role="status" aria-live="polite">
              <span class="notice-signal" aria-hidden="true"></span>
              <div>
                <strong>{{ noticeTitle }}</strong>
                <p>{{ noticeMessage }}</p>
                <div v-if="noticeActions.length" class="notice-actions">
                  <button
                    v-for="action in noticeActions"
                    :key="action"
                    class="secondary-button"
                    type="button"
                    :disabled="noticeActionDisabled(action)"
                    @click="runNoticeAction(action)"
                  >
                    {{ noticeActionLabel(action) }}
                  </button>
                </div>
              </div>
            </div>
          </template>

          <template v-else>
            <div class="section-heading composer-heading"><div><p class="eyebrow">TRANSLATION API</p><h2>翻译与热键设置</h2></div><span class="key-status">{{ savedSettings.apiKeyConfigured ? 'Key 已保存' : '未配置 Key' }}</span></div>
            <form class="settings-form" @submit.prevent="saveSettings()">
              <label>API 地址<input v-model="settingsDraft.apiUrl" type="url" placeholder="https://example.com/v1" /></label>
              <label>网络代理（可选）<input v-model="settingsDraft.proxyUrl" type="url" placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:7891" /></label>
              <label>模型名<input v-model="settingsDraft.model" type="text" placeholder="gpt-4.1-mini" /></label>
              <label>API Key<input v-model="settingsDraft.apiKey" type="password" :placeholder="savedSettings.apiKeyConfigured ? '留空以保留已保存 Key' : '可选，本地接口可留空'" /></label>
              <label>离线 OCR<select v-model="settingsDraft.ocrLanguage"><option value="auto">自动中英混合（推荐）</option><option v-for="language in ocrLanguages" :key="language.tag" :value="language.tag">回退：{{ language.nativeName }} · {{ language.tag }}</option></select></label>
              <label>英译中结果位置<select v-model="settingsDraft.incomingTranslationDisplayMode"><option value="chatTranslationPage">聊天翻译页</option><option value="typingOverlay">游戏内 HUD</option></select></label>
              <div v-if="settingsDraft.incomingTranslationDisplayMode === 'typingOverlay'" class="hud-position-settings">
                <label class="overlay-toggle-setting"><input type="checkbox" :checked="settingsDraft.translationHudPosition !== null" @change="onTranslationHudPositionToggle" />自定义 HUD 位置</label>
                <div v-if="settingsDraft.translationHudPosition" class="hud-position-grid">
                  <label>
                    <span class="delay-setting-label">水平位置 <output>{{ Math.round(settingsDraft.translationHudPosition.x * 100) }}%</output></span>
                    <input v-model.number="settingsDraft.translationHudPosition.x" type="range" min="0" max="1" step="0.01" />
                  </label>
                  <label>
                    <span class="delay-setting-label">垂直位置 <output>{{ Math.round(settingsDraft.translationHudPosition.y * 100) }}%</output></span>
                    <input v-model.number="settingsDraft.translationHudPosition.y" type="range" min="0" max="1" step="0.01" />
                  </label>
                </div>
              </div>
              <label>游戏聊天英译中提示词<textarea v-model="settingsDraft.incomingPrompt" rows="8" placeholder="留空时使用内置《绝地潜兵2》玩家黑话提示词"></textarea></label>
              <label>输入消息中译英提示词<textarea v-model="settingsDraft.outgoingPrompt" rows="8" placeholder="留空时使用内置《绝地潜兵2》Gamer Slang 提示词"></textarea></label>
              <label class="overlay-toggle-setting"><input v-model="settingsDraft.gameOverlayEnabled" type="checkbox" />游戏前台时启用右侧中文输入栏</label>
              <label class="overlay-toggle-setting"><input v-model="settingsDraft.autoLockCaps" type="checkbox" />启用 CapsLock 输入法保护</label>
              <label class="overlay-toggle-setting"><input v-model="settingsDraft.autoRestoreGameplayInput" type="checkbox" />发送后自动恢复游戏状态（隐藏侧栏并恢复 HD2，不切换 Windows 键盘布局）</label>
              <div class="settings-row">
                <div><span class="field-label">游戏聊天键</span><strong>{{ overlayChatKeyLabel(settingsDraft.overlayChatKey) }}</strong></div>
                <button class="secondary-button" type="button" :disabled="anyBusy || !settingsDraft.gameOverlayEnabled" @click="overlayChatKeyRecording ? stopOverlayChatKeyRecording() : startOverlayChatKeyRecording()">{{ overlayChatKeyRecording ? '按下单个按键…' : '重新绑定' }}</button>
              </div>
              <label>游戏文字注入方式<select v-model="settingsDraft.gameInputMethod"><option value="unicodeSendInput">Unicode 逐字符（稳定推荐）</option><option value="gbkAltCode" disabled>GBK Alt 数字码（已停用）</option></select></label>
              <label class="delay-setting">
                <span class="delay-setting-label">文字输入间隔 <output>{{ settingsDraft.gameInputDelayMs }} ms</output></span>
                <input v-model.number="settingsDraft.gameInputDelayMs" type="range" :min="MIN_GAME_INPUT_DELAY_MS" :max="MAX_GAME_INPUT_DELAY_MS" :step="GAME_INPUT_DELAY_STEP_MS" />
              </label>
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
              <div class="settings-actions"><button class="primary-button compact" type="submit" :disabled="anyBusy">{{ isSavingSettings ? '保存中…' : '保存设置' }}</button><button class="secondary-button" type="button" :disabled="anyBusy" @click="testApi">{{ isTestingApi ? '测试中…' : '测试接口' }}</button><button class="secondary-button" type="button" :disabled="anyBusy || isCheckingUpdates" @click="runUpdateCheck(true)">{{ isCheckingUpdates ? '检查中…' : '检查更新' }}</button><button class="secondary-button" type="button" :disabled="anyBusy" @click="exportLogs">{{ isExportingLogs ? '导出中…' : '导出诊断日志' }}</button><button v-if="savedSettings.apiKeyConfigured" class="ghost-button" type="button" :disabled="anyBusy" @click="saveSettings({ clearKey: true })">清除 Key</button></div>
            </form>
            <div class="notice settings-notice" :data-tone="noticeTone" role="status" aria-live="polite">
              <span class="notice-signal" aria-hidden="true"></span>
              <div>
                <strong>{{ noticeTitle }}</strong>
                <p>{{ noticeMessage }}</p>
                <div v-if="noticeActions.length" class="notice-actions">
                  <button
                    v-for="action in noticeActions"
                    :key="action"
                    class="secondary-button"
                    type="button"
                    :disabled="noticeActionDisabled(action)"
                    @click="runNoticeAction(action)"
                  >
                    {{ noticeActionLabel(action) }}
                  </button>
                </div>
              </div>
            </div>
          </template>
        </section>
        <TargetDiagnosticPanel :diagnostic="target" :loading="isCapturing || isRefreshing" :available="canCapture" @capture="captureTarget" />
      </div>
      <footer class="safety-note"><span class="safety-line" aria-hidden="true"></span><p>仅向已锁定且重新验证的前台 HD2 窗口注入；截图使用 Windows 本地 OCR，不上传游戏画面。</p></footer>
    </section>
  </main>
</template>

<style scoped>
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
.send-actions, .translation-toolbar, .settings-actions, .calibration-actions, .stratagem-toolbar { display: flex; gap: 10px; align-items: center; }
.settings-actions { flex-wrap: wrap; }
.notice-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }
.notice-actions .secondary-button { min-height: 32px; padding: 0 12px; border-radius: 6px; font-size: 11px; }
.stratagem-toolbar { flex-wrap: wrap; justify-content: flex-end; }
.stratagem-mode-control { display: grid; grid-template-columns: auto minmax(112px, 150px); gap: 8px; align-items: center; color: var(--text-secondary); font-size: 12px; font-weight: 650; }
.stratagem-mode-control select { width: 100%; min-height: 36px; padding: 0 10px; border: 1px solid var(--line-strong); border-radius: 6px; outline: none; background: #11140f; color: var(--text); }
.stratagem-mode-control select:focus { border-color: var(--accent); box-shadow: 0 0 0 3px var(--focus); }
.stratagem-number-hotkey-toggle { display: flex; min-height: 36px; align-items: center; gap: 7px; color: var(--text-secondary); cursor: pointer; font-size: 12px; font-weight: 650; }
.stratagem-number-hotkey-toggle input { width: 16px; height: 16px; min-height: 0; margin: 0; accent-color: var(--accent); }
.stratagem-number-hotkey-toggle:has(input:disabled) { cursor: default; opacity: .55; }
.send-actions .primary-button { flex: 1; }
.compact { width: auto; min-width: 150px; padding: 0 18px; }
.translation-preview { margin: 0 0 16px; padding: 12px 14px; border-left: 3px solid var(--info); background: #141913; }
.translation-preview span, .field-label { display: block; margin-bottom: 5px; color: var(--text-muted); font-size: 11px; }
.translation-preview p { margin: 0; color: var(--text-secondary); font-size: 13px; line-height: 1.5; }
.quick-shout-panel { margin: 0 0 16px; padding: 14px 0; border-top: 1px solid var(--line); border-bottom: 1px solid var(--line); }
.stratagem-library-panel { display: grid; gap: 12px; margin-bottom: 16px; padding: 14px 0; border-top: 1px solid var(--line); border-bottom: 1px solid var(--line); }
.stratagem-library-tools { display: grid; gap: 10px; }
.stratagem-library-tools input { width: 100%; min-height: 42px; padding: 0 12px; border: 1px solid var(--line-strong); border-radius: 6px; outline: none; background: #11140f; color: var(--text); }
.stratagem-library-tools input:focus { border-color: var(--accent); box-shadow: 0 0 0 3px var(--focus); }
.stratagem-group-tabs { display: flex; flex-wrap: wrap; gap: 6px; }
.stratagem-group-tabs button { min-height: 30px; padding: 0 10px; border: 1px solid var(--line-strong); border-radius: 5px; background: #141812; color: var(--text-muted); cursor: pointer; font-size: 11px; font-weight: 700; }
.stratagem-group-tabs button[aria-pressed='true'] { border-color: var(--accent); color: var(--accent); background: rgba(230, 200, 76, .1); }
.stratagem-preset-grid { display: grid; gap: 8px; max-height: 360px; overflow-y: auto; padding-right: 4px; scrollbar-gutter: stable; }
.stratagem-preset-card { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 10px 12px; border: 1px solid var(--line); border-radius: 7px; background: var(--surface-soft); }
.stratagem-preset-copy { display: grid; min-width: 0; gap: 3px; }
.stratagem-preset-copy strong { overflow: hidden; color: var(--text); font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.stratagem-preset-copy span { overflow: hidden; color: var(--text-muted); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.stratagem-preset-copy kbd { color: var(--accent); font-size: 15px; letter-spacing: 0; }
.stratagem-preset-card button { min-width: 74px; min-height: 34px; padding: 0 12px; }
.stratagem-panel { margin-top: -4px; }
.stratagem-editor-panel { display: grid; gap: 11px; padding-top: 4px; border-top: 1px solid var(--line); }
.subsection-heading { display: flex; align-items: center; justify-content: space-between; gap: 14px; margin-bottom: 11px; }
.subsection-heading h3 { margin: 2px 0 0; color: var(--text); font-size: 15px; letter-spacing: 0; }
.subsection-heading > span { color: var(--text-muted); font-size: 10px; }
.quick-shout-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; }
.quick-shout-button { display: grid; grid-template-rows: 20px 17px; min-width: 0; min-height: 52px; align-content: center; justify-items: start; padding: 7px 10px; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 6px; background: var(--surface-soft); color: var(--text-secondary); cursor: pointer; text-align: left; }
.quick-shout-button:hover:not(:disabled) { border-color: var(--accent); color: var(--text); }
.quick-shout-button span { max-width: 100%; overflow: hidden; font-size: 12px; font-weight: 700; text-overflow: ellipsis; white-space: nowrap; }
.quick-shout-button kbd { max-width: 100%; overflow: hidden; color: var(--text-muted); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.quick-shout-button:disabled { cursor: not-allowed; opacity: .45; }
.stratagem-button { border-color: rgba(87, 128, 120, .55); }
.stratagem-button:hover:not(:disabled) { border-color: var(--info); }
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
.hud-position-settings { display: grid; gap: 12px; padding: 12px 14px; border: 1px solid var(--line); border-radius: 6px; background: #121510; }
.hud-position-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
.settings-form textarea { min-height: 150px; padding: 11px 12px; resize: vertical; font-family: ui-monospace, Consolas, monospace; font-size: 11px; line-height: 1.55; }
.settings-form input:focus, .settings-form select:focus, .settings-form textarea:focus { border-color: var(--accent); box-shadow: 0 0 0 3px var(--focus); }
.delay-setting-label { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.delay-setting-label output { color: var(--accent); font-family: ui-monospace, Consolas, monospace; font-size: 12px; }
.quick-shout-settings { padding-top: 4px; border-top: 1px solid var(--line); }
.stratagem-settings { margin-top: 2px; }
.quick-shout-editor-list { display: grid; gap: 9px; }
.quick-shout-editor { display: grid; grid-template-columns: minmax(100px, .55fr) minmax(180px, 1.45fr); gap: 10px; padding: 11px; border: 1px solid var(--line); border-radius: 7px; background: var(--surface-soft); }
.stratagem-editor { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.stratagem-editor label { display: grid; gap: 7px; color: var(--text-secondary); font-size: 12px; font-weight: 650; }
.stratagem-editor input, .stratagem-editor select { width: 100%; min-height: 42px; padding: 0 12px; border: 1px solid var(--line-strong); border-radius: 6px; outline: none; background: #11140f; color: var(--text); }
.stratagem-editor input[type='range'] { min-height: 24px; padding: 0; border: 0; background: transparent; accent-color: var(--accent); }
.stratagem-editor input:focus, .stratagem-editor select:focus { border-color: var(--accent); box-shadow: 0 0 0 3px var(--focus); }
.stratagem-editor .delay-setting { min-width: 0; }
.quick-hotkey-field { display: grid; grid-column: 1 / -1; grid-template-columns: minmax(72px, auto) minmax(120px, 1fr) auto auto auto; gap: 8px; align-items: center; }
.stratagem-hotkey-field { grid-template-columns: minmax(72px, auto) minmax(120px, 1fr) auto auto auto auto; }
.quick-hotkey-field .field-label { margin: 0; }
.quick-hotkey-field strong { min-width: 0; overflow: hidden; color: var(--text-secondary); font-family: ui-monospace, Consolas, monospace; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.quick-hotkey-field button { min-height: 34px; padding: 0 10px; }
.danger-action { color: var(--danger); }
.settings-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 0; border-top: 1px solid var(--line); }
.settings-row strong { color: var(--text-secondary); font-family: ui-monospace, Consolas, monospace; font-size: 12px; }
.security-note { margin: 0; color: var(--danger); font-size: 11px; line-height: 1.55; }
@media (max-width: 820px) { .content-grid { grid-template-columns: 1fr; } .work-panel { min-height: 0; } .quick-shout-grid { grid-template-columns: repeat(3, minmax(0, 1fr)); } }
@media (max-width: 600px) { .view-tabs { width: 100%; margin-left: 0; } .view-tabs button { flex: 1; } .update-banner, .update-banner div { align-items: stretch; flex-direction: column; } .update-banner .compact { width: 100%; } .section-heading, .subsection-heading { align-items: stretch; flex-direction: column; } .segmented-control button { flex: 1; } .send-actions, .translation-toolbar, .settings-actions { align-items: stretch; flex-direction: column; } .send-actions button, .translation-toolbar button, .settings-actions button { width: 100%; } .hud-position-grid { grid-template-columns: 1fr; } .quick-shout-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } .quick-shout-editor { grid-template-columns: 1fr; } .quick-hotkey-field { grid-column: auto; grid-template-columns: 1fr 1fr; } .quick-hotkey-field strong { grid-column: 1 / -1; } .translation-item { grid-template-columns: 1fr; gap: 4px; } .message-speaker.is-empty { display: none; } }
</style>
