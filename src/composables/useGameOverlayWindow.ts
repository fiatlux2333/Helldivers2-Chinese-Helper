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
  let transition = Promise.resolve()
  const unlisteners: UnlistenFn[] = []

  function queue(operation: () => Promise<void>): Promise<void> {
    transition = transition.then(operation, operation).catch((error) => {
      options.onError(error instanceof Error ? error.message : String(error))
    })
    return transition
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
    const maximized = await window.isMaximized()
    if (maximized) await window.unmaximize()
    mainGeometry = {
      position: await window.outerPosition(),
      size: await window.outerSize(),
      maximized,
    }
  }

  async function enterCompactMode(payload: GameForegroundEvent): Promise<void> {
    if (!appWindow || suspended || hiddenUntilChatKey || !options.enabled.value || payload.state !== 'game') return
    trackingCompactDrag = false
    await rememberMainGeometry(appWindow)

    const scale = Math.max(1, payload.scaleFactor ?? 1)
    const width = Math.round(COMPACT_WIDTH * scale)
    const height = Math.round(COMPACT_HEIGHT * scale)
    const workArea = payload.workArea
    const position =
      savedCompactPosition(workArea, width, height) ??
      (workArea
        ? new PhysicalPosition(
          Math.round(workArea.x + workArea.width - width - COMPACT_RIGHT_MARGIN * scale),
          Math.round(workArea.y + (workArea.height - height) * COMPACT_VERTICAL_RATIO),
        )
        : null)

    isCompact.value = true
    isComposerFocused.value = false
    await nextTick()
    await appWindow.unminimize()
    await appWindow.setFocusable(true)
    await appWindow.setAlwaysOnTop(true)
    await appWindow.setSkipTaskbar(true)
    await appWindow.setDecorations(false)
    await appWindow.setShadow(false)
    await appWindow.setResizable(false)
    await appWindow.setSizeConstraints({
      minWidth: COMPACT_WIDTH,
      minHeight: COMPACT_HEIGHT,
      maxWidth: COMPACT_WIDTH,
      maxHeight: COMPACT_HEIGHT,
    })
    await appWindow.setSize(new PhysicalSize(width, height))
    if (position) await appWindow.setPosition(position)
  }

  async function restoreFullMode(focus: boolean): Promise<void> {
    if (!appWindow) return
    trackingCompactDrag = false
    hiddenUntilChatKey = false
    isCompact.value = false
    isComposerFocused.value = false
    await nextTick()
    await appWindow.unminimize()
    await appWindow.setFocusable(true)
    await appWindow.setAlwaysOnTop(false)
    await appWindow.setSkipTaskbar(false)
    await appWindow.setDecorations(true)
    await appWindow.setShadow(true)
    await appWindow.setSizeConstraints({ minWidth: 720, minHeight: 560 })
    await appWindow.setResizable(true)
    if (mainGeometry) {
      await appWindow.setSize(mainGeometry.size)
      await appWindow.setPosition(mainGeometry.position)
      if (mainGeometry.maximized) await appWindow.maximize()
    }
    if (focus) await appWindow.setFocus()
  }

  async function handleForeground(payload: GameForegroundEvent): Promise<void> {
    latestForeground = payload
    if (payload.state === 'game') {
      if (options.enabled.value) {
        await options.onGameForeground()
      }
      if (!suspended && options.enabled.value) {
        await enterCompactMode(payload)
      }
      return
    }
    if (payload.state === 'other' && isCompact.value) {
      await restoreFullMode(false)
    }
  }

  async function focusComposer(): Promise<void> {
    if (!appWindow || !isCompact.value || suspended || options.busy.value) return
    await appWindow.setFocusable(true)
    await appWindow.setFocus()
    isComposerFocused.value = true
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
    await appWindow.setFocusable(false)
    await appWindow.minimize()
  }

  async function dismissCompact(): Promise<void> {
    if (!appWindow || !isCompact.value) return
    trackingCompactDrag = false
    hiddenUntilChatKey = true
    isComposerFocused.value = false
    await appWindow.setFocusable(false)
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
    if (latestForeground.state === 'game' && options.enabled.value) {
      await enterCompactMode(latestForeground)
    }
  }

  async function start(): Promise<void> {
    appWindow = getCurrentWindow()
    unlisteners.push(
      await appWindow.onMoved(({ payload: position }) => {
        if (!isCompact.value || !trackingCompactDrag) return
        localStorage.setItem(COMPACT_POSITION_KEY, JSON.stringify({ x: position.x, y: position.y }))
      }),
      await listen<GameForegroundEvent>('game-foreground-changed', (event) => {
        void queue(() => handleForeground(event.payload))
      }),
      await listen('game-chat-key-released', () => {
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
    } else if (enabled && latestForeground.state === 'game') {
      void queue(() => enterCompactMode(latestForeground))
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
    expandFull: (focus = true) => queue(() => restoreFullMode(focus)),
    yieldWindow: () => queue(yieldWindow),
    restoreWindow: (focus = true) => queue(() => restoreWindow(focus)),
    resumeCompactIfGame: () => queue(resumeCompactIfGame),
  }
}
