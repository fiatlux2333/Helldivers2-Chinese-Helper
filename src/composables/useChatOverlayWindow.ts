import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  getCurrentWindow,
  PhysicalPosition,
  PhysicalSize,
  Window,
} from '@tauri-apps/api/window'
import { readonly, ref, watch, type Ref } from 'vue'

import type { GameForegroundEvent } from '@/composables/useGameOverlayWindow'
import {
  CHAT_OVERLAY_ACTION_EVENT,
  CHAT_OVERLAY_FOCUS_REQUEST_EVENT,
  CHAT_OVERLAY_FOCUS_RESULT_EVENT,
  CHAT_OVERLAY_INPUT_EVENT,
  CHAT_OVERLAY_MODE_EVENT,
  CHAT_OVERLAY_POSITION_EVENT,
  CHAT_OVERLAY_STATE_EVENT,
  CHAT_OVERLAY_WINDOW_LABEL,
  type ChatOverlayAction,
  type ChatOverlayFocusRequest,
  type ChatOverlayFocusResult,
  type ChatOverlayMode,
  type ChatOverlayPosition,
  type ChatOverlayStatePayload,
} from '@/types/chatOverlay'

type ChatOverlayWindowOptions = {
  enabled: Ref<boolean>
  busy: Ref<boolean>
  compactHeight?: Ref<number>
  getComposerState: () => ChatOverlayStatePayload
  onComposerInput: (text: string) => void
  onComposerModeChanged: (mode: ChatOverlayMode) => void
  onComposerAction: (action: ChatOverlayAction) => void | Promise<void>
  onGameForeground: () => void | Promise<void>
  onForegroundChanged?: (event: GameForegroundEvent) => void | Promise<void>
  onComposerFocused: () => void | Promise<void>
  onDiagnostic?: (stage: string, message: string) => void | Promise<void>
  onCapsProtectionFailure?: (message: string) => void
  onError: (message: string) => void
}

type ProgrammaticGeometry = {
  position: PhysicalPosition
  size: PhysicalSize
  expiresAt: number
}

const OVERLAY_WIDTH = 500
const OVERLAY_HEIGHT = 58
const OVERLAY_RIGHT_MARGIN = 20
const OVERLAY_VERTICAL_RATIO = 0.5
const OVERLAY_POSITION_KEY = 'hd2cn.overlay.position.v1'
const GEOMETRY_EVENT_SETTLE_MS = 250
const WINDOW_FOCUS_RETRY_COUNT = 3
const WINDOW_FOCUS_RETRY_DELAY_MS = 40
const DOM_FOCUS_TIMEOUT_MS = 350

export function useChatOverlayWindow(options: ChatOverlayWindowOptions) {
  const isCompact = ref(false)
  const isComposerFocused = ref(false)
  const compactHeight = options.compactHeight ?? ref(OVERLAY_HEIGHT)
  let mainWindow: Window | null = null
  let overlayWindow: Window | null = null
  let latestForeground: GameForegroundEvent = {
    state: 'assistant',
    workArea: null,
    scaleFactor: null,
  }
  let suspended = false
  let hiddenUntilChatKey = false
  let diagnosticRefreshInFlight = false
  let chatKeyPrewarmQueued = false
  let chatKeyShowQueued = false
  let chatKeyPrewarmed = false
  let chatKeyPressedAt = 0
  let geometryMutationDepth = 0
  let ignoreGeometryEventsUntil = 0
  let programmaticGeometryHistory: ProgrammaticGeometry[] = []
  let focusRequestId = 0
  let pendingFocus:
    | {
      requestId: number
      resolve: (result: ChatOverlayFocusResult) => void
      timeoutId: number
    }
    | null = null
  let transition = Promise.resolve()
  const unlisteners: UnlistenFn[] = []

  function diagnostic(stage: string, message: string): void {
    void Promise.resolve(options.onDiagnostic?.(stage, message)).catch(() => undefined)
  }

  function resetChatKeyPrewarm(): void {
    chatKeyPrewarmed = false
    chatKeyPressedAt = 0
  }

  function schedule(operation: () => Promise<void>): Promise<void> {
    const next = transition.then(operation, operation)
    transition = next.catch((error) => {
      options.onError(error instanceof Error ? error.message : String(error))
    })
    return next
  }

  function queue(operation: () => Promise<void>): Promise<void> {
    return schedule(operation).catch(() => undefined)
  }

  function queueStrict(operation: () => Promise<void>): Promise<void> {
    return schedule(operation)
  }

  async function notifyForegroundChanged(payload: GameForegroundEvent): Promise<void> {
    try {
      await options.onForegroundChanged?.(payload)
    } catch (error) {
      options.onError(error instanceof Error ? error.message : String(error))
    }
  }

  async function refreshGameDiagnostic(): Promise<void> {
    if (diagnosticRefreshInFlight) return
    diagnosticRefreshInFlight = true
    try {
      await options.onGameForeground()
    } catch (error) {
      options.onError(error instanceof Error ? error.message : String(error))
    } finally {
      diagnosticRefreshInFlight = false
    }
  }

  function copyPosition(position: PhysicalPosition): PhysicalPosition {
    return new PhysicalPosition(position.x, position.y)
  }

  function savedOverlayPosition(
    workArea: GameForegroundEvent['workArea'],
    width: number,
    height: number,
  ): PhysicalPosition | null {
    if (!workArea) return null
    try {
      const raw = localStorage.getItem(OVERLAY_POSITION_KEY)
      if (!raw) return null
      const parsed = JSON.parse(raw) as { x?: unknown; y?: unknown }
      if (typeof parsed.x !== 'number' || typeof parsed.y !== 'number') return null
      const minX = workArea.x
      const minY = workArea.y
      const maxX = workArea.x + Math.max(0, workArea.width - width)
      const maxY = workArea.y + Math.max(0, workArea.height - height)
      if (parsed.x < minX || parsed.x > maxX || parsed.y < minY || parsed.y > maxY) return null
      return new PhysicalPosition(parsed.x, parsed.y)
    } catch {
      return null
    }
  }

  function isProgrammaticGeometryEvent(position: PhysicalPosition, size?: PhysicalSize): boolean {
    const now = Date.now()
    programmaticGeometryHistory = programmaticGeometryHistory.filter((entry) => entry.expiresAt > now)
    return programmaticGeometryHistory.some((entry) => {
      const samePosition = entry.position.x === position.x && entry.position.y === position.y
      const sameSize = !size || (entry.size.width === size.width && entry.size.height === size.height)
      return samePosition && sameSize
    })
  }

  async function withProgrammaticGeometry<T>(operation: () => Promise<T>): Promise<T> {
    geometryMutationDepth += 1
    try {
      return await operation()
    } finally {
      geometryMutationDepth -= 1
      ignoreGeometryEventsUntil = Math.max(ignoreGeometryEventsUntil, Date.now() + GEOMETRY_EVENT_SETTLE_MS)
    }
  }

  async function rememberProgrammaticGeometry(): Promise<void> {
    if (!overlayWindow) return
    const position = await overlayWindow.outerPosition()
    const size = await overlayWindow.outerSize()
    const now = Date.now()
    programmaticGeometryHistory = [
      {
        position: copyPosition(position),
        size: new PhysicalSize(size.width, size.height),
        expiresAt: now + 2_000,
      },
      ...programmaticGeometryHistory.filter((entry) => entry.expiresAt > now),
    ].slice(0, 4)
  }

  async function sendComposerState(): Promise<void> {
    if (!overlayWindow) return
    await emitTo(CHAT_OVERLAY_WINDOW_LABEL, CHAT_OVERLAY_STATE_EVENT, options.getComposerState())
  }

  function completeFocusRequest(result: ChatOverlayFocusResult): void {
    if (!pendingFocus || result.requestId !== pendingFocus.requestId) return
    window.clearTimeout(pendingFocus.timeoutId)
    const resolve = pendingFocus.resolve
    pendingFocus = null
    resolve(result)
  }

  async function requestDomFocus(): Promise<ChatOverlayFocusResult> {
    if (pendingFocus) {
      window.clearTimeout(pendingFocus.timeoutId)
      pendingFocus.resolve({
        requestId: pendingFocus.requestId,
        focused: false,
        error: '新的输入框聚焦请求已替代旧请求',
      })
      pendingFocus = null
    }
    const requestId = focusRequestId + 1
    focusRequestId = requestId
    const result = new Promise<ChatOverlayFocusResult>((resolve) => {
      const timeoutId = window.setTimeout(() => {
        if (!pendingFocus || pendingFocus.requestId !== requestId) return
        pendingFocus = null
        resolve({ requestId, focused: false, error: '等待侧栏输入框焦点确认超时' })
      }, DOM_FOCUS_TIMEOUT_MS)
      pendingFocus = { requestId, resolve, timeoutId }
    })
    const request: ChatOverlayFocusRequest = { requestId }
    await emitTo(CHAT_OVERLAY_WINDOW_LABEL, CHAT_OVERLAY_FOCUS_REQUEST_EVENT, request)
    return result
  }

  async function placeAndShowOverlay(payload: GameForegroundEvent): Promise<void> {
    if (!overlayWindow || payload.state !== 'game' || !payload.workArea) {
      throw new Error('无法确定 HD2 所在显示器的可用区域')
    }
    const scale = Math.max(1, payload.scaleFactor ?? 1)
    const width = Math.round(OVERLAY_WIDTH * scale)
    const height = Math.round(compactHeight.value * scale)
    const savedPosition = savedOverlayPosition(payload.workArea, width, height)
    const position = savedPosition ?? new PhysicalPosition(
      Math.round(payload.workArea.x + payload.workArea.width - width - OVERLAY_RIGHT_MARGIN * scale),
      Math.round(payload.workArea.y + (payload.workArea.height - height) * OVERLAY_VERTICAL_RATIO),
    )
    await withProgrammaticGeometry(async () => {
      await overlayWindow!.setFocusable(false)
      await overlayWindow!.setSkipTaskbar(true)
      await overlayWindow!.setAlwaysOnTop(true)
      await overlayWindow!.setDecorations(false)
      await overlayWindow!.setShadow(false)
      await overlayWindow!.setResizable(false)
      await overlayWindow!.setSize(new PhysicalSize(width, height))
      await overlayWindow!.setPosition(position)
      await overlayWindow!.show()
      if (await overlayWindow!.isMinimized()) await overlayWindow!.unminimize()
    })
    await rememberProgrammaticGeometry()
    isCompact.value = true
    await sendComposerState()
  }

  async function focusComposer(): Promise<void> {
    if (!overlayWindow || !isCompact.value || suspended || options.busy.value) return
    const startedAt = performance.now()
    diagnostic('overlay_focus', 'stage=start window=chat-overlay')
    await overlayWindow.setFocusable(true)
    await overlayWindow.setAlwaysOnTop(true)
    isComposerFocused.value = false
    let windowFocused = false
    let lastError: unknown = null
    for (let attempt = 0; attempt < WINDOW_FOCUS_RETRY_COUNT; attempt += 1) {
      try {
        diagnostic('overlay_focus', `stage=set_focus attempt=${attempt + 1} window=chat-overlay`)
        await overlayWindow.setFocus()
        const focused = await overlayWindow.isFocused()
        diagnostic('overlay_focus', `stage=verify_window attempt=${attempt + 1} focused=${focused}`)
        if (focused) {
          windowFocused = true
          break
        }
      } catch (error) {
        lastError = error
      }
      if (attempt + 1 < WINDOW_FOCUS_RETRY_COUNT) {
        await new Promise<void>((resolve) => window.setTimeout(resolve, WINDOW_FOCUS_RETRY_DELAY_MS))
      }
    }
    if (!windowFocused) {
      const detail = lastError instanceof Error ? `：${lastError.message}` : ''
      throw new Error(`Windows 未将键盘焦点交给独立中文侧栏${detail}`)
    }
    await sendComposerState()
    const domFocus = await requestDomFocus()
    diagnostic(
      'overlay_focus',
      `stage=verify_composer focused=${domFocus.focused} elapsed_ms=${Math.round(performance.now() - startedAt)}`,
    )
    if (!domFocus.focused) throw new Error(domFocus.error ?? '中文侧栏输入框没有取得键盘焦点')
    await options.onComposerFocused()
    isComposerFocused.value = true
    diagnostic('overlay_focus', `stage=complete elapsed_ms=${Math.round(performance.now() - startedAt)}`)
  }

  async function restoreMainWindow(focus: boolean): Promise<void> {
    if (!mainWindow) return
    await mainWindow.setFocusable(true)
    await mainWindow.setSkipTaskbar(false)
    await mainWindow.show()
    if (await mainWindow.isMinimized()) await mainWindow.unminimize()
    if (focus) {
      await mainWindow.setAlwaysOnTop(true)
      await mainWindow.setFocus()
      await mainWindow.setAlwaysOnTop(false)
    }
  }

  async function recoverFromTransitionFailure(focus = true): Promise<void> {
    resetChatKeyPrewarm()
    isComposerFocused.value = false
    hiddenUntilChatKey = false
    isCompact.value = false
    try {
      if (overlayWindow) {
        await overlayWindow.setFocusable(false)
        await overlayWindow.hide()
      }
      await restoreMainWindow(focus)
    } catch (error) {
      options.onError(`窗口恢复失败：${error instanceof Error ? error.message : String(error)}`)
    }
  }

  async function prewarmComposerForChatKey(): Promise<void> {
    if (!overlayWindow || !options.enabled.value || options.busy.value || latestForeground.state !== 'game') return
    suspended = false
    hiddenUntilChatKey = false
    chatKeyPressedAt = performance.now()
    diagnostic('overlay_trigger', 'stage=keydown_prepare window=chat-overlay')
    try {
      await placeAndShowOverlay(latestForeground)
      chatKeyPrewarmed = true
      diagnostic(
        'overlay_trigger',
        `stage=prewarmed elapsed_ms=${Math.round(performance.now() - chatKeyPressedAt)}`,
      )
    } catch (error) {
      await recoverFromTransitionFailure(true)
      throw error
    }
  }

  async function showComposerForChatKey(): Promise<void> {
    if (!overlayWindow || !options.enabled.value || options.busy.value || latestForeground.state !== 'game') return
    suspended = false
    hiddenUntilChatKey = false
    const startedAt = chatKeyPressedAt || performance.now()
    diagnostic(
      'overlay_trigger',
      `stage=keyup_activate prewarmed=${chatKeyPrewarmed} elapsed_ms=${Math.round(performance.now() - startedAt)}`,
    )
    try {
      if (!chatKeyPrewarmed || !isCompact.value) await placeAndShowOverlay(latestForeground)
      chatKeyPrewarmed = false
      await focusComposer()
    } catch (error) {
      chatKeyPrewarmed = false
      await recoverFromTransitionFailure(true)
      throw error
    }
  }

  async function cancelChatKeyPrewarm(): Promise<void> {
    if (!overlayWindow || !chatKeyPrewarmed || isComposerFocused.value) return
    diagnostic('overlay_trigger', 'stage=prewarm_cancelled foreground_left_game')
    resetChatKeyPrewarm()
    isCompact.value = false
    await overlayWindow.setFocusable(false)
    await overlayWindow.hide()
  }

  async function handleForeground(payload: GameForegroundEvent): Promise<void> {
    latestForeground = payload
    await notifyForegroundChanged(payload)
    if (payload.state !== 'game') await cancelChatKeyPrewarm()
    if (payload.state === 'game' && options.enabled.value) void refreshGameDiagnostic()
  }

  async function handleOverlayFocusChanged(focused: boolean): Promise<void> {
    if (!isCompact.value || options.busy.value) return
    if (!focused) {
      isComposerFocused.value = false
      if (await overlayWindow?.isFocused()) {
        try {
          const domFocus = await requestDomFocus()
          if (!domFocus.focused) throw new Error(domFocus.error ?? '侧栏输入框焦点复核失败')
          await options.onComposerFocused()
          isComposerFocused.value = true
          diagnostic('overlay_focus', 'stage=stale_focus_loss_reverified focused=true')
        } catch (error) {
          diagnostic(
            'overlay_focus',
            `stage=stale_focus_loss_reverify_failed error=${error instanceof Error ? error.message : String(error)}`,
          )
          await recoverFromTransitionFailure(true)
          throw error
        }
      }
      return
    }
    if (hiddenUntilChatKey) {
      await expandFull(true)
    } else if (!isComposerFocused.value) {
      await focusComposer()
    }
  }

  async function handleOverlayMoved(position: PhysicalPosition): Promise<void> {
    if (!isCompact.value || geometryMutationDepth > 0 || Date.now() < ignoreGeometryEventsUntil) return
    const size = await overlayWindow?.outerSize()
    if (size && isProgrammaticGeometryEvent(position, size)) return
    localStorage.setItem(OVERLAY_POSITION_KEY, JSON.stringify({ x: position.x, y: position.y }))
  }

  async function yieldWindow(): Promise<void> {
    if (!mainWindow) return
    resetChatKeyPrewarm()
    suspended = true
    hiddenUntilChatKey = false
    isComposerFocused.value = false
    if (overlayWindow) {
      await overlayWindow.setFocusable(false)
      await overlayWindow.hide()
    }
    await mainWindow.setFocusable(false)
    await mainWindow.setSkipTaskbar(true)
    await mainWindow.hide()
  }

  async function dismissCompact(): Promise<void> {
    if (!overlayWindow || !isCompact.value) return
    resetChatKeyPrewarm()
    diagnostic('overlay_dismiss', 'stage=start window=chat-overlay')
    hiddenUntilChatKey = true
    isComposerFocused.value = false
    await overlayWindow.setFocusable(false)
    await overlayWindow.setSkipTaskbar(true)
    await overlayWindow.hide()
    diagnostic('overlay_dismiss', 'stage=hidden window=chat-overlay')
  }

  async function restoreWindow(focus = true): Promise<void> {
    resetChatKeyPrewarm()
    suspended = false
    hiddenUntilChatKey = false
    isComposerFocused.value = false
    isCompact.value = false
    if (overlayWindow) {
      await overlayWindow.setFocusable(false)
      await overlayWindow.hide()
    }
    await restoreMainWindow(focus)
  }

  async function resumeCompactIfGame(focus = false): Promise<void> {
    suspended = false
    hiddenUntilChatKey = false
    if (!isCompact.value || latestForeground.state !== 'game' || !options.enabled.value) return
    try {
      await placeAndShowOverlay(latestForeground)
      if (focus) await focusComposer()
    } catch (error) {
      await recoverFromTransitionFailure(true)
      throw error
    }
  }

  async function expandFull(focus = true): Promise<void> {
    await restoreWindow(focus)
  }

  async function deactivateComposer(): Promise<void> {
    if (!overlayWindow || !isCompact.value) return
    isComposerFocused.value = false
    await overlayWindow.setFocusable(false)
  }

  async function startDragging(): Promise<void> {
    if (!overlayWindow || !isCompact.value) return
    await overlayWindow.startDragging()
  }

  async function updateComposerState(): Promise<void> {
    if (!overlayWindow) return
    await sendComposerState()
  }

  async function start(): Promise<void> {
    if (mainWindow) return
    mainWindow = getCurrentWindow()
    overlayWindow = await Window.getByLabel(CHAT_OVERLAY_WINDOW_LABEL)
    if (!overlayWindow) throw new Error('独立中文侧栏窗口未创建')
    await overlayWindow.setFocusable(false)
    await overlayWindow.setSkipTaskbar(true)
    await overlayWindow.setAlwaysOnTop(true)
    await overlayWindow.hide()
    unlisteners.push(
      await mainWindow.onFocusChanged(({ payload }) => {
        if (!payload || !isCompact.value || options.busy.value) return
        void queue(() => restoreWindow(false))
      }),
      await overlayWindow.onMoved(({ payload }) => {
        void queue(() => handleOverlayMoved(payload))
      }),
      await overlayWindow.onFocusChanged(({ payload }) => {
        void queue(() => handleOverlayFocusChanged(payload))
      }),
      await listen<GameForegroundEvent>('game-foreground-changed', (event) => {
        void queue(() => handleForeground(event.payload))
      }),
      await listen<GameForegroundEvent>('game-chat-key-prepare', (event) => {
        if (chatKeyPrewarmQueued || chatKeyShowQueued) return
        chatKeyPrewarmQueued = true
        latestForeground = event.payload
        void notifyForegroundChanged(event.payload)
        void queue(async () => {
          try {
            if (isComposerFocused.value && await overlayWindow?.isFocused()) return
            await prewarmComposerForChatKey()
          } finally {
            chatKeyPrewarmQueued = false
          }
        })
      }),
      await listen<GameForegroundEvent>('game-chat-key-released', (event) => {
        if (chatKeyShowQueued) return
        chatKeyShowQueued = true
        latestForeground = event.payload
        void notifyForegroundChanged(event.payload)
        void queue(async () => {
          try {
            if (isComposerFocused.value && await overlayWindow?.isFocused()) return
            isComposerFocused.value = false
            await showComposerForChatKey()
          } finally {
            resetChatKeyPrewarm()
            chatKeyShowQueued = false
          }
        })
      }),
      await listen<string>('caps-protection-failed', (event) => {
        options.onCapsProtectionFailure?.(event.payload)
      }),
      await listen<string>(CHAT_OVERLAY_INPUT_EVENT, (event) => {
        options.onComposerInput(event.payload)
      }),
      await listen<ChatOverlayMode>(CHAT_OVERLAY_MODE_EVENT, (event) => {
        options.onComposerModeChanged(event.payload)
      }),
      await listen<ChatOverlayPosition>(CHAT_OVERLAY_POSITION_EVENT, (event) => {
        const position = event.payload
        localStorage.setItem(OVERLAY_POSITION_KEY, JSON.stringify(position))
        diagnostic('overlay_position', `stage=drag_complete x=${position.x} y=${position.y}`)
      }),
      await listen<ChatOverlayAction>(CHAT_OVERLAY_ACTION_EVENT, (event) => {
        void Promise.resolve(options.onComposerAction(event.payload)).catch((error) => {
          options.onError(error instanceof Error ? error.message : String(error))
        })
      }),
      await listen<ChatOverlayFocusResult>(CHAT_OVERLAY_FOCUS_RESULT_EVENT, (event) => {
        completeFocusRequest(event.payload)
      }),
    )
  }

  async function dispose(): Promise<void> {
    chatKeyShowQueued = false
    if (pendingFocus) {
      window.clearTimeout(pendingFocus.timeoutId)
      pendingFocus.resolve({
        requestId: pendingFocus.requestId,
        focused: false,
        error: '侧栏窗口已关闭',
      })
      pendingFocus = null
    }
    for (const unlisten of unlisteners.splice(0)) unlisten()
    if (overlayWindow) {
      await overlayWindow.setFocusable(false).catch(() => undefined)
      await overlayWindow.hide().catch(() => undefined)
    }
    overlayWindow = null
    mainWindow = null
    isCompact.value = false
    isComposerFocused.value = false
  }

  watch(options.enabled, (enabled) => {
    if (enabled) return
    void queue(async () => {
      hiddenUntilChatKey = false
      isCompact.value = false
      isComposerFocused.value = false
      if (overlayWindow) {
        await overlayWindow.setFocusable(false)
        await overlayWindow.hide()
      }
    })
  })

  return {
    isCompact: readonly(isCompact),
    isComposerFocused: readonly(isComposerFocused),
    start,
    dispose,
    updateComposerState: () => queue(updateComposerState),
    focusComposer: () => queue(focusComposer),
    deactivateComposer: () => queue(deactivateComposer),
    dismissCompact: () => queueStrict(dismissCompact),
    resumeCompactIfGame: (focus = false) => queue(() => resumeCompactIfGame(focus)),
    expandFull: (focus = true) => queue(() => expandFull(focus)),
    restoreWindow: (focus = true) => queueStrict(() => restoreWindow(focus)),
    yieldWindow: () => queueStrict(yieldWindow),
    startDragging: () => queue(startDragging),
  }
}
