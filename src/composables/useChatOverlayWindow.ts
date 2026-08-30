import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import {
  getCurrentWindow,
  PhysicalPosition,
  PhysicalSize,
  type Window,
} from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { readonly, ref, watch, type Ref } from 'vue'

import type { GameForegroundEvent } from '@/composables/useGameOverlayWindow'
import {
  CHAT_OVERLAY_ACTION_EVENT,
  CHAT_OVERLAY_BLUR_INPUT_EVENT,
  CHAT_OVERLAY_FOCUS_REQUEST_EVENT,
  CHAT_OVERLAY_FOCUS_RESULT_EVENT,
  CHAT_OVERLAY_HEALTH_REQUEST_EVENT,
  CHAT_OVERLAY_HEALTH_RESULT_EVENT,
  CHAT_OVERLAY_INPUT_EVENT,
  CHAT_OVERLAY_MODE_EVENT,
  CHAT_OVERLAY_POSITION_EVENT,
  CHAT_OVERLAY_STATE_EVENT,
  CHAT_OVERLAY_WINDOW_LABEL,
  type ChatOverlayAction,
  type ChatOverlayFocusRequest,
  type ChatOverlayFocusResult,
  type ChatOverlayHealthRequest,
  type ChatOverlayHealthResult,
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
const OVERLAY_HEALTH_TIMEOUT_MS = 600
const OVERLAY_HEALTH_RETRY_COUNT = 3
const OVERLAY_INIT_RETRY_COUNT = 5
const OVERLAY_INIT_RETRY_DELAY_MS = 100
const OVERLAY_CREATE_TIMEOUT_MS = 1_500
const OVERLAY_VISIBILITY_RETRY_COUNT = 3
const OVERLAY_VISIBILITY_RETRY_DELAY_MS = 60
// HD2 opens its own chat UI when the physical chat key is released and the
// engine briefly re-asserts the game window as foreground ~1.1s later. That
// flicker steals the first few keystrokes. Within this guard window after the
// composer took focus, a focus loss is treated as hostile and re-taken with
// the shared retry loop — unless the user clearly left on purpose (the
// foreground moved to a third-party window, or the main assistant window
// took focus). Note the overlay itself foregrounds as "assistant", so that
// state alone must not skip the guard: the flicker happens while the monitor
// still reports assistant.
const OVERLAY_FOCUS_GUARD_WINDOW_MS = 1_500
// Foreground events come from a 250ms monitor poll, so the guard cannot trust
// its snapshot alone to tell "HD2 stole the composer focus" from "the user
// Alt+Tabbed": a switch may still be in flight when the loss event arrives.
// The settle loop therefore polls intent signals every GUARD_SETTLE_POLL_MS
// AND probes the OS for the actual foreground owner: a probed game owner
// starts the refocus loop immediately (every extra settled millisecond is a
// keystroke landing in the game chat), a probed third-party owner stands the
// guard down, and an unavailable probe degrades to the plain timed settle.
// A flicker that recovers natively still takes the fast path within one tick.
const GUARD_SETTLE_MS = 120
const GUARD_SETTLE_POLL_MS = 40
const FOREGROUND_PROBE_COMMAND = 'probe_foreground_state'
// The probe rides the guard's hot path: a normal round-trip is single-digit
// milliseconds, so a probe that has not answered within one settle poll tick
// can no longer help the decision — drop it and fall back to the timed
// settle. Timed-out, failed, and slow probes are logged: field logs must
// show WHY a guard event ran in snapshot-only mode instead of hiding the
// degraded path.
const FOREGROUND_PROBE_TIMEOUT_MS = 40
const FOREGROUND_PROBE_SLOW_MS = 15

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
  let overlayNeedsRecreate = false
  let visibilityCheckCompatibilityMode = false
  let focusGuardUntil = 0
  let composerFocusedAt = 0
  let focusRequestId = 0
  let healthRequestId = 0
  let pendingFocus:
    | {
      requestId: number
      resolve: (result: ChatOverlayFocusResult) => void
      timeoutId: number
    }
    | null = null
  let pendingHealth:
    | {
      requestId: number
      resolve: (result: ChatOverlayHealthResult) => void
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

  function delay(ms: number): Promise<void> {
    return new Promise((resolve) => window.setTimeout(resolve, ms))
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

  // Tell the overlay page to release DOM focus (blur the input + reset IME
  // composition) before the native window is hidden. Without this the IME
  // keeps routing keystrokes to the now-invisible input — the "虚空打字"
  // symptom where WASD ends up in the hidden composer after sending.
  async function blurComposerInput(): Promise<void> {
    if (!overlayWindow) return
    try {
      await emitTo(CHAT_OVERLAY_WINDOW_LABEL, CHAT_OVERLAY_BLUR_INPUT_EVENT)
    } catch (error) {
      diagnostic('overlay_focus', `stage=blur_input_failed error=${error instanceof Error ? error.message : String(error)}`)
    }
  }

  async function waitForCreatedWindow(createdWindow: WebviewWindow): Promise<void> {
    await new Promise<void>((resolve, reject) => {
      let settled = false
      const finish = (error?: unknown) => {
        if (settled) return
        settled = true
        window.clearTimeout(timeoutId)
        if (error) reject(error instanceof Error ? error : new Error(String(error)))
        else resolve()
      }
      const timeoutId = window.setTimeout(() => {
        finish(new Error('等待侧栏窗口创建超时'))
      }, OVERLAY_CREATE_TIMEOUT_MS)
      void createdWindow.once('tauri://created', () => {
        diagnostic('chat_overlay_init', 'stage=create_event result=created')
        finish()
      }).catch(finish)
      void createdWindow.once<string>('tauri://error', (event) => {
        diagnostic(
          'chat_overlay_init',
          `stage=create_event result=error message=${typeof event.payload === 'string' ? event.payload : 'unknown'}`,
        )
        finish(new Error(typeof event.payload === 'string' ? event.payload : '侧栏窗口创建失败'))
      }).catch(finish)
    })
  }

  async function getOrCreateOverlayWindow(): Promise<Window> {
    for (let attempt = 0; attempt < OVERLAY_INIT_RETRY_COUNT; attempt += 1) {
      const existing = await WebviewWindow.getByLabel(CHAT_OVERLAY_WINDOW_LABEL)
      diagnostic(
        'chat_overlay_init',
        `stage=get_by_label attempt=${attempt + 1} found=${existing !== null}`,
      )
      if (existing) {
        return existing
      }
      if (attempt + 1 < OVERLAY_INIT_RETRY_COUNT) await delay(OVERLAY_INIT_RETRY_DELAY_MS)
    }

    diagnostic('chat_overlay_init', 'stage=create start')
    const created = new WebviewWindow(CHAT_OVERLAY_WINDOW_LABEL, {
      url: 'index.html?chat-overlay',
      title: 'HD2CN 中文侧栏',
      width: OVERLAY_WIDTH,
      height: OVERLAY_HEIGHT,
      resizable: false,
      decorations: false,
      transparent: false,
      alwaysOnTop: true,
      visible: false,
      focus: false,
      focusable: true,
      skipTaskbar: true,
      shadow: false,
      closable: false,
    })
    try {
      await waitForCreatedWindow(created)
      diagnostic('chat_overlay_init', 'stage=create result=success')
      return created
    } catch (error) {
      // Compatibility fallback for Tauri's occasional lost create event: the
      // native window may exist even when the event listener timed out.
      diagnostic(
        'chat_overlay_init',
        `stage=compatibility_fallback mode=requery reason=${error instanceof Error ? error.message : String(error)}`,
      )
      const recovered = await WebviewWindow.getByLabel(CHAT_OVERLAY_WINDOW_LABEL)
      if (recovered) {
        diagnostic('chat_overlay_init', 'stage=compatibility_fallback result=recovered_existing')
        return recovered
      }
      throw error
    }
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

  async function requestOverlayHealth(): Promise<ChatOverlayHealthResult> {
    if (pendingHealth) {
      window.clearTimeout(pendingHealth.timeoutId)
      pendingHealth.resolve({
        requestId: pendingHealth.requestId,
        documentReady: false,
        inputReady: false,
        focused: false,
        error: '新的侧栏健康检查请求已替代旧请求',
      })
      pendingHealth = null
    }
    const requestId = healthRequestId + 1
    healthRequestId = requestId
    const result = new Promise<ChatOverlayHealthResult>((resolve) => {
      const timeoutId = window.setTimeout(() => {
        if (!pendingHealth || pendingHealth.requestId !== requestId) return
        pendingHealth = null
        resolve({
          requestId,
          documentReady: false,
          inputReady: false,
          focused: false,
          error: '等待侧栏页面健康检查超时',
        })
      }, OVERLAY_HEALTH_TIMEOUT_MS)
      pendingHealth = { requestId, resolve, timeoutId }
    })
    const request: ChatOverlayHealthRequest = { requestId }
    await emitTo(CHAT_OVERLAY_WINDOW_LABEL, CHAT_OVERLAY_HEALTH_REQUEST_EVENT, request)
    return result
  }

  async function checkOverlayHealth(): Promise<boolean> {
    diagnostic('chat_overlay_health', 'stage=start window=chat-overlay')
    let lastResult: ChatOverlayHealthResult | null = null
    for (let attempt = 0; attempt < OVERLAY_HEALTH_RETRY_COUNT; attempt += 1) {
      const result = await requestOverlayHealth()
      lastResult = result
      diagnostic(
        'chat_overlay_health',
        `stage=verify attempt=${attempt + 1} document_ready=${result.documentReady} input_ready=${result.inputReady} focused=${result.focused} error=${result.error ?? 'none'}`,
      )
      if (result.documentReady && result.inputReady) {
        diagnostic('chat_overlay_health', `stage=complete attempt=${attempt + 1}`)
        return true
      }
      if (attempt + 1 < OVERLAY_HEALTH_RETRY_COUNT) await delay(100)
    }
    diagnostic(
      'chat_overlay_health',
      `stage=failed document_ready=${lastResult?.documentReady ?? false} input_ready=${lastResult?.inputReady ?? false}`,
    )
    return false
  }

  async function verifyOverlayVisibility(): Promise<boolean> {
    if (visibilityCheckCompatibilityMode) {
      diagnostic('overlay_show', 'stage=verify_visible mode=compatibility_fallback reason=permission_cached')
      return true
    }
    try {
      return await overlayWindow!.isVisible()
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (!message.includes('core:window:allow-is-visible')) throw error
      visibilityCheckCompatibilityMode = true
      diagnostic(
        'overlay_show',
        `stage=verify_visible mode=compatibility_fallback reason=is_visible_permission message=${message}`,
      )
      return true
    }
  }

  async function placeAndShowOverlay(payload: GameForegroundEvent): Promise<void> {
    if (!overlayWindow || payload.state !== 'game' || !payload.workArea) {
      throw new Error('无法确定 HD2 所在显示器的可用区域')
    }
    if (overlayNeedsRecreate) {
      diagnostic('chat_overlay_health', 'stage=retry_before_show')
      const healthy = await checkOverlayHealth()
      if (!healthy) throw new Error('独立中文侧栏页面尚未准备好，请稍后再次按聊天键重试')
      overlayNeedsRecreate = false
      diagnostic('chat_overlay_health', 'stage=retry_before_show result=passed')
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
      for (let attempt = 0; attempt < OVERLAY_VISIBILITY_RETRY_COUNT; attempt += 1) {
        diagnostic('overlay_show', `stage=show attempt=${attempt + 1}`)
        await overlayWindow!.show()
        if (await overlayWindow!.isMinimized()) await overlayWindow!.unminimize()
        const visible = await verifyOverlayVisibility()
        const minimized = await overlayWindow!.isMinimized()
        diagnostic(
          'overlay_show',
          `stage=verify_visible attempt=${attempt + 1} visible=${visible} minimized=${minimized}`,
        )
        if (visible && !minimized) break
        if (attempt + 1 === OVERLAY_VISIBILITY_RETRY_COUNT) {
          throw new Error('独立中文侧栏显示后未通过可见性检查')
        }
        await delay(OVERLAY_VISIBILITY_RETRY_DELAY_MS)
      }
    })
    await rememberProgrammaticGeometry()
    isCompact.value = true
    await sendComposerState()
  }

  // Shared focus retry loop for the initial composer focus and the flicker
  // guard, so retry count / delays / logging cannot drift apart.
  // `shouldAbort` (when provided) is polled before every retry attempt so a
  // user who leaves mid-retry (third-party window or the main assistant
  // window — both invisible to foregroundIsOther alone) is not dragged back.
  async function setFocusWithRetries(
    stage: string,
    shouldAbort?: () => Promise<boolean> | boolean,
  ): Promise<{ focused: boolean; lastError: unknown }> {
    let lastError: unknown = null
    for (let attempt = 0; attempt < WINDOW_FOCUS_RETRY_COUNT; attempt += 1) {
      if (attempt > 0) {
        await new Promise<void>((resolve) => window.setTimeout(resolve, WINDOW_FOCUS_RETRY_DELAY_MS))
        // A switch can land mid-retry (the foreground event only needs to
        // beat a 40ms gap); stop pulling the user back.
        if (foregroundIsOther() || (await shouldAbort?.())) {
          diagnostic('overlay_focus', `stage=${stage} result=user_left_during_retry`)
          return { focused: false, lastError }
        }
      }
      try {
        await overlayWindow?.setFocus()
        const focused = await overlayWindow?.isFocused()
        diagnostic('overlay_focus', `stage=${stage} attempt=${attempt + 1} focused=${focused ?? false}`)
        if (focused) {
          return { focused: true, lastError: null }
        }
      } catch (error) {
        lastError = error
      }
    }
    return { focused: false, lastError }
  }

  async function focusComposer(): Promise<void> {
    if (!overlayWindow || !isCompact.value || suspended || options.busy.value) return
    // The focus-regained event can re-enter here right after the guard stood
    // down for a third-party switch; do not pull the user back.
    if (foregroundIsOther()) {
      diagnostic('overlay_focus', 'stage=skip reason=foreground_other')
      return
    }
    const startedAt = performance.now()
    diagnostic('overlay_focus', 'stage=start window=chat-overlay')
    await overlayWindow.setFocusable(true)
    await overlayWindow.setAlwaysOnTop(true)
    isComposerFocused.value = false
    const { focused: windowFocused, lastError } = await setFocusWithRetries('set_focus')
    if (!windowFocused) {
      // Recovery attempt before giving up: a hide/show cycle resets the
      // window's foreground-lock candidacy — the classic fix when Windows
      // refuses SetForegroundWindow after a game took the foreground.
      diagnostic('overlay_focus', 'stage=focus_reset hide_show begin')
      try {
        await overlayWindow.hide()
        await overlayWindow.show()
        const { focused: refocusedAfterReset } = await setFocusWithRetries('set_focus_reset')
        if (refocusedAfterReset) {
          diagnostic('overlay_focus', 'stage=focus_reset result=recovered')
          await sendComposerState()
          const resetDomFocus = await requestDomFocus()
          if (resetDomFocus.focused && !foregroundIsOther()) {
            await options.onComposerFocused()
            isComposerFocused.value = true
            composerFocusedAt = performance.now()
            focusGuardUntil = performance.now() + OVERLAY_FOCUS_GUARD_WINDOW_MS
            diagnostic('overlay_focus', 'stage=complete after_reset')
            return
          }
          diagnostic('overlay_focus', 'stage=focus_reset result=dom_not_ready')
        } else {
          diagnostic('overlay_focus', 'stage=focus_reset result=still_unfocused')
        }
      } catch (error) {
        diagnostic(
          'overlay_focus',
          `stage=focus_reset result=failed error=${error instanceof Error ? error.message : String(error)}`,
        )
      }
      // Native state snapshot for the failure path: 03.log showed three dead
      // setFocus attempts with zero diagnostics. Visibility + main-window
      // focus tell us whether the overlay is fighting a foreground lock.
      let visible = 'unknown'
      let minimized = 'unknown'
      try {
        visible = String(await overlayWindow.isVisible())
        minimized = String(await overlayWindow.isMinimized())
      } catch {
        // native queries can fail alongside focus; keep placeholders
      }
      const mainFocused = await mainWindow?.isFocused().catch(() => false)
      diagnostic(
        'overlay_focus',
        `stage=native_focus_failed visible=${visible} minimized=${minimized} main_focused=${mainFocused ?? 'unknown'} fg=${latestForeground.state}`,
      )
      const detail = lastError instanceof Error ? `：${lastError.message}` : ''
      // Do not assert elevation mismatch as THE cause (03.log/#6 shows
      // integrity=high still failing); list it as one possibility and point
      // at the main composer, which keeps the draft as a working fallback.
      throw new Error(
        `Windows 未将键盘焦点交给独立中文侧栏${detail}。可尝试：让助手与游戏以相同权限运行，或在主界面输入（文字会保留）`,
      )
    }
    await sendComposerState()
    const domFocus = await requestDomFocus()
    diagnostic(
      'overlay_focus',
      `stage=verify_composer focused=${domFocus.focused} elapsed_ms=${Math.round(performance.now() - startedAt)}`,
    )
    if (!domFocus.focused) throw new Error(domFocus.error ?? '中文侧栏输入框没有取得键盘焦点')
    // The DOM round-trip can outlast a just-landed Alt+Tab (the foreground
    // event only needs to beat a 250ms poll). Do not mark the composer as
    // focused when the user has already left.
    if (foregroundIsOther()) {
      diagnostic('overlay_focus', 'stage=skip_after_dom reason=foreground_other')
      return
    }
    await options.onComposerFocused()
    isComposerFocused.value = true
    composerFocusedAt = performance.now()
    focusGuardUntil = performance.now() + OVERLAY_FOCUS_GUARD_WINDOW_MS
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
    composerFocusedAt = 0
    focusGuardUntil = 0
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
    // Note: no snapshot write here. The listener updates latestForeground
    // synchronously on arrival; writing it again from this queued handler
    // would let an older queued event overwrite a newer synchronous snapshot
    // (the focus guard reads the snapshot while holding the queue).
    await notifyForegroundChanged(payload)
    if (payload.state !== 'game') await cancelChatKeyPrewarm()
    if (payload.state === 'game' && options.enabled.value) void refreshGameDiagnostic()
  }

  // Read through a function on purpose: latestForeground is mutated by event
  // callbacks between awaits, and a direct property read would let TypeScript
  // narrow away the "other" checks below.
  function foregroundIsOther(): boolean {
    return latestForeground.state === 'other'
  }

  // One IPC round-trip asking the backend who owns the OS foreground right
  // now, independent of the monitor's 250ms cadence. Returns null when the
  // backend cannot answer in time (browser preview, IPC failure, stalled
  // webview task, older build), so callers fall back to snapshot-only logic
  // instead of hanging the settle loop on an unanswered probe.
  async function probeForegroundOwner(): Promise<'game' | 'assistant' | 'other' | null> {
    const startedAt = performance.now()
    try {
      const state = await new Promise<string>((resolve, reject) => {
        const timer = window.setTimeout(
          () => reject(new Error('probe_timeout')),
          FOREGROUND_PROBE_TIMEOUT_MS,
        )
        invoke<string>(FOREGROUND_PROBE_COMMAND)
          .then(resolve, reject)
          .finally(() => window.clearTimeout(timer))
      })
      const elapsed = Math.round(performance.now() - startedAt)
      if (state !== 'game' && state !== 'assistant' && state !== 'other') {
        diagnostic('overlay_focus', `stage=guard_probe result=invalid_state elapsed_ms=${elapsed}`)
        return null
      }
      if (elapsed >= FOREGROUND_PROBE_SLOW_MS) {
        diagnostic('overlay_focus', `stage=guard_probe result=slow elapsed_ms=${elapsed}`)
      }
      return state
    } catch (error) {
      const elapsed = Math.round(performance.now() - startedAt)
      const kind = error instanceof Error && error.message === 'probe_timeout' ? 'timeout' : 'error'
      diagnostic('overlay_focus', `stage=guard_probe result=${kind} elapsed_ms=${elapsed}`)
      return null
    }
  }

  async function handleOverlayFocusChanged(focused: boolean): Promise<void> {
    if (!isCompact.value || options.busy.value) return
    if (!focused) {
      isComposerFocused.value = false
      // The guard must never fire while the overlay is intentionally hidden
      // (post-submit dismiss, yield): a late focus event would otherwise
      // setFocus() a hidden window and resurrect it.
      const guardActive =
        !hiddenUntilChatKey && !suspended && performance.now() < focusGuardUntil
      if (guardActive) {
        // Field diagnostics for the swallowed-first-keys reports: when the
        // loss happened relative to the composer taking focus, and who the
        // OS says owns the foreground right now. This is what distinguishes
        // "keys eaten during the open sequence" from "keys eaten by the
        // ~1.1s HD2 flicker" (and explains anomalies like standing down on
        // main_window_focused mid-gameplay) without logging any key values.
        const lossOwner = await probeForegroundOwner()
        diagnostic(
          'overlay_focus',
          `stage=focus_loss elapsed_since_open_ms=${
            composerFocusedAt ? Math.round(performance.now() - composerFocusedAt) : 'null'
          } owner=${lossOwner ?? 'unknown'}`,
        )
        // The flicker often self-recovers by the time the loss event clears
        // the queue — native recovery keeps the fast path, checked first.
        // Beyond that, the monitor snapshot alone cannot decide hostile vs.
        // intent within milliseconds (it lags reality by up to a full 250ms
        // poll), so every tick also asks the OS directly who owns the
        // foreground: a probed game owner starts the refocus loop below
        // immediately instead of waiting out the settle window — while the
        // game provably holds the foreground, settling is keystrokes landing
        // in the game chat. A probed third-party owner stands the guard down,
        // and an unavailable probe degrades to the plain timed settle.
        const settleDeadline = performance.now() + GUARD_SETTLE_MS
        let nativeFocusBack = false
        let probeSawGame = false
        while (performance.now() < settleDeadline) {
          if (!isCompact.value || options.busy.value) return
          if (foregroundIsOther()) {
            diagnostic('overlay_focus', 'stage=guard_skip reason=foreground_other')
            return
          }
          if (await mainWindow?.isFocused()) {
            diagnostic('overlay_focus', 'stage=guard_skip reason=main_window_focused')
            return
          }
          if (await overlayWindow?.isFocused()) {
            nativeFocusBack = true
            break
          }
          const owner = await probeForegroundOwner()
          if (owner === 'game') {
            // Probed foreground is the HD2 window itself: this is exactly the
            // first-keystroke swallowing case — stop waiting, act.
            probeSawGame = true
            break
          }
          if (owner === 'other') {
            diagnostic('overlay_focus', 'stage=guard_skip reason=probed_foreground_other')
            return
          }
          await delay(GUARD_SETTLE_POLL_MS)
        }
        if (nativeFocusBack) {
          diagnostic('overlay_focus', 'stage=guard_skip_settle reason=native_focus_already_back')
          const domFocus = await requestDomFocus()
          const userLeft = foregroundIsOther() || (await mainWindow?.isFocused())
          if (!domFocus.focused) {
            // DOM focus failed while the user is still here: that is a broken
            // composer, not an intentional switch — collapse to the full
            // assistant with an error instead of leaving a dead overlay.
            if (userLeft) {
              diagnostic('overlay_focus', 'stage=guard_skip_settle result=user_left')
              return
            }
            const message = domFocus.error ?? '侧栏输入框焦点复核失败'
            diagnostic('overlay_focus', `stage=guard_skip_settle result=dom_failed error=${message}`)
            await recoverFromTransitionFailure(true)
            options.onError(message)
            return
          }
          if (userLeft) {
            diagnostic('overlay_focus', 'stage=guard_skip_settle result=user_left')
            return
          }
          try {
            await options.onComposerFocused()
          } catch (error) {
            const message = error instanceof Error ? error.message : String(error)
            diagnostic(
              'overlay_focus',
              `stage=guard_skip_settle result=composer_callback_failed error=${message}`,
            )
            await recoverFromTransitionFailure(true)
            options.onError(message)
            return
          }
          isComposerFocused.value = true
          // A second flicker can follow the first (the game re-asserts
          // itself in bursts); restart the guard window from this recovery.
          composerFocusedAt = performance.now()
          focusGuardUntil = performance.now() + OVERLAY_FOCUS_GUARD_WINDOW_MS
          diagnostic('overlay_focus', 'stage=guard_skip_settle result=recovered')
          return
        }
        // Native focus did not come back within the settle window (or the
        // probe confirmed the game still owns the foreground): proceed to the
        // retry loop, re-checking intent before every attempt.
        if (!isCompact.value || options.busy.value) return
        // The settle wait may itself cross the window's edge; do not act on a
        // guard that has already expired.
        if (performance.now() >= focusGuardUntil) {
          diagnostic('overlay_focus', 'stage=guard_skip reason=window_expired_during_settle')
          return
        }
        // Focus flicker while the guard window is open: HD2 just re-asserted
        // its chat UI. Re-take the focus with the same retry loop the initial
        // focus uses. The main window reports the same "assistant" foreground
        // state as the overlay, so it is re-checked natively before every
        // retry — a user clicking it mid-retry must not be dragged back, and
        // the final check could already be poisoned by our own setFocus.
        // If every retry fails the window state is genuinely uncertain:
        // collapse to the full assistant (draft preserved by the app) instead
        // of leaving a compact overlay that keeps losing keystrokes to the game.
        diagnostic(
          'overlay_focus',
          `stage=guard_refocus begin trigger=${
            probeSawGame ? 'probe_game' : 'settle_timeout'
          } fg=${latestForeground.state} remaining_ms=${Math.max(0, Math.round(focusGuardUntil - performance.now()))}`,
        )
        const guardAbort = async () => (await mainWindow?.isFocused()) === true
        const { focused: refocused, lastError } = await setFocusWithRetries(
          'guard_refocus',
          guardAbort,
        )
        if (!refocused) {
          // If the user left while the retries ran, stand down quietly: no
          // error notice, no main-window restore — the loss was intentional.
          if (foregroundIsOther() || (await mainWindow?.isFocused())) {
            diagnostic('overlay_focus', 'stage=guard_refocus result=user_left_during_retry')
            return
          }
          const detail = lastError instanceof Error ? `：${lastError.message}` : ''
          const message = `侧栏焦点被游戏抢占后未能恢复${detail}`
          diagnostic('overlay_focus', `stage=guard_refocus result=failed error=${message}`)
          await recoverFromTransitionFailure(true)
          options.onError(message)
          return
        }
        // The retries may have raced an Alt+Tab: the third-party window can
        // take the foreground right after our last successful setFocus.
        if (foregroundIsOther()) {
          diagnostic('overlay_focus', 'stage=guard_refocus result=user_left_after_retry')
          return
        }
        const domFocus = await requestDomFocus()
        if (!domFocus.focused) {
          // The user may have completed a switch during the round-trip; do
          // not yank the main window up over their new foreground.
          if (foregroundIsOther() || (await mainWindow?.isFocused())) {
            diagnostic('overlay_focus', 'stage=guard_refocus result=user_left_after_dom_focus')
            return
          }
          const message = domFocus.error ?? '侧栏输入框焦点复核失败'
          diagnostic('overlay_focus', `stage=guard_refocus result=dom_refocus_failed error=${message}`)
          await recoverFromTransitionFailure(true)
          options.onError(message)
          return
        }
        // Re-check after the DOM-focus round-trip too: that round-trip spans
        // two windows and can be where the Alt+Tab lands.
        if (foregroundIsOther()) {
          diagnostic('overlay_focus', 'stage=guard_refocus result=user_left_after_dom_focus')
          return
        }
        // The user may have clicked the main window while the retries ran.
        if (await mainWindow?.isFocused()) {
          diagnostic('overlay_focus', 'stage=guard_refocus result=superseded_by_main_window')
          return
        }
        try {
          await options.onComposerFocused()
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          diagnostic(
            'overlay_focus',
            `stage=guard_refocus result=composer_callback_failed error=${message}`,
          )
          await recoverFromTransitionFailure(true)
          options.onError(message)
          return
        }
        isComposerFocused.value = true
        // A second flicker can follow the first (the game re-asserts itself
        // in bursts); restart the guard window from this recovery.
        composerFocusedAt = performance.now()
        focusGuardUntil = performance.now() + OVERLAY_FOCUS_GUARD_WINDOW_MS
        diagnostic('overlay_focus', 'stage=guard_refocus result=recovered')
        return
      }
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
    composerFocusedAt = 0
    focusGuardUntil = 0
    suspended = true
    hiddenUntilChatKey = false
    isComposerFocused.value = false
    if (overlayWindow) {
      await overlayWindow.setFocusable(false)
      await blurComposerInput()
      await overlayWindow.hide()
    }
    await mainWindow.setFocusable(false)
    await mainWindow.setSkipTaskbar(true)
    await mainWindow.hide()
  }

  async function dismissCompact(): Promise<void> {
    if (!overlayWindow || !isCompact.value) return
    resetChatKeyPrewarm()
    composerFocusedAt = 0
    focusGuardUntil = 0
    diagnostic('overlay_dismiss', 'stage=start window=chat-overlay')
    hiddenUntilChatKey = true
    isComposerFocused.value = false
    await overlayWindow.setFocusable(false)
    await blurComposerInput()
    await overlayWindow.setSkipTaskbar(true)
    await overlayWindow.hide()
    diagnostic('overlay_dismiss', 'stage=hidden window=chat-overlay')
  }

  async function restoreWindow(focus = true): Promise<void> {
    resetChatKeyPrewarm()
    composerFocusedAt = 0
    focusGuardUntil = 0
    suspended = false
    hiddenUntilChatKey = false
    isComposerFocused.value = false
    isCompact.value = false
    if (overlayWindow) {
      await overlayWindow.setFocusable(false)
      await blurComposerInput()
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
    await blurComposerInput()
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
    diagnostic('chat_overlay_init', 'stage=start')
    mainWindow = getCurrentWindow()
    overlayWindow = await getOrCreateOverlayWindow()
    diagnostic('chat_overlay_init', 'stage=window_ready label=chat-overlay')
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
        // Snapshot update must be synchronous: the focus guard reads it while
        // it holds the transition queue, so a queued update would only land
        // after the guard finished and would never be seen.
        latestForeground = event.payload
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
      await listen<ChatOverlayHealthResult>(CHAT_OVERLAY_HEALTH_RESULT_EVENT, (event) => {
        if (!pendingHealth || pendingHealth.requestId !== event.payload.requestId) return
        window.clearTimeout(pendingHealth.timeoutId)
        const resolve = pendingHealth.resolve
        pendingHealth = null
        resolve(event.payload)
      }),
    )
    const healthy = await checkOverlayHealth()
    if (!healthy) {
      diagnostic(
        'chat_overlay_init',
        'stage=degraded action=keep_window',
      )
      overlayNeedsRecreate = true
      options.onError('独立中文侧栏尚未完成加载；已保留窗口，稍后按聊天键会自动重试。')
    }
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
    if (pendingHealth) {
      window.clearTimeout(pendingHealth.timeoutId)
      pendingHealth.resolve({
        requestId: pendingHealth.requestId,
        documentReady: false,
        inputReady: false,
        focused: false,
        error: '侧栏窗口已关闭',
      })
      pendingHealth = null
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
