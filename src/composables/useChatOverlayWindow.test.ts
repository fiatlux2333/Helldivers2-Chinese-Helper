import { ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  CHAT_OVERLAY_ACTION_EVENT,
  CHAT_OVERLAY_FOCUS_REQUEST_EVENT,
  CHAT_OVERLAY_FOCUS_RESULT_EVENT,
  CHAT_OVERLAY_HEALTH_REQUEST_EVENT,
  CHAT_OVERLAY_HEALTH_RESULT_EVENT,
  CHAT_OVERLAY_INPUT_EVENT,
  CHAT_OVERLAY_POSITION_EVENT,
  CHAT_OVERLAY_WINDOW_LABEL,
  type ChatOverlayFocusRequest,
  type ChatOverlayHealthRequest,
} from '@/types/chatOverlay'

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  emitted: [] as Array<{ label: string; event: string; payload: unknown }>,
  domFocusSucceeds: true,
  healthReady: true,
  overlayDestroyed: false,
  getByLabelMisses: 0,
  createdOverlayCount: 0,
  overlayFocused: false,
  overlayPosition: { x: 100, y: 100 },
  overlaySize: { width: 500, height: 58 },
  // Response for the guard's synchronous foreground probe. undefined means
  // "backend endpoint unavailable" — the probe rejects and the guard falls
  // back to its snapshot-only timed settle (the pre-probe behavior), which is
  // what most tests below were written against. 'hang' returns a promise
  // that never settles, exercising the probe's own IPC timeout.
  probeResult: undefined as 'game' | 'assistant' | 'other' | 'hang' | undefined,
  mainFocusChangedListener: undefined as ((event: { payload: boolean }) => void) | undefined,
  focusChangedListener: undefined as ((event: { payload: boolean }) => void) | undefined,
  movedListener: undefined as ((event: { payload: { x: number; y: number } }) => void) | undefined,
  mainWindow: {
    setFocusable: vi.fn(),
    setSkipTaskbar: vi.fn(),
    show: vi.fn(),
    hide: vi.fn(),
    isVisible: vi.fn(),
    isMinimized: vi.fn(),
    isFocused: vi.fn(),
    unminimize: vi.fn(),
    setFocus: vi.fn(),
    setAlwaysOnTop: vi.fn(),
    onFocusChanged: vi.fn(),
  },
  overlayWindow: {
    setFocusable: vi.fn(),
    setSkipTaskbar: vi.fn(),
    setAlwaysOnTop: vi.fn(),
    setDecorations: vi.fn(),
    setShadow: vi.fn(),
    setResizable: vi.fn(),
    setSize: vi.fn(),
    setPosition: vi.fn(),
    show: vi.fn(),
    hide: vi.fn(),
    isVisible: vi.fn(),
    isMinimized: vi.fn(),
    unminimize: vi.fn(),
    isFocused: vi.fn(),
    setFocus: vi.fn(),
    outerPosition: vi.fn(),
    outerSize: vi.fn(),
    startDragging: vi.fn(),
    onMoved: vi.fn(),
    onFocusChanged: vi.fn(),
    destroy: vi.fn(),
  },
}))

// Factory behavior for emitTo; hoisted alongside the mocks so the vi.mock
// factory (itself hoisted to the top of the file) can reference it, and so
// resetMocks() can restore it after a test swaps in a one-off implementation
// (otherwise the swap leaks into every later test in the file). Event names
// are inlined as literals because imported constants are not initialized yet
// inside vi.hoisted.
const emitToFactory = vi.hoisted(() => {
  const factory = async (label: string, event: string, payload: unknown) => {
    mocks.emitted.push({ label, event, payload })
    if (label === 'chat-overlay' && event === 'chat-overlay:focus-request') {
      const request = payload as ChatOverlayFocusRequest
      queueMicrotask(() => {
        mocks.listeners.get('chat-overlay:focus-result')?.({
          payload: {
            requestId: request.requestId,
            focused: mocks.domFocusSucceeds,
            error: mocks.domFocusSucceeds ? null : 'dom focus rejected',
          },
        })
      })
    }
    if (label === 'chat-overlay' && event === 'chat-overlay:health-request') {
      const request = payload as ChatOverlayHealthRequest
      queueMicrotask(() => {
        mocks.listeners.get('chat-overlay:health-result')?.({
          payload: {
            requestId: request.requestId,
            documentReady: true,
            inputReady: mocks.healthReady,
            focused: false,
            error: mocks.healthReady ? null : '侧栏输入框尚未准备好',
          },
        })
      })
    }
  }
  return factory
})

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, listener: (event: { payload: unknown }) => void) => {
    mocks.listeners.set(name, listener)
    return () => mocks.listeners.delete(name)
  }),
  emitTo: vi.fn(emitToFactory),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command !== 'probe_foreground_state') throw new Error(`unexpected invoke: ${command}`)
    if (mocks.probeResult === 'hang') return new Promise<never>(() => {})
    if (mocks.probeResult === undefined) throw new Error('probe endpoint unavailable')
    return mocks.probeResult
  }),
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => mocks.mainWindow,
  Window: class {},
  PhysicalPosition: class {
    constructor(public x: number, public y: number) {}
  },
  PhysicalSize: class {
    constructor(public width: number, public height: number) {}
  },
}))

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: class {
    constructor(label: string) {
      if (label === CHAT_OVERLAY_WINDOW_LABEL) {
        mocks.createdOverlayCount += 1
        mocks.overlayDestroyed = false
        Object.assign(this, mocks.overlayWindow)
      }
    }

    static async getByLabel(label: string) {
      if (label !== CHAT_OVERLAY_WINDOW_LABEL) return null
      if (mocks.overlayDestroyed) return null
      if (mocks.getByLabelMisses > 0) {
        mocks.getByLabelMisses -= 1
        return null
      }
      return mocks.overlayWindow
    }

    async once(event: string, handler: (event: { payload: unknown }) => void) {
      if (event === 'tauri://created') {
        queueMicrotask(() => handler({ payload: null }))
      }
      return () => undefined
    }
  },
}))

import { emitTo } from '@tauri-apps/api/event'
import { useChatOverlayWindow } from './useChatOverlayWindow'
import type { GameForegroundEvent } from './useGameOverlayWindow'

const gameForeground: GameForegroundEvent = {
  state: 'game',
  workArea: { x: 0, y: 0, width: 1920, height: 1080 },
  scaleFactor: 1,
}

async function flushTransitions(): Promise<void> {
  await Promise.resolve()
  await Promise.resolve()
  await new Promise<void>((resolve) => window.setTimeout(resolve, 0))
  await Promise.resolve()
}

// The guard settles for up to GUARD_SETTLE_MS before falling through to its
// retry loop; tests must wait past the full window (settle + probes +
// retries + recover calls) before asserting on the guard's outcome.
async function waitMs(ms: number): Promise<void> {
  await new Promise<void>((resolve) => window.setTimeout(resolve, ms))
}

function resetMocks(): void {
  mocks.listeners.clear()
  mocks.emitted.length = 0
  mocks.domFocusSucceeds = true
  mocks.healthReady = true
  mocks.overlayDestroyed = false
  mocks.getByLabelMisses = 0
  mocks.createdOverlayCount = 0
  mocks.overlayFocused = false
  mocks.overlayPosition = { x: 100, y: 100 }
  mocks.overlaySize = { width: 500, height: 58 }
  // Default to the unavailable probe so a test must opt into probe-driven
  // guard behavior; the timed-settle fallback keeps the older scenarios valid.
  mocks.probeResult = undefined
  mocks.mainFocusChangedListener = undefined
  mocks.focusChangedListener = undefined
  mocks.movedListener = undefined
  // A test may swap emitTo's implementation; restore the factory behavior so
  // the swap cannot leak into later tests.
  vi.mocked(emitTo).mockImplementation(emitToFactory as never)
  for (const window of [mocks.mainWindow, mocks.overlayWindow]) {
    for (const method of Object.values(window)) method.mockReset()
  }
  mocks.mainWindow.setFocusable.mockResolvedValue(undefined)
  mocks.mainWindow.setSkipTaskbar.mockResolvedValue(undefined)
  mocks.mainWindow.show.mockResolvedValue(undefined)
  mocks.mainWindow.hide.mockResolvedValue(undefined)
  mocks.mainWindow.isFocused.mockResolvedValue(false)
  mocks.mainWindow.isMinimized.mockResolvedValue(false)
  mocks.mainWindow.unminimize.mockResolvedValue(undefined)
  mocks.mainWindow.setFocus.mockResolvedValue(undefined)
  mocks.mainWindow.setAlwaysOnTop.mockResolvedValue(undefined)
  mocks.mainWindow.onFocusChanged.mockImplementation(async (listener) => {
    mocks.mainFocusChangedListener = listener
    return () => {
      mocks.mainFocusChangedListener = undefined
    }
  })
  mocks.overlayWindow.setFocusable.mockResolvedValue(undefined)
  mocks.overlayWindow.setSkipTaskbar.mockResolvedValue(undefined)
  mocks.overlayWindow.setAlwaysOnTop.mockResolvedValue(undefined)
  mocks.overlayWindow.setDecorations.mockResolvedValue(undefined)
  mocks.overlayWindow.setShadow.mockResolvedValue(undefined)
  mocks.overlayWindow.setResizable.mockResolvedValue(undefined)
  mocks.overlayWindow.setSize.mockImplementation(async (size) => {
    mocks.overlaySize = { width: size.width, height: size.height }
  })
  mocks.overlayWindow.setPosition.mockImplementation(async (position) => {
    mocks.overlayPosition = { x: position.x, y: position.y }
  })
  mocks.overlayWindow.show.mockResolvedValue(undefined)
  mocks.overlayWindow.hide.mockResolvedValue(undefined)
  mocks.overlayWindow.destroy.mockImplementation(async () => {
    mocks.overlayDestroyed = true
  })
  mocks.overlayWindow.isMinimized.mockResolvedValue(false)
  mocks.overlayWindow.isVisible.mockResolvedValue(true)
  mocks.overlayWindow.unminimize.mockResolvedValue(undefined)
  mocks.overlayWindow.isFocused.mockImplementation(async () => mocks.overlayFocused)
  mocks.overlayWindow.setFocus.mockImplementation(async () => {
    mocks.overlayFocused = true
  })
  mocks.overlayWindow.outerPosition.mockImplementation(async () => ({ ...mocks.overlayPosition }))
  mocks.overlayWindow.outerSize.mockImplementation(async () => ({ ...mocks.overlaySize }))
  mocks.overlayWindow.startDragging.mockResolvedValue(undefined)
  mocks.overlayWindow.onMoved.mockImplementation(async (listener) => {
    mocks.movedListener = listener
    return () => {
      mocks.movedListener = undefined
    }
  })
  mocks.overlayWindow.onFocusChanged.mockImplementation(async (listener) => {
    mocks.focusChangedListener = listener
    return () => {
      mocks.focusChangedListener = undefined
    }
  })
}

// Every overlay created by a test is tracked so afterEach can dispose it:
// an undisposed instance keeps its settle/retry timers and transition
// queue alive across tests, and those late tasks call into the shared mocks
// after resetMocks() has run — the source of intermittent cross-test flakes.
const liveOverlays: Array<ReturnType<typeof useChatOverlayWindow>> = []

function createOverlay(overrides?: {
  onAction?: ReturnType<typeof vi.fn>
  onError?: (message: string) => void
  onDiagnostic?: (stage: string, message: string) => void
}) {
  const overlay = useChatOverlayWindow({
    enabled: ref(true),
    busy: ref(false),
    getComposerState: () => ({
      text: '测试',
      mode: 'direct',
      busy: false,
      canSubmit: true,
      characterCount: 2,
      characterLimit: 100,
      counterTone: 'normal',
    }),
    onComposerInput: vi.fn(),
    onComposerModeChanged: vi.fn(),
    onComposerAction: overrides?.onAction ?? vi.fn(),
    onGameForeground: vi.fn(),
    onComposerFocused: vi.fn(),
    onDiagnostic: overrides?.onDiagnostic,
    onError: overrides?.onError ?? vi.fn(),
  })
  liveOverlays.push(overlay)
  return overlay
}

describe('useChatOverlayWindow', () => {
  beforeEach(() => {
    localStorage.clear()
    resetMocks()
    liveOverlays.length = 0
  })

  afterEach(async () => {
    // Stop settle/retry timers and transition queues before the next test's
    // resetMocks() clears the shared mocks underneath them.
    for (const overlay of liveOverlays.splice(0)) {
      await overlay.dispose().catch(() => undefined)
    }
    // Give any just-fired timer callback microtasks a final chance to drain.
    await waitMs(20)
  })

  it('shows and focuses the dedicated overlay without resizing the main window', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.overlayWindow.show).toHaveBeenCalledOnce()
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledOnce()
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(mocks.mainWindow.show).not.toHaveBeenCalled()
    expect(mocks.mainWindow.hide).not.toHaveBeenCalled()
    expect('setSize' in mocks.mainWindow).toBe(false)
  })

  it('creates the dedicated overlay when the configured hidden window is missing at startup', async () => {
    mocks.getByLabelMisses = 5
    const overlay = createOverlay()

    await overlay.start()

    expect(mocks.createdOverlayCount).toBe(1)
    expect(mocks.overlayWindow.setFocusable).toHaveBeenCalledWith(false)
    expect(mocks.overlayWindow.setSkipTaskbar).toHaveBeenCalledWith(true)
    expect(mocks.overlayWindow.setAlwaysOnTop).toHaveBeenCalledWith(true)
    expect(mocks.overlayWindow.hide).toHaveBeenCalled()
  })

  it('keeps an unhealthy overlay and retries health before the next show', async () => {
    mocks.healthReady = false
    const overlay = createOverlay()

    await overlay.start()
    expect(mocks.overlayWindow.destroy).not.toHaveBeenCalled()
    expect(mocks.listeners.size).toBeGreaterThan(0)

    mocks.healthReady = true
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.createdOverlayCount).toBe(0)
    expect(mocks.overlayWindow.show).toHaveBeenCalledOnce()
  })

  it('retries a transient invisible overlay before reporting a lifecycle failure', async () => {
    let visibilityChecks = 0
    mocks.overlayWindow.isVisible.mockImplementation(async () => {
      visibilityChecks += 1
      return visibilityChecks > 1
    })
    const overlay = createOverlay()

    await overlay.start()
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await new Promise<void>((resolve) => window.setTimeout(resolve, 80))
    await flushTransitions()

    expect(mocks.overlayWindow.show).toHaveBeenCalledTimes(2)
    expect(overlay.isComposerFocused.value).toBe(true)
  })

  it('keeps the overlay usable when visibility permission is missing in an older build', async () => {
    mocks.overlayWindow.isVisible.mockRejectedValue(
      new Error('window.is_visible not allowed. Permissions associated with this command: core:window:allow-is-visible'),
    )
    const overlay = createOverlay()

    await overlay.start()
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.overlayWindow.show).toHaveBeenCalledOnce()
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledOnce()
    expect(overlay.isComposerFocused.value).toBe(true)
  })

  it('prewarms the dedicated overlay on chat key down and focuses only after release', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-prepare')?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.overlayWindow.show).toHaveBeenCalledOnce()
    expect(mocks.overlayWindow.setFocusable).toHaveBeenLastCalledWith(false)
    expect(mocks.overlayWindow.setFocus).not.toHaveBeenCalled()
    expect(overlay.isComposerFocused.value).toBe(false)

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.overlayWindow.show).toHaveBeenCalledOnce()
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledOnce()
    expect(overlay.isComposerFocused.value).toBe(true)
  })

  it('serializes immediate chat key down and release without showing the overlay twice', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-prepare')?.({ payload: gameForeground })
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.overlayWindow.show).toHaveBeenCalledOnce()
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledOnce()
    expect(overlay.isComposerFocused.value).toBe(true)
  })

  it('hides a prewarmed overlay when the game loses foreground before key release', async () => {
    const overlay = createOverlay()
    await overlay.start()
    mocks.listeners.get('game-chat-key-prepare')?.({ payload: gameForeground })
    await flushTransitions()

    mocks.listeners.get('game-foreground-changed')?.({
      payload: { state: 'other', workArea: null, scaleFactor: null },
    })
    await flushTransitions()

    expect(mocks.overlayWindow.hide).toHaveBeenCalled()
    expect(overlay.isCompact.value).toBe(false)
    expect(overlay.isComposerFocused.value).toBe(false)
  })

  it('does not reopen a prewarmed overlay after the main assistant is restored', async () => {
    const overlay = createOverlay()
    await overlay.start()
    mocks.listeners.get('game-chat-key-prepare')?.({ payload: gameForeground })
    await flushTransitions()

    mocks.mainFocusChangedListener?.({ payload: true })
    await flushTransitions()
    mocks.listeners.get('game-chat-key-released')?.({
      payload: { state: 'assistant', workArea: null, scaleFactor: null },
    })
    await flushTransitions()

    expect(mocks.overlayWindow.show).toHaveBeenCalledOnce()
    expect(mocks.overlayWindow.setFocus).not.toHaveBeenCalled()
    expect(overlay.isCompact.value).toBe(false)
  })

  it('hides the dedicated overlay before injection without minimizing either window', async () => {
    const overlay = createOverlay()
    await overlay.start()
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    await overlay.dismissCompact()

    expect(mocks.overlayWindow.setFocusable).toHaveBeenLastCalledWith(false)
    expect(mocks.overlayWindow.hide).toHaveBeenCalled()
    expect(mocks.mainWindow.hide).not.toHaveBeenCalled()
  })

  it('hides the overlay when the user activates the main assistant', async () => {
    const overlay = createOverlay()
    await overlay.start()
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    mocks.mainFocusChangedListener?.({ payload: true })
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(false)
    expect(mocks.overlayWindow.hide).toHaveBeenCalled()
    expect(mocks.mainWindow.show).toHaveBeenCalled()
  })

  it('restores the full assistant when the dedicated input cannot take DOM focus', async () => {
    mocks.domFocusSucceeds = false
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(false)
    expect(mocks.overlayWindow.hide).toHaveBeenCalled()
    expect(mocks.mainWindow.show).toHaveBeenCalled()
    expect(mocks.mainWindow.setFocus).toHaveBeenCalled()
  })

  it('immediately re-takes focus when a flicker hits within the guard window', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)
    // The overlay lost native focus while still reporting focused: the exact
    // HD2 chat-opening flicker that swallows the first keystrokes. Native
    // focus is genuinely gone (not the self-recovered case), and the guard's
    // setFocus brings it back.
    mocks.overlayFocused = false
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      mocks.overlayFocused = true
    })
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(400)
    await flushTransitions()

    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(2)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(overlay.isCompact.value).toBe(true)
    expect(mocks.mainWindow.show).not.toHaveBeenCalled()
  })

  it('starts the guard refocus as soon as a probe confirms the game owns the foreground', async () => {
    const onError = vi.fn()
    const overlay = createOverlay({ onError })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)

    // Stale-monitor shape: the last event snapshot still reports "assistant",
    // but the OS says the HD2 window owns the foreground right now. Waiting
    // out GUARD_SETTLE_MS before any setFocus is exactly how first
    // keystrokes used to fall into the game; the probe must collapse that
    // budget into the settle loop's first tick.
    mocks.probeResult = 'game'
    let mainWindowFocusChecks = 0
    let checksBeforeGuardRefocus = -1
    // The initial focusComposer already consumed one setFocus before this
    // replacement lands, so the FIRST call reaching the new implementation
    // below is the guard's own first attempt.
    let guardSetFocusCalls = 0
    mocks.mainWindow.isFocused.mockImplementation(async () => {
      mainWindowFocusChecks += 1
      return false
    })
    mocks.overlayFocused = false
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      guardSetFocusCalls += 1
      if (guardSetFocusCalls === 1) checksBeforeGuardRefocus = mainWindowFocusChecks
      mocks.overlayFocused = true
    })
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(200)
    await flushTransitions()

    // Exactly ONE main-window intent check can precede the guard's setFocus:
    // the settle loop's first tick before its first probe. A larger count
    // means the guard slept through settlement ticks instead of acting.
    expect(checksBeforeGuardRefocus).toBe(1)
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(2)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(overlay.isCompact.value).toBe(true)
    expect(onError).not.toHaveBeenCalled()
  })

  it('stands down immediately when a probe identifies a third-party foreground', async () => {
    const onError = vi.fn()
    const overlay = createOverlay({ onError })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    // Real-log shape: focusing the overlay makes the monitor emit an
    // "assistant" snapshot (our own window), which must not by itself end the
    // guard — only the probe's authoritative third-party answer may.
    mocks.listeners.get('game-foreground-changed')?.({
      payload: { state: 'assistant', workArea: null, scaleFactor: null },
    })
    await flushTransitions()

    mocks.probeResult = 'other'
    mocks.overlayFocused = false
    mocks.focusChangedListener?.({ payload: false })
    // Comfortably past what the legacy path needed to sleep and regrab: if
    // the probe answer were ignored, the guard would have focused again here.
    await waitMs(400)
    await flushTransitions()

    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(1)
    expect(overlay.isComposerFocused.value).toBe(false)
    expect(overlay.isCompact.value).toBe(true)
    expect(mocks.mainWindow.show).not.toHaveBeenCalled()
    expect(onError).not.toHaveBeenCalled()
  })

  it('falls back to the timed settle when the probe endpoint is unavailable', async () => {
    const onError = vi.fn()
    const overlay = createOverlay({ onError })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)

    // probeResult stays undefined: every probe rejects, so the settle loop
    // must degrade to the pre-probe timed behavior and still recover.
    mocks.overlayFocused = false
    let setFocusCalls = 0
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      setFocusCalls += 1
      if (setFocusCalls >= 2) mocks.overlayFocused = true
    })
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(400)
    await flushTransitions()

    expect(setFocusCalls).toBe(2)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(overlay.isCompact.value).toBe(true)
    expect(onError).not.toHaveBeenCalled()
  })

  it('falls back to the timed settle when the probe hangs past its IPC timeout', async () => {
    const onError = vi.fn()
    const overlay = createOverlay({ onError })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)

    // A stalled IPC task must not stall the settle loop forever: each probe
    // is raced against its own timeout, and the guard degrades to the
    // pre-probe timed behavior while the answer never arrives.
    mocks.probeResult = 'hang'
    mocks.overlayFocused = false
    let setFocusCalls = 0
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      setFocusCalls += 1
      if (setFocusCalls >= 2) mocks.overlayFocused = true
    })
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(600)
    await flushTransitions()

    expect(setFocusCalls).toBeGreaterThanOrEqual(2)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(overlay.isCompact.value).toBe(true)
    expect(onError).not.toHaveBeenCalled()
  })

  it('logs a numeric focus_loss elapsed time after the normal open path', async () => {
    const diagnostics: Array<{ stage: string; message: string }> = []
    const onDiagnostic = vi.fn((stage: string, message: string) => {
      diagnostics.push({ stage, message })
    })
    const overlay = createOverlay({ onDiagnostic })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)

    // The elapsed-since-open field exists to tell the open-sequence window
    // apart from the ~1.1s flicker in field logs; it is only useful when the
    // normal focusComposer success path stamps the clock (not just the
    // recovery paths). Wait long enough that a stale/absent stamp is
    // distinguishable from a fresh one.
    await waitMs(150)
    mocks.overlayFocused = false
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(400)
    await flushTransitions()

    const focusLoss = diagnostics.find((entry) => entry.message.includes('stage=focus_loss'))
    expect(focusLoss).toBeDefined()
    const match = /elapsed_since_open_ms=(\d+|null)/.exec(focusLoss!.message)
    expect(match).not.toBeNull()
    expect(match![1]).not.toBe('null')
    expect(Number(match![1])).toBeGreaterThanOrEqual(100)
  })

  it('stamps focus_loss from the fresh open after dismiss, not the previous session', async () => {
    const diagnostics: Array<{ stage: string; message: string }> = []
    const onDiagnostic = vi.fn((stage: string, message: string) => {
      diagnostics.push({ stage, message })
    })
    const overlay = createOverlay({ onDiagnostic })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.dismissCompact()
    // Long enough that a stale timestamp from the first open would produce a
    // much larger elapsed value than the fresh one.
    await waitMs(600)
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)

    mocks.overlayFocused = false
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(400)
    await flushTransitions()

    const focusLoss = diagnostics.find((entry) => entry.message.includes('stage=focus_loss'))
    expect(focusLoss).toBeDefined()
    const match = /elapsed_since_open_ms=(\d+|null)/.exec(focusLoss!.message)
    expect(match).not.toBeNull()
    expect(Number(match![1])).toBeLessThan(400)
  })

  it('still guards the flicker when the foreground monitor reports assistant after focus', async () => {
    // Real-log sequence: focusing the overlay makes the monitor classify the
    // foreground as "assistant" (the overlay belongs to this app), and ~1.1s
    // later the HD2 flicker arrives. The guard must still fire.
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    mocks.listeners.get('game-foreground-changed')?.({
      payload: { state: 'assistant', workArea: null, scaleFactor: null },
    })
    await flushTransitions()
    // Genuine flicker: native focus is gone when the loss event is processed,
    // and the guard's setFocus brings it back.
    mocks.overlayFocused = false
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      mocks.overlayFocused = true
    })
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(400)
    await flushTransitions()

    // One call from the initial focusComposer, one from the guard: the guard
    // must fire even though the monitor now classifies the foreground as
    // "assistant" (the overlay belongs to this app).
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(2)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(overlay.isCompact.value).toBe(true)
  })

  it('keeps the compact overlay when the guard refocus succeeds on a retry', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)
    // First setFocus during the flicker loses the race; the retry wins, and
    // the regained focus triggers one extra setFocus from focusComposer.
    let setFocusCalls = 0
    mocks.overlayFocused = false
    mocks.overlayWindow.isFocused.mockImplementation(async () => setFocusCalls >= 2)
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      setFocusCalls += 1
      mocks.overlayFocused = setFocusCalls >= 2
    })
    mocks.focusChangedListener?.({ payload: false })
    // timed settle + first failed attempt + one retry gap
    await waitMs(500)
    await flushTransitions()

    expect(setFocusCalls).toBe(2)
    expect(overlay.isCompact.value).toBe(true)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(mocks.mainWindow.show).not.toHaveBeenCalled()
  })

  it('restores the full assistant with an error when the guard cannot re-take focus', async () => {
    const onError = vi.fn()
    const overlay = useChatOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      getComposerState: () => ({
        text: '',
        mode: 'direct',
        busy: false,
        canSubmit: false,
        characterCount: 0,
        characterLimit: 100,
        counterTone: 'normal',
      }),
      onComposerInput: vi.fn(),
      onComposerModeChanged: vi.fn(),
      onComposerAction: vi.fn(),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError,
    })
    liveOverlays.push(overlay)
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    mocks.overlayFocused = false
    mocks.overlayWindow.isFocused.mockResolvedValue(false)
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      mocks.overlayFocused = false
    })
    mocks.focusChangedListener?.({ payload: false })
    // timed settle (40ms polls) + all three attempts with retry gaps +
    // recovery calls: leave real headroom, a tight budget flakes under load.
    await waitMs(700)
    await flushTransitions()

    // Uncertain window state must abort input instead of silently losing
    // keystrokes: collapse to the full assistant and surface the error.
    expect(overlay.isCompact.value).toBe(false)
    expect(mocks.overlayWindow.hide).toHaveBeenCalled()
    expect(mocks.mainWindow.show).toHaveBeenCalled()
    expect(onError).toHaveBeenCalled()
  })

  it('respects a focus loss when the foreground has left for a third-party window inside the guard window', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(1)
    // The user Alt+Tabs to a third-party window; the guard must not yank the
    // focus back.
    mocks.listeners.get('game-foreground-changed')?.({
      payload: { state: 'other', workArea: null, scaleFactor: null },
    })
    await flushTransitions()
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(150)
    await flushTransitions()

    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(1)
    expect(overlay.isCompact.value).toBe(true)
  })

  it('stands down quietly when the foreground turns third-party while guard retries run', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    // The refocus cannot win (the user is already in another app); the
    // monitor event lands while the retries are still running.
    mocks.overlayFocused = false
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      mocks.overlayFocused = false
    })
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(120)
    mocks.listeners.get('game-foreground-changed')?.({
      payload: { state: 'other', workArea: null, scaleFactor: null },
    })
    await waitMs(300)
    await flushTransitions()

    // No error notice, no main-window restore: the loss was intentional.
    expect(overlay.isCompact.value).toBe(true)
    expect(mocks.mainWindow.show).not.toHaveBeenCalled()
  })

  it('respects a focus loss when the main assistant window took focus inside the guard window', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(1)
    // The user clicked the main assistant window. Event arrival order is not
    // guaranteed, so the guard must re-check native focus and stand down.
    mocks.mainWindow.isFocused.mockResolvedValue(true)
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(150)
    await flushTransitions()

    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(1)
  })

  it('recovers composer focus through a hide-show reset when setFocus keeps failing', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)
    // Foreground lock shape: normal setFocus never wins, but after the
    // hide/show reset cycle the window regains focus candidacy.
    let resetDone = false
    mocks.overlayWindow.hide.mockImplementation(async () => {
      if (resetDone) return
      resetDone = true
    })
    mocks.overlayWindow.show.mockImplementation(async () => undefined)
    mocks.overlayFocused = false
    mocks.overlayWindow.isFocused.mockImplementation(async () => resetDone)
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      mocks.overlayFocused = resetDone
    })
    // Re-trigger the focus flow (next chat key) while the lock is active.
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await waitMs(400)
    await flushTransitions()

    expect(resetDone).toBe(true)
    expect(overlay.isCompact.value).toBe(true)
    expect(overlay.isComposerFocused.value).toBe(true)
  })

  it('still guards a second flicker after recovering from the first', async () => {    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)
    // First flicker: focus is gone at event time, the guard's setFocus
    // brings it back.
    mocks.overlayFocused = false
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      mocks.overlayFocused = true
    })
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(400)
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)
    const setFocusAfterFirst = mocks.overlayWindow.setFocus.mock.calls.length
    // Second flicker inside the restarted guard window: the guard must fire
    // again, not fall back to the stale focus-regained path.
    mocks.overlayFocused = false
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(400)
    await flushTransitions()

    expect(mocks.overlayWindow.setFocus.mock.calls.length).toBeGreaterThan(setFocusAfterFirst)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(overlay.isCompact.value).toBe(true)
  })

  it('does not drag the user back when the main window is clicked between guard retries', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(1)
    // First guard attempt loses the flicker race; the user then clicks the
    // main assistant window during the retry gaps. The main window also
    // reports "assistant", so only the native focus check can catch it.
    // Native focus must be gone up front, or the guard's fast path (focus
    // already self-recovered) would short-circuit before any retry. The main
    // window takes focus only after the guard's first setFocus attempt, so
    // the settle-poll intent checks pass and the abort must fire mid-retry.
    let guardAttempts = 0
    mocks.overlayFocused = false
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      guardAttempts += 1
      mocks.overlayFocused = false
    })
    mocks.mainWindow.isFocused.mockImplementation(async () => guardAttempts >= 1)
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(600)
    await flushTransitions()

    // The retry loop must abort instead of re-focusing: only the initial
    // focusComposer call happened, and the guard stood down quietly.
    expect(guardAttempts).toBe(1)
    expect(overlay.isCompact.value).toBe(true)
    expect(mocks.mainWindow.show).not.toHaveBeenCalled()
  })

  it('skips the settle wait and restores DOM focus when native focus already returned', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(1)
    // Self-recovered flicker: native focus is back when the loss event is
    // processed. The guard must restore DOM focus immediately, without
    // settling or calling setFocus again.
    mocks.focusChangedListener?.({ payload: false })
    await flushTransitions()

    expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(1)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(overlay.isCompact.value).toBe(true)
  })

  it('does not mark the composer focused when the user switches away as the retry wins', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useChatOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      getComposerState: () => ({
        text: '',
        mode: 'direct',
        busy: false,
        canSubmit: false,
        characterCount: 0,
        characterLimit: 100,
        counterTone: 'normal',
      }),
      onComposerInput: vi.fn(),
      onComposerModeChanged: vi.fn(),
      onComposerAction: vi.fn(),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    liveOverlays.push(overlay)
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)
    // The guard's winning setFocus lands at the same moment the user
    // Alt+Tabs: the third-party foreground event arrives right after the
    // retry succeeded, before the composer is marked focused.
    mocks.overlayFocused = false
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      mocks.overlayFocused = true
      mocks.listeners.get('game-foreground-changed')?.({
        payload: { state: 'other', workArea: null, scaleFactor: null },
      })
    })
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(600)
    await flushTransitions()

    // The user left: the composer must not be treated as focused and the
    // focused callback must not run, or keystrokes keep routing to the game.
    expect(overlay.isCompact.value).toBe(true)
    expect(overlay.isComposerFocused.value).toBe(false)
    expect(onComposerFocused).toHaveBeenCalledTimes(1)
    expect(mocks.mainWindow.show).not.toHaveBeenCalled()
  })

  it('does not mark the composer focused when the user switches away during the DOM focus round-trip', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useChatOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      getComposerState: () => ({
        text: '',
        mode: 'direct',
        busy: false,
        canSubmit: false,
        characterCount: 0,
        characterLimit: 100,
        counterTone: 'normal',
      }),
      onComposerInput: vi.fn(),
      onComposerModeChanged: vi.fn(),
      onComposerAction: vi.fn(),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    liveOverlays.push(overlay)
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)
    // Pin the baseline only after the initial focus has fully settled,
    // otherwise the count races the queue under full-suite load.
    await expect.poll(() => onComposerFocused.mock.calls.length, { timeout: 1_000 }).toBe(1)
    const composerFocusedCalls = onComposerFocused.mock.calls.length
    // Real flicker shape: the guard's first two setFocus attempts lose the
    // race, the third succeeds and the guard proceeds into the DOM round-trip
    // — where the Alt+Tab lands.
    let setFocusCalls = 0
    mocks.overlayWindow.setFocus.mockImplementation(async () => {
      setFocusCalls += 1
      mocks.overlayFocused = setFocusCalls >= 3
    })
    // The Alt+Tab lands while the guard's DOM focus request is in flight. The
    // initial focusComposer ran before this mock was installed, so the guard's
    // request is the first one the mock sees; inject the switch on it and
    // still complete the round-trip — a missing result would make
    // requestDomFocus wait out its 350ms timeout and the test would assert
    // before the guard finished (a false pass). Later requests fall through
    // to the default implementation.
    const originalEmitTo = (await import('@tauri-apps/api/event')).emitTo
    const defaultEmitTo = vi.mocked(originalEmitTo).getMockImplementation()
    let focusRequests = 0
    vi.mocked(originalEmitTo).mockImplementation(async (label, event, payload) => {
      if (label !== CHAT_OVERLAY_WINDOW_LABEL || event !== CHAT_OVERLAY_FOCUS_REQUEST_EVENT) {
        return defaultEmitTo?.(label, event, payload)
      }
      focusRequests += 1
      if (focusRequests > 1) {
        return defaultEmitTo?.(label, event, payload)
      }
      const request = payload as ChatOverlayFocusRequest
      queueMicrotask(() => {
        mocks.listeners.get('game-foreground-changed')?.({
          payload: { state: 'other', workArea: null, scaleFactor: null },
        })
        // The Alt+Tab already took the foreground: the DOM focus request
        // completes but reports the composer as unfocused, as it would in a
        // real switch. The guard must stand down on this result.
        mocks.listeners.get(CHAT_OVERLAY_FOCUS_RESULT_EVENT)?.({
          payload: { requestId: request.requestId, focused: false, error: null },
        })
      })
    })
    mocks.focusChangedListener?.({ payload: false })
    await waitMs(500)
    await flushTransitions()
    // Restore the saved factory implementation: mockRestore() would strip the
    // vi.mock factory itself and poison every later test in the suite.
    vi.mocked(originalEmitTo).mockImplementation(defaultEmitTo ?? (async () => undefined))
    expect(overlay.isCompact.value).toBe(true)
    // The guard itself must stand down: no composer marking, no error path.
    // (The focus-regained cascade may still run the pre-existing v0.6.0
    // "overlay refocused while hidden -> expand main window" behavior; that
    // interaction is a real-machine validation item, not a guard regression.)
  })

  it('respects a focus loss after the guard window expires', async () => {
    const performanceNow = vi.spyOn(performance, 'now')
    try {
      const overlay = createOverlay()
      await overlay.start()

      mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
      await flushTransitions()
      // Fast-forward the clock past the 1.5s guard window without fake timers.
      const atFocusComplete = performanceNow.mock.results.at(-1)?.value ?? 0
      performanceNow.mockReturnValue(atFocusComplete + 2_500)
      // The native window truly lost focus; outside the guard window this is
      // treated as the user's own action, not a flicker.
      mocks.overlayFocused = false
      mocks.focusChangedListener?.({ payload: false })
      await flushTransitions()

      expect(mocks.overlayWindow.setFocus).toHaveBeenCalledTimes(1)
      expect(overlay.isCompact.value).toBe(true)
    } finally {
      performanceNow.mockRestore()
    }
  })

  it('does not resurrect the overlay when a late focus event arrives after dismiss', async () => {
    const overlay = createOverlay()
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(true)
    await overlay.dismissCompact()
    // A stale focus-loss event lands while the overlay is intentionally
    // hidden after submit; the guard must not setFocus() it back.
    mocks.focusChangedListener?.({ payload: false })
    await flushTransitions()

    expect(mocks.overlayWindow.show).toHaveBeenCalledOnce()
    expect(overlay.isCompact.value).toBe(true)
    expect(mocks.mainWindow.show).not.toHaveBeenCalled()
  })

  it('forwards overlay input and actions to the main application', async () => {
    const onAction = vi.fn()
    const onInput = vi.fn()
    const overlay = useChatOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      getComposerState: () => ({
        text: '',
        mode: 'direct',
        busy: false,
        canSubmit: false,
        characterCount: 0,
        characterLimit: 100,
        counterTone: 'normal',
      }),
      onComposerInput: onInput,
      onComposerModeChanged: vi.fn(),
      onComposerAction: onAction,
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    liveOverlays.push(overlay)
    await overlay.start()

    mocks.listeners.get(CHAT_OVERLAY_INPUT_EVENT)?.({ payload: '你好' })
    mocks.listeners.get(CHAT_OVERLAY_ACTION_EVENT)?.({ payload: 'submit' })
    await flushTransitions()

    expect(onInput).toHaveBeenCalledWith('你好')
    expect(onAction).toHaveBeenCalledWith('submit')
  })

  it('persists the final drag position even during the programmatic settle window', async () => {
    const overlay = createOverlay()
    await overlay.start()
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    mocks.listeners.get(CHAT_OVERLAY_POSITION_EVENT)?.({ payload: { x: 320, y: 240 } })
    await flushTransitions()
    await overlay.dismissCompact()
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.overlayWindow.setPosition).toHaveBeenLastCalledWith({ x: 320, y: 240 })
  })
})
