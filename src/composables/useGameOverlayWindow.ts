import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  getCurrentWindow,
  PhysicalPosition,
  PhysicalSize,
  type Window,
} from '@tauri-apps/api/window'
import { nextTick, readonly, ref, watch, type Ref } from 'vue'

export interface GameForegroundEvent {
  state: 'game' | 'assistant' | 'other'
  workArea: { x: number; y: number; width: number; height: number } | null
  scaleFactor: number | null
}

type WindowGeometry = {
  position: PhysicalPosition
  size: PhysicalSize
  maximized: boolean
}

type ProgrammaticGeometry = {
  position: PhysicalPosition
  size: PhysicalSize
  expiresAt: number
}

type OverlayOptions = {
  enabled: Ref<boolean>
  busy: Ref<boolean>
  compactHeight?: Ref<number>
  onGameForeground: () => void | Promise<void>
  onForegroundChanged?: (event: GameForegroundEvent) => void | Promise<void>
  onComposerFocused: () => void | Promise<void>
  onDiagnostic?: (stage: string, message: string) => void | Promise<void>
  onCapsProtectionFailure?: (message: string) => void
  onError: (message: string) => void
}

const MAIN_MIN_WIDTH = 720
const MAIN_MIN_HEIGHT = 560
const MAIN_DEFAULT_WIDTH = 980
const MAIN_DEFAULT_HEIGHT = 680
const MINIMIZED_POSITION_THRESHOLD = -30_000
const COMPACT_WIDTH = 500
const COMPACT_HEIGHT = 58
const COMPACT_RIGHT_MARGIN = 20
const COMPACT_VERTICAL_RATIO = 0.5
const COMPACT_POSITION_KEY = 'hd2cn.overlay.position.v1'
const GEOMETRY_EVENT_SETTLE_MS = 250
const COMPACT_SIZE_RETRY_COUNT = 3
const COMPOSER_FOCUS_RETRY_COUNT = 3
const COMPOSER_FOCUS_RETRY_DELAY_MS = 40

export function useGameOverlayWindow(options: OverlayOptions) {
  const isCompact = ref(false)
  const isComposerFocused = ref(false)
  const compactHeight = options.compactHeight ?? ref(COMPACT_HEIGHT)
  let appWindow: Window | null = null
  let mainGeometry: WindowGeometry | null = null
  let latestForeground: GameForegroundEvent = {
    state: 'assistant',
    workArea: null,
    scaleFactor: null,
  }
  let suspended = false
  let hiddenUntilChatKey = false
  let hiddenByYield = false
  let trackingCompactDrag = false
  let compactRestorePosition: PhysicalPosition | null = null
  let compactRestoreWorkArea: GameForegroundEvent['workArea'] = null
  let diagnosticRefreshInFlight = false
  let chatKeyShowQueued = false
  let geometryMutationDepth = 0
  let ignoreGeometryEventsUntil = 0
  let programmaticGeometryHistory: ProgrammaticGeometry[] = []
  let transition = Promise.resolve()
  const unlisteners: UnlistenFn[] = []

  function diagnostic(stage: string, message: string): void {
    void Promise.resolve(options.onDiagnostic?.(stage, message)).catch(() => undefined)
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

  async function notifyForegroundChanged(payload: GameForegroundEvent): Promise<void> {
    try {
      await options.onForegroundChanged?.(payload)
    } catch (error) {
      options.onError(error instanceof Error ? error.message : String(error))
    }
  }

  function savedCompactPosition(
    workArea: GameForegroundEvent['workArea'],
    width: number,
    height: number,
  ): PhysicalPosition | null {
    if (!workArea) return null
    try {
      const value = JSON.parse(localStorage.getItem(COMPACT_POSITION_KEY) ?? '') as {
        x?: unknown
        y?: unknown
      }
      if (typeof value.x !== 'number' || typeof value.y !== 'number') return null
      return new PhysicalPosition(
        Math.min(Math.max(value.x, workArea.x), workArea.x + workArea.width - width),
        Math.min(Math.max(value.y, workArea.y), workArea.y + workArea.height - height),
      )
    } catch {
      return null
    }
  }

  async function rememberMainGeometry(window: Window): Promise<void> {
    if (mainGeometry) return
    if (await window.isMinimized()) return
    const maximized = await window.isMaximized()
    if (maximized) await window.unmaximize()
    const position = await window.outerPosition()
    if (isMinimizedShellPosition(position)) return
    mainGeometry = {
      position,
      size: await window.outerSize(),
      maximized,
    }
  }

  async function withProgrammaticGeometry<T>(operation: () => Promise<T>): Promise<T> {
    geometryMutationDepth += 1
    try {
      return await operation()
    } finally {
      geometryMutationDepth -= 1
      ignoreGeometryEventsUntil = Math.max(
        ignoreGeometryEventsUntil,
        Date.now() + GEOMETRY_EVENT_SETTLE_MS,
      )
    }
  }

  function shouldIgnoreGeometryEvent(): boolean {
    return geometryMutationDepth > 0 || Date.now() < ignoreGeometryEventsUntil
  }

  async function ensureCompactNativeSize(
    window: Window,
    width: number,
    height: number,
  ): Promise<void> {
    let actual = await window.outerSize()
    for (let attempt = 0; attempt < COMPACT_SIZE_RETRY_COUNT; attempt += 1) {
      if (actual.width === width && actual.height === height) return
      await window.setSize(new PhysicalSize(width, height))
      actual = await window.outerSize()
    }
    throw new Error(
      `紧凑输入栏尺寸校验失败：期望 ${width}x${height}，实际 ${actual.width}x${actual.height}`,
    )
  }

  function compactSizeConstraints(
    width: number,
    height: number,
  ): { minWidth: number; minHeight: number; maxWidth: number; maxHeight: number } {
    return {
      minWidth: width,
      minHeight: height,
      maxWidth: width,
      maxHeight: height,
    }
  }

  async function rememberProgrammaticGeometry(window: Window): Promise<void> {
    const position = await window.outerPosition()
    const size = await window.outerSize()
    const now = Date.now()
    programmaticGeometryHistory = [
      { position: copyPosition(position), size: new PhysicalSize(size.width, size.height), expiresAt: now + 2_000 },
      ...programmaticGeometryHistory.filter((entry) => entry.expiresAt > now),
    ].slice(0, 6)
  }

  function isProgrammaticGeometryEvent(
    position: PhysicalPosition,
    size?: PhysicalSize,
  ): boolean {
    const now = Date.now()
    programmaticGeometryHistory = programmaticGeometryHistory.filter((entry) => entry.expiresAt > now)
    return programmaticGeometryHistory.some((entry) => {
      const samePosition = entry.position.x === position.x && entry.position.y === position.y
      const sameSize = !size || (entry.size.width === size.width && entry.size.height === size.height)
      return samePosition && sameSize
    })
  }

  async function prepareCompactNativeWindow(
    window: Window,
    width: number,
    height: number,
    position: PhysicalPosition | null,
  ): Promise<void> {
    await withProgrammaticGeometry(async () => {
      if (await window.isMaximized()) await window.unmaximize()
      await window.setFocusable(false)
      await window.setSkipTaskbar(true)
      await window.setAlwaysOnTop(true)
      await window.setResizable(true)
      await window.setSizeConstraints(null)
      await window.setDecorations(false)
      await window.setShadow(false)
      await window.setSize(new PhysicalSize(width, height))
      await ensureCompactNativeSize(window, width, height)
      await window.setSizeConstraints(compactSizeConstraints(width, height))
      await window.setResizable(false)
      await ensureCompactNativeSize(window, width, height)
      if (position) await window.setPosition(position)
    })
  }

  function isMinimizedShellPosition(position: PhysicalPosition): boolean {
    return position.x <= MINIMIZED_POSITION_THRESHOLD || position.y <= MINIMIZED_POSITION_THRESHOLD
  }

  function copyPosition(position: PhysicalPosition): PhysicalPosition {
    return new PhysicalPosition(position.x, position.y)
  }

  function clampMainPosition(
    position: PhysicalPosition,
    workArea: GameForegroundEvent['workArea'],
    size: PhysicalSize,
  ): PhysicalPosition {
    if (!workArea) return copyPosition(position)
    const maxX = workArea.x + Math.max(0, workArea.width - size.width)
    const maxY = workArea.y + Math.max(0, workArea.height - size.height)
    return new PhysicalPosition(
      Math.min(Math.max(position.x, workArea.x), maxX),
      Math.min(Math.max(position.y, workArea.y), maxY),
    )
  }

  function fallbackMainPosition(
    workArea: GameForegroundEvent['workArea'],
    size: PhysicalSize,
  ): PhysicalPosition | null {
    if (!workArea) return null
    return new PhysicalPosition(
      Math.round(workArea.x + Math.max(0, workArea.width - size.width) / 2),
      Math.round(workArea.y + Math.max(0, workArea.height - size.height) / 2),
    )
  }

  async function enterCompactMode(payload: GameForegroundEvent): Promise<void> {
    if (!appWindow || suspended || hiddenUntilChatKey || !options.enabled.value || payload.state !== 'game') return
    trackingCompactDrag = false
    if (payload.workArea) latestForeground = payload
    compactRestoreWorkArea = payload.workArea

    const scale = Math.max(1, payload.scaleFactor ?? 1)
    const width = Math.round(COMPACT_WIDTH * scale)
    const height = Math.round(compactHeight.value * scale)
    const workArea = payload.workArea
    const savedPosition = savedCompactPosition(workArea, width, height)
    const position =
      savedPosition ??
      (workArea
        ? new PhysicalPosition(
          Math.round(workArea.x + workArea.width - width - COMPACT_RIGHT_MARGIN * scale),
          Math.round(workArea.y + (workArea.height - height) * COMPACT_VERTICAL_RATIO),
        )
        : null)
    compactRestorePosition = savedPosition ? copyPosition(savedPosition) : null

    if (!isCompact.value) {
      await rememberMainGeometry(appWindow)
      await prepareCompactNativeWindow(appWindow, width, height, position)
    } else {
      await withProgrammaticGeometry(async () => {
        await appWindow!.setFocusable(false)
        await appWindow!.setSkipTaskbar(true)
        await appWindow!.setAlwaysOnTop(true)
        await appWindow!.setResizable(true)
        await appWindow!.setSizeConstraints(null)
        await appWindow!.setSize(new PhysicalSize(width, height))
        await ensureCompactNativeSize(appWindow!, width, height)
        await appWindow!.setSizeConstraints(compactSizeConstraints(width, height))
        await appWindow!.setResizable(false)
        await ensureCompactNativeSize(appWindow!, width, height)
      })
    }
    await withProgrammaticGeometry(async () => {
      await appWindow!.show()
      if (await appWindow!.isMinimized()) await appWindow!.unminimize()
      await ensureCompactNativeSize(appWindow!, width, height)
    })
    await rememberProgrammaticGeometry(appWindow)
    hiddenByYield = false
    isCompact.value = true
    isComposerFocused.value = false
    await nextTick()
  }

  async function restoreFullMode(focus: boolean): Promise<void> {
    if (!appWindow) return
    const restoreWorkArea = latestForeground.workArea ?? compactRestoreWorkArea
    const restoreFromCompactPosition = isCompact.value && compactRestorePosition
      ? copyPosition(compactRestorePosition)
      : null
    trackingCompactDrag = false
    hiddenUntilChatKey = false
    latestForeground = { state: 'assistant', workArea: null, scaleFactor: null }
    isCompact.value = false
    isComposerFocused.value = false
    await nextTick()
    await withProgrammaticGeometry(async () => {
      await appWindow!.setFocusable(true)
      await appWindow!.setSkipTaskbar(false)
      await appWindow!.setResizable(true)
      await appWindow!.setSizeConstraints(null)
      await appWindow!.setDecorations(true)
      await appWindow!.setShadow(true)
      await appWindow!.show()
      if (await appWindow!.isMinimized()) await appWindow!.unminimize()
      await appWindow!.setAlwaysOnTop(focus)
      const restoreSize = mainGeometry?.size ?? new PhysicalSize(MAIN_DEFAULT_WIDTH, MAIN_DEFAULT_HEIGHT)
      await appWindow!.setSize(restoreSize)
      const restorePosition = restoreFromCompactPosition
        ? clampMainPosition(restoreFromCompactPosition, restoreWorkArea, restoreSize)
        : mainGeometry && !isMinimizedShellPosition(mainGeometry.position)
          ? copyPosition(mainGeometry.position)
          : fallbackMainPosition(restoreWorkArea, restoreSize)
      if (restorePosition) {
        await appWindow!.setPosition(restorePosition)
      }
      if (mainGeometry?.maximized) {
        await appWindow!.maximize()
      }
      await appWindow!.setSizeConstraints({ minWidth: MAIN_MIN_WIDTH, minHeight: MAIN_MIN_HEIGHT })
    })
    await rememberProgrammaticGeometry(appWindow)
    if (focus) await appWindow.setFocus()
    await appWindow.setAlwaysOnTop(false)
  }

  async function handleForeground(payload: GameForegroundEvent): Promise<void> {
    latestForeground = payload
    await notifyForegroundChanged(payload)
    if (payload.state === 'game') {
      if (options.enabled.value) {
        void refreshGameDiagnostic()
      }
      return
    }
  }

  async function handleWindowFocusChanged(focused: boolean): Promise<void> {
    if (!isCompact.value || options.busy.value) return
    if (!focused) {
      isComposerFocused.value = false
      if (await appWindow?.isFocused()) {
        try {
          await nextTick()
          await options.onComposerFocused()
          isComposerFocused.value = true
          diagnostic('overlay_focus', 'stage=stale_focus_loss_reverified focused=true')
        } catch (error) {
          diagnostic(
            'overlay_focus',
            `stage=stale_focus_loss_reverify_failed error=${error instanceof Error ? error.message : String(error)}`,
          )
          throw error
        }
      }
      return
    }
    if (hiddenUntilChatKey) {
      await restoreWindow(true)
    } else if (!isComposerFocused.value) {
      await focusComposer()
    }
  }

  async function handleWindowMoved(
    position: PhysicalPosition,
    ignoreMainGeometry = false,
  ): Promise<void> {
    if (!appWindow || isMinimizedShellPosition(position)) return
    const nextPosition = copyPosition(position)
    if (isCompact.value) {
      if (!trackingCompactDrag) return
      compactRestorePosition = nextPosition
      localStorage.setItem(COMPACT_POSITION_KEY, JSON.stringify({ x: nextPosition.x, y: nextPosition.y }))
      return
    }
    if (ignoreMainGeometry || shouldIgnoreGeometryEvent() || isProgrammaticGeometryEvent(nextPosition)) return
    if (await appWindow.isMinimized()) return
    mainGeometry = {
      position: nextPosition,
      size: await appWindow.outerSize(),
      maximized: await appWindow.isMaximized(),
    }
  }

  async function handleWindowResized(
    size: PhysicalSize,
    ignoreMainGeometry = false,
  ): Promise<void> {
    if (!appWindow || isCompact.value || ignoreMainGeometry || shouldIgnoreGeometryEvent()) return
    if (await appWindow.isMinimized()) return
    const position = await appWindow.outerPosition()
    if (isMinimizedShellPosition(position)) return
    if (isProgrammaticGeometryEvent(position, size)) return
    mainGeometry = {
      position: copyPosition(position),
      size: new PhysicalSize(size.width, size.height),
      maximized: await appWindow.isMaximized(),
    }
  }

  async function focusComposer(): Promise<void> {
    if (!appWindow || !isCompact.value || suspended || options.busy.value) return
    const startedAt = performance.now()
    diagnostic('overlay_focus', 'stage=start')
    await appWindow.setFocusable(true)
    diagnostic('overlay_focus', 'stage=set_focusable focusable=true')
    isComposerFocused.value = false
    let windowFocused = false
    let lastError: unknown = null
    await appWindow.setAlwaysOnTop(true)
    diagnostic('overlay_focus', 'stage=set_always_on_top enabled=true')
    for (let attempt = 0; attempt < COMPOSER_FOCUS_RETRY_COUNT; attempt += 1) {
      try {
        diagnostic('overlay_focus', `stage=set_focus attempt=${attempt + 1}`)
        await appWindow.setFocus()
        const focused = await appWindow.isFocused()
        diagnostic('overlay_focus', `stage=verify_window attempt=${attempt + 1} focused=${focused}`)
        if (focused) {
          windowFocused = true
          break
        }
      } catch (error) {
        lastError = error
        diagnostic(
          'overlay_focus',
          `stage=set_focus_error attempt=${attempt + 1} error=${error instanceof Error ? error.message : String(error)}`,
        )
      }
      if (attempt + 1 < COMPOSER_FOCUS_RETRY_COUNT) {
        await new Promise<void>((resolve) => window.setTimeout(resolve, COMPOSER_FOCUS_RETRY_DELAY_MS))
      }
    }
    if (!windowFocused) {
      const detail = lastError instanceof Error ? `：${lastError.message}` : ''
      diagnostic('overlay_focus', `stage=failed elapsed_ms=${Math.round(performance.now() - startedAt)}`)
      throw new Error(`Windows 未将键盘焦点交给中文侧栏${detail}`)
    }
    await nextTick()
    try {
      await options.onComposerFocused()
      isComposerFocused.value = true
    } catch (error) {
      isComposerFocused.value = false
      diagnostic(
        'overlay_focus',
        `stage=verify_composer_failed elapsed_ms=${Math.round(performance.now() - startedAt)} error=${error instanceof Error ? error.message : String(error)}`,
      )
      throw error
    }
    diagnostic('overlay_focus', `stage=complete elapsed_ms=${Math.round(performance.now() - startedAt)}`)
  }

  async function showComposerForChatKey(): Promise<void> {
    if (!appWindow || !options.enabled.value || options.busy.value || latestForeground.state !== 'game') return
    suspended = false
    hiddenUntilChatKey = false
    try {
      await enterCompactMode(latestForeground)
      await focusComposer()
    } catch (error) {
      await recoverFromTransitionFailure(true)
      throw error
    }
  }

  async function recoverFromTransitionFailure(focus = false): Promise<void> {
    try {
      await restoreFullMode(focus)
    } catch (error) {
      options.onError(`窗口恢复失败：${error instanceof Error ? error.message : String(error)}`)
    }
  }

  async function deactivateComposer(): Promise<void> {
    if (!appWindow || !isCompact.value) return
    isComposerFocused.value = false
    await appWindow.setFocusable(false)
  }

  async function startDragging(): Promise<void> {
    if (!appWindow || !isCompact.value) return
    trackingCompactDrag = true
    try {
      await appWindow.startDragging()
    } catch (error) {
      trackingCompactDrag = false
      throw error
    }
  }

  async function yieldWindow(): Promise<void> {
    if (!appWindow) return
    trackingCompactDrag = false
    suspended = true
    hiddenUntilChatKey = false
    hiddenByYield = true
    isComposerFocused.value = false
    await appWindow.setFocusable(false)
    await appWindow.setSkipTaskbar(true)
    await appWindow.hide()
  }

  async function dismissCompact(): Promise<void> {
    if (!appWindow || !isCompact.value) return
    diagnostic('overlay_dismiss', 'stage=start')
    trackingCompactDrag = false
    hiddenUntilChatKey = true
    isComposerFocused.value = false
    await appWindow.setFocusable(false)
    await appWindow.setSkipTaskbar(true)
    await appWindow.hide()
    diagnostic('overlay_dismiss', 'stage=hidden')
  }

  async function restoreWindow(focus = true): Promise<void> {
    suspended = false
    hiddenUntilChatKey = false
    if (isCompact.value) {
      await restoreFullMode(focus)
      return
    }
    if (!appWindow) return
    const wasHiddenByYield = hiddenByYield
    hiddenByYield = false
    await appWindow.setFocusable(true)
    await appWindow.setSkipTaskbar(false)
    await appWindow.show()
    if (!wasHiddenByYield && await appWindow.isMinimized()) await appWindow.unminimize()
    if (focus) {
      await appWindow.setAlwaysOnTop(true)
      await appWindow.setFocus()
      await appWindow.setAlwaysOnTop(false)
    }
  }

  async function resumeCompactIfGame(focus = false): Promise<void> {
    suspended = false
    hiddenUntilChatKey = false
    if (isCompact.value && latestForeground.state === 'game' && options.enabled.value) {
      try {
        await enterCompactMode(latestForeground)
        if (focus) await focusComposer()
      } catch (error) {
        await recoverFromTransitionFailure()
        throw error
      }
    }
  }

  async function start(): Promise<void> {
    if (appWindow) return
    appWindow = getCurrentWindow()
    unlisteners.push(
      await appWindow.onMoved(({ payload: position }) => {
        const ignoreMainGeometry = shouldIgnoreGeometryEvent()
        void queue(() => handleWindowMoved(position, ignoreMainGeometry))
      }),
      await appWindow.onResized(({ payload: size }) => {
        const ignoreMainGeometry = shouldIgnoreGeometryEvent()
        void queue(() => handleWindowResized(size, ignoreMainGeometry))
      }),
      await appWindow.onFocusChanged(({ payload: focused }) => {
        void queue(() => handleWindowFocusChanged(focused))
      }),
      await listen<GameForegroundEvent>('game-foreground-changed', (event) => {
        void queue(() => handleForeground(event.payload))
      }),
      await listen<GameForegroundEvent>('game-chat-key-released', (event) => {
        if (chatKeyShowQueued) return
        chatKeyShowQueued = true
        latestForeground = event.payload
        void notifyForegroundChanged(event.payload)
        void queue(async () => {
          try {
            if (isComposerFocused.value && await appWindow?.isFocused()) return
            isComposerFocused.value = false
            await showComposerForChatKey()
          } finally {
            chatKeyShowQueued = false
          }
        })
      }),
      await listen<string>('caps-protection-failed', (event) => {
        options.onCapsProtectionFailure?.(event.payload)
      }),
    )
  }

  async function dispose(): Promise<void> {
    for (const unlisten of unlisteners.splice(0)) unlisten()
    suspended = false
    chatKeyShowQueued = false
    if (isCompact.value) await queue(() => restoreFullMode(false))
    appWindow = null
  }

  watch(options.enabled, (enabled) => {
    if (!appWindow) return
    if (!enabled && isCompact.value) {
      void queue(() => restoreFullMode(false))
    }
  })

  return {
    isCompact: readonly(isCompact),
    isComposerFocused: readonly(isComposerFocused),
    start,
    dispose,
    focusComposer: () => queue(focusComposer),
    startDragging: () => queue(startDragging),
    deactivateComposer: () => queue(deactivateComposer),
    dismissCompact: () => queueStrict(dismissCompact),
    expandFull: (focus = true) => queue(() => restoreWindow(focus)),
    yieldWindow: () => queue(yieldWindow),
    restoreWindow: (focus = true) => queue(() => restoreWindow(focus)),
    resumeCompactIfGame: (focus = false) => queue(() => resumeCompactIfGame(focus)),
  }
}
