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

type OverlayOptions = {
  enabled: Ref<boolean>
  busy: Ref<boolean>
  onGameForeground: () => void | Promise<void>
  onComposerFocused: () => void | Promise<void>
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

export function useGameOverlayWindow(options: OverlayOptions) {
  const isCompact = ref(false)
  const isComposerFocused = ref(false)
  let appWindow: Window | null = null
  let mainGeometry: WindowGeometry | null = null
  let latestForeground: GameForegroundEvent = {
    state: 'assistant',
    workArea: null,
    scaleFactor: null,
  }
  let suspended = false
  let hiddenUntilChatKey = false
  let trackingCompactDrag = false
  let compactRestorePosition: PhysicalPosition | null = null
  let compactRestoreWorkArea: GameForegroundEvent['workArea'] = null
  let diagnosticRefreshInFlight = false
  let transition = Promise.resolve()
  const unlisteners: UnlistenFn[] = []

  function queue(operation: () => Promise<void>): Promise<void> {
    transition = transition.then(operation, operation).catch((error) => {
      options.onError(error instanceof Error ? error.message : String(error))
    })
    return transition
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

  async function prepareCompactNativeWindow(
    window: Window,
    width: number,
    height: number,
    position: PhysicalPosition | null,
  ): Promise<void> {
    if (await window.isMaximized()) await window.unmaximize()
    await window.setFocusable(false)
    await window.setSkipTaskbar(true)
    await window.setAlwaysOnTop(true)
    await window.setDecorations(false)
    await window.setShadow(false)
    await window.setResizable(false)
    await window.setSizeConstraints({
      minWidth: COMPACT_WIDTH,
      minHeight: COMPACT_HEIGHT,
      maxWidth: COMPACT_WIDTH,
      maxHeight: COMPACT_HEIGHT,
    })
    await window.setSize(new PhysicalSize(width, height))
    if (position) await window.setPosition(position)
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
    await rememberMainGeometry(appWindow)

    const scale = Math.max(1, payload.scaleFactor ?? 1)
    const width = Math.round(COMPACT_WIDTH * scale)
    const height = Math.round(COMPACT_HEIGHT * scale)
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

    await prepareCompactNativeWindow(appWindow, width, height, position)
    isCompact.value = true
    isComposerFocused.value = false
    await nextTick()
    await appWindow.show()
    await appWindow.unminimize()
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
    await appWindow.setFocusable(true)
    await appWindow.setSkipTaskbar(false)
    await appWindow.setResizable(true)
    await appWindow.setSizeConstraints(null)
    await appWindow.setDecorations(true)
    await appWindow.setShadow(true)
    await appWindow.show()
    await appWindow.unminimize()
    await appWindow.setAlwaysOnTop(focus)
    const restoreSize = mainGeometry?.size ?? new PhysicalSize(MAIN_DEFAULT_WIDTH, MAIN_DEFAULT_HEIGHT)
    await appWindow.setSize(restoreSize)
    const restorePosition = restoreFromCompactPosition
      ? clampMainPosition(restoreFromCompactPosition, restoreWorkArea, restoreSize)
      : mainGeometry && !isMinimizedShellPosition(mainGeometry.position)
        ? copyPosition(mainGeometry.position)
        : fallbackMainPosition(restoreWorkArea, restoreSize)
    if (restorePosition) {
      await appWindow.setPosition(restorePosition)
    }
    if (mainGeometry) {
      if (mainGeometry.maximized) await appWindow.maximize()
    }
    await appWindow.setSizeConstraints({ minWidth: MAIN_MIN_WIDTH, minHeight: MAIN_MIN_HEIGHT })
    if (focus) await appWindow.setFocus()
    await appWindow.setAlwaysOnTop(false)
  }

  async function handleForeground(payload: GameForegroundEvent): Promise<void> {
    latestForeground = payload
    if (payload.state === 'game') {
      if (options.enabled.value) {
        void refreshGameDiagnostic()
      }
      return
    }
    if (payload.state === 'assistant' && isCompact.value) {
      await appWindow?.setFocusable(true)
    }
  }

  async function handleWindowFocusChanged(focused: boolean): Promise<void> {
    if (!focused || !isCompact.value || isComposerFocused.value || options.busy.value) return
    await restoreWindow(true)
  }

  async function handleWindowMoved(position: PhysicalPosition): Promise<void> {
    if (!appWindow || isMinimizedShellPosition(position)) return
    const nextPosition = copyPosition(position)
    if (isCompact.value) {
      if (!trackingCompactDrag) return
      compactRestorePosition = nextPosition
      localStorage.setItem(COMPACT_POSITION_KEY, JSON.stringify({ x: nextPosition.x, y: nextPosition.y }))
      return
    }
    if (await appWindow.isMinimized()) return
    mainGeometry = {
      position: nextPosition,
      size: await appWindow.outerSize(),
      maximized: await appWindow.isMaximized(),
    }
  }

  async function focusComposer(): Promise<void> {
    if (!appWindow || !isCompact.value || suspended || options.busy.value) return
    await appWindow.setFocusable(true)
    isComposerFocused.value = true
    try {
      await appWindow.setFocus()
    } catch (error) {
      isComposerFocused.value = false
      throw error
    }
    await nextTick()
    await options.onComposerFocused()
  }

  async function showComposerForChatKey(): Promise<void> {
    if (!appWindow || !options.enabled.value || options.busy.value || latestForeground.state !== 'game') return
    suspended = false
    hiddenUntilChatKey = false
    await enterCompactMode(latestForeground)
    await focusComposer()
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
    isComposerFocused.value = false
    await appWindow.setSkipTaskbar(false)
    await appWindow.setFocusable(true)
    await appWindow.minimize()
  }

  async function dismissCompact(): Promise<void> {
    if (!appWindow || !isCompact.value) return
    trackingCompactDrag = false
    hiddenUntilChatKey = true
    isComposerFocused.value = false
    await appWindow.setFocusable(false)
    await appWindow.setSkipTaskbar(true)
    await appWindow.minimize()
  }

  async function restoreWindow(focus = true): Promise<void> {
    suspended = false
    hiddenUntilChatKey = false
    await restoreFullMode(focus)
  }

  async function resumeCompactIfGame(): Promise<void> {
    suspended = false
    hiddenUntilChatKey = false
    if (isCompact.value && latestForeground.state === 'game' && options.enabled.value) {
      await enterCompactMode(latestForeground)
    }
  }

  async function start(): Promise<void> {
    appWindow = getCurrentWindow()
    unlisteners.push(
      await appWindow.onMoved(({ payload: position }) => {
        void queue(() => handleWindowMoved(position))
      }),
      await appWindow.onFocusChanged(({ payload: focused }) => {
        void queue(() => handleWindowFocusChanged(focused))
      }),
      await listen<GameForegroundEvent>('game-foreground-changed', (event) => {
        void queue(() => handleForeground(event.payload))
      }),
      await listen<GameForegroundEvent>('game-chat-key-released', (event) => {
        latestForeground = event.payload
        void queue(showComposerForChatKey)
      }),
    )
  }

  async function dispose(): Promise<void> {
    for (const unlisten of unlisteners.splice(0)) unlisten()
    suspended = false
    if (isCompact.value) await queue(() => restoreFullMode(false))
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
    dismissCompact: () => queue(dismissCompact),
    expandFull: (focus = true) => queue(() => restoreWindow(focus)),
    yieldWindow: () => queue(yieldWindow),
    restoreWindow: (focus = true) => queue(() => restoreWindow(focus)),
    resumeCompactIfGame: () => queue(resumeCompactIfGame),
  }
}
