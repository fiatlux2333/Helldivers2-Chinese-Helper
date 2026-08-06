import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  nativeFocused: false,
  nativeSize: { width: 980, height: 680 },
  focusChangedListener: undefined as
    | ((event: { payload: boolean }) => void)
    | undefined,
  movedListener: undefined as
    | ((event: { payload: { x: number; y: number } }) => void)
    | undefined,
  resizedListener: undefined as
    | ((event: { payload: { width: number; height: number } }) => void)
    | undefined,
  window: {
    isMaximized: vi.fn(),
    isMinimized: vi.fn(),
    unmaximize: vi.fn(),
    outerPosition: vi.fn(),
    outerSize: vi.fn(),
    show: vi.fn(),
    hide: vi.fn(),
    unminimize: vi.fn(),
    setFocusable: vi.fn(),
    setAlwaysOnTop: vi.fn(),
    setSkipTaskbar: vi.fn(),
    setDecorations: vi.fn(),
    setShadow: vi.fn(),
    setResizable: vi.fn(),
    setSizeConstraints: vi.fn(),
    setSize: vi.fn(),
    setPosition: vi.fn(),
    isFocused: vi.fn(),
    setFocus: vi.fn(),
    maximize: vi.fn(),
    minimize: vi.fn(),
    startDragging: vi.fn(),
    onMoved: vi.fn(),
    onResized: vi.fn(),
    onFocusChanged: vi.fn(),
  },
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, listener: (event: { payload: unknown }) => void) => {
    mocks.listeners.set(name, listener)
    return () => mocks.listeners.delete(name)
  }),
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => mocks.window,
  PhysicalPosition: class {
    constructor(public x: number, public y: number) {}
  },
  PhysicalSize: class {
    constructor(public width: number, public height: number) {}
  },
}))

import { useGameOverlayWindow, type GameForegroundEvent } from './useGameOverlayWindow'

const gameForeground: GameForegroundEvent = {
  state: 'game',
  workArea: { x: 0, y: 0, width: 1920, height: 1080 },
  scaleFactor: 1,
}

async function flushTransitions(): Promise<void> {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
  await new Promise<void>((resolve) => window.setTimeout(resolve, 0))
  await Promise.resolve()
}

function resetWindow(): void {
  mocks.listeners.clear()
  mocks.nativeFocused = false
  mocks.nativeSize = { width: 980, height: 680 }
  mocks.focusChangedListener = undefined
  mocks.movedListener = undefined
  mocks.resizedListener = undefined
  for (const method of Object.values(mocks.window)) {
    method.mockReset()
  }
  mocks.window.isMaximized.mockResolvedValue(false)
  mocks.window.isMinimized.mockResolvedValue(false)
  mocks.window.unmaximize.mockResolvedValue(undefined)
  mocks.window.outerPosition.mockResolvedValue({ x: 100, y: 100 })
  mocks.window.outerSize.mockImplementation(async () => ({ ...mocks.nativeSize }))
  mocks.window.show.mockResolvedValue(undefined)
  mocks.window.hide.mockResolvedValue(undefined)
  mocks.window.unminimize.mockResolvedValue(undefined)
  mocks.window.setFocusable.mockResolvedValue(undefined)
  mocks.window.setAlwaysOnTop.mockResolvedValue(undefined)
  mocks.window.setSkipTaskbar.mockResolvedValue(undefined)
  mocks.window.setDecorations.mockResolvedValue(undefined)
  mocks.window.setShadow.mockResolvedValue(undefined)
  mocks.window.setResizable.mockResolvedValue(undefined)
  mocks.window.setSizeConstraints.mockResolvedValue(undefined)
  mocks.window.setSize.mockImplementation(async (size) => {
    mocks.nativeSize = { width: size.width, height: size.height }
  })
  mocks.window.setPosition.mockResolvedValue(undefined)
  mocks.window.isFocused.mockImplementation(async () => mocks.nativeFocused)
  mocks.window.setFocus.mockImplementation(async () => {
    mocks.nativeFocused = true
    mocks.focusChangedListener?.({ payload: true })
  })
  mocks.window.maximize.mockResolvedValue(undefined)
  mocks.window.minimize.mockResolvedValue(undefined)
  mocks.window.startDragging.mockResolvedValue(undefined)
  mocks.window.onMoved.mockImplementation(async (listener) => {
    mocks.movedListener = listener
    return () => {
      mocks.movedListener = undefined
    }
  })
  mocks.window.onResized.mockImplementation(async (listener) => {
    mocks.resizedListener = listener
    return () => {
      mocks.resizedListener = undefined
    }
  })
  mocks.window.onFocusChanged.mockImplementation(async (listener) => {
    mocks.focusChangedListener = listener
    return () => {
      mocks.focusChangedListener = undefined
    }
  })
}

function emitWindowFocusChanged(focused: boolean): void {
  mocks.nativeFocused = focused
  mocks.focusChangedListener?.({ payload: focused })
}

describe('useGameOverlayWindow', () => {
  beforeEach(() => {
    localStorage.clear()
    resetWindow()
  })

  it('opens the composer from a verified chat-key foreground payload', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(true)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(mocks.window.setFocusable).toHaveBeenNthCalledWith(1, false)
    expect(mocks.window.setFocusable).toHaveBeenLastCalledWith(true)
    expect(mocks.window.setSkipTaskbar).toHaveBeenCalledWith(true)
    expect(mocks.window.setSkipTaskbar).not.toHaveBeenCalledWith(false)
    expect(mocks.window.setFocus).toHaveBeenCalledOnce()
    expect(onComposerFocused).toHaveBeenCalledOnce()
  })

  it('coalesces repeated chat-key events while the composer is opening', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    await overlay.start()

    const listener = mocks.listeners.get('game-chat-key-released')
    listener?.({ payload: gameForeground })
    listener?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.window.setFocus).toHaveBeenCalledOnce()
    expect(onComposerFocused).toHaveBeenCalledOnce()
  })

  it('does not register native listeners twice', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })

    await overlay.start()
    await overlay.start()

    expect(mocks.window.onMoved).toHaveBeenCalledOnce()
    expect(mocks.window.onResized).toHaveBeenCalledOnce()
    expect(mocks.window.onFocusChanged).toHaveBeenCalledOnce()
  })

  it('does not queue chat-key opening behind a slow diagnostic refresh', async () => {
    let resolveDiagnostic: (() => void) | undefined
    const onGameForeground = vi.fn(
      () => new Promise<void>((resolve) => { resolveDiagnostic = resolve }),
    )
    const onComposerFocused = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground,
      onComposerFocused,
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-foreground-changed')?.({ payload: gameForeground })
    await flushTransitions()
    expect(onGameForeground).toHaveBeenCalledOnce()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(overlay.isComposerFocused.value).toBe(true)
    expect(onComposerFocused).toHaveBeenCalledOnce()
    resolveDiagnostic?.()
  })

  it('waits for compact native sizing before rendering the overlay shell', async () => {
    let resolveSize: (() => void) | undefined
    mocks.window.setSize.mockImplementationOnce(
      () => new Promise<void>((resolve) => { resolveSize = resolve }),
    )
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.window.setSize).toHaveBeenCalled()
    expect(overlay.isCompact.value).toBe(false)
    resolveSize?.()
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(true)
  })

  it('reapplies compact sizing when the native outer size is stale', async () => {
    const sizes = [
      { width: 980, height: 680 },
      { width: 420, height: 40 },
      { width: 500, height: 58 },
      { width: 500, height: 58 },
    ]
    mocks.window.outerSize.mockImplementation(async () => sizes.shift() ?? { width: 500, height: 58 })
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.window.setSize).toHaveBeenCalledWith({ width: 500, height: 58 })
    expect(mocks.window.setSizeConstraints).toHaveBeenCalledWith({
      minWidth: 500,
      minHeight: 58,
      maxWidth: 500,
      maxHeight: 58,
    })
    expect(overlay.isCompact.value).toBe(true)
  })

  it('uses the scaled compact size for native constraints on high DPI displays', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({
      payload: {
        state: 'game',
        workArea: { x: 0, y: 0, width: 2560, height: 1390 },
        scaleFactor: 1.25,
      },
    })
    await flushTransitions()

    expect(mocks.window.setSize).toHaveBeenCalledWith({ width: 625, height: 73 })
    expect(mocks.window.setSizeConstraints).toHaveBeenCalledWith({
      minWidth: 625,
      minHeight: 73,
      maxWidth: 625,
      maxHeight: 73,
    })
    expect(overlay.isCompact.value).toBe(true)
  })

  it('restores focus when compact mode is resumed for typing', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.deactivateComposer()
    await flushTransitions()

    await overlay.resumeCompactIfGame(true)
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(true)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(onComposerFocused).toHaveBeenCalledTimes(2)
    expect(mocks.window.setFocus).toHaveBeenCalledTimes(2)
  })

  it('does not enter compact mode when native sizing cannot converge', async () => {
    mocks.window.outerSize.mockResolvedValue({ width: 420, height: 40 })
    const onError = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError,
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(false)
    expect(onError).toHaveBeenCalledWith(expect.stringContaining('紧凑输入栏尺寸校验失败'))
  })

  it('keeps compact mode through transient other foreground and expands on demand', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(overlay.isCompact.value).toBe(true)

    mocks.listeners.get('game-foreground-changed')?.({
      payload: { state: 'other', workArea: null, scaleFactor: null },
    })
    await flushTransitions()
    expect(overlay.isCompact.value).toBe(true)

    await overlay.expandFull(true)
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(false)
    expect(mocks.window.setAlwaysOnTop).toHaveBeenLastCalledWith(false)
    expect(mocks.window.setSizeConstraints).toHaveBeenCalledWith({
      minWidth: 500,
      minHeight: 58,
      maxWidth: 500,
      maxHeight: 58,
    })
    expect(mocks.window.setSizeConstraints).toHaveBeenCalledWith(null)
    expect(mocks.window.setSizeConstraints).toHaveBeenCalledWith({
      minWidth: 720,
      minHeight: 560,
    })
    expect(mocks.window.setSize).toHaveBeenLastCalledWith({ width: 980, height: 680 })
    expect(mocks.window.show).toHaveBeenCalled()
    expect(mocks.window.setFocus).toHaveBeenCalled()
  })

  it('refocuses the compact composer when the assistant becomes foreground', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.deactivateComposer()
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(true)
    expect(mocks.window.setFocusable).toHaveBeenLastCalledWith(false)

    mocks.listeners.get('game-foreground-changed')?.({
      payload: { state: 'assistant', workArea: null, scaleFactor: null },
    })
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(true)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(mocks.window.setFocusable).toHaveBeenLastCalledWith(true)
    expect(mocks.window.setFocus).toHaveBeenCalledTimes(2)
    expect(onComposerFocused).toHaveBeenCalledTimes(2)
  })

  it('refocuses a visible compact window instead of expanding it', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    expect(onComposerFocused).toHaveBeenCalledOnce()

    emitWindowFocusChanged(false)
    await flushTransitions()
    expect(overlay.isComposerFocused.value).toBe(false)

    emitWindowFocusChanged(true)
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(true)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(onComposerFocused).toHaveBeenCalledTimes(2)
  })

  it('ignores a stale focus-loss event when the native compact window is still focused', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    mocks.focusChangedListener?.({ payload: false })
    await flushTransitions()

    expect(mocks.window.isFocused).toHaveBeenCalledOnce()
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(onComposerFocused).toHaveBeenCalledOnce()
  })

  it('allows the next chat key to reopen a compact window after it loses focus', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    emitWindowFocusChanged(false)
    await flushTransitions()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(true)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(onComposerFocused).toHaveBeenCalledTimes(2)
  })

  it('does not resize the full window again when restoring an already full assistant', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    await overlay.restoreWindow(true)
    await flushTransitions()
    await overlay.restoreWindow(true)
    await flushTransitions()

    expect(mocks.window.setSize).not.toHaveBeenCalled()
    expect(mocks.window.setDecorations).not.toHaveBeenCalledWith(false)
  })

  it('ignores programmatic restore move and resize events', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.expandFull(true)
    await flushTransitions()

    mocks.window.outerSize.mockResolvedValue({ width: 1_400, height: 1_000 })
    mocks.movedListener?.({ payload: { x: 12, y: 24 } })
    mocks.resizedListener?.({ payload: { width: 1_400, height: 1_000 } })
    await flushTransitions()

    await overlay.dismissCompact()
    await flushTransitions()
    await overlay.expandFull(true)
    await flushTransitions()

    expect(mocks.window.setSize).toHaveBeenLastCalledWith({ width: 980, height: 680 })
  })

  it('ignores a delayed stale compact resize after the full window is restored', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.expandFull(true)
    await flushTransitions()
    await new Promise<void>((resolve) => window.setTimeout(resolve, 300))

    mocks.resizedListener?.({ payload: { width: 500, height: 58 } })
    await flushTransitions()
    await overlay.dismissCompact()
    await flushTransitions()
    await overlay.expandFull(true)
    await flushTransitions()

    expect(mocks.window.setSize).toHaveBeenLastCalledWith({ width: 980, height: 680 })
  })

  it('expands the full assistant when taskbar activation focuses a compact window', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.dismissCompact()
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(true)
    expect(mocks.window.minimize).not.toHaveBeenCalled()
    expect(mocks.window.setSkipTaskbar).toHaveBeenLastCalledWith(true)

    emitWindowFocusChanged(true)
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(false)
    expect(mocks.window.show).toHaveBeenCalled()
    expect(mocks.window.setFocus).toHaveBeenCalled()
  })

  it('hides the compact window without minimizing the shell', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.dismissCompact()
    await flushTransitions()

    expect(mocks.window.hide).toHaveBeenCalledOnce()
    expect(mocks.window.minimize).not.toHaveBeenCalled()
  })

  it('surfaces a compact hide failure before any caller can inject text', async () => {
    const onError = vi.fn()
    mocks.window.hide.mockRejectedValueOnce(new Error('hide failed'))
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError,
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()

    await expect(overlay.dismissCompact()).rejects.toThrow('hide failed')
    await flushTransitions()

    expect(mocks.window.minimize).not.toHaveBeenCalled()
    expect(onError).toHaveBeenCalledWith('hide failed')
  })

  it('reopens the compact composer after it was hidden for a completed send', async () => {
    const onComposerFocused = vi.fn()
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused,
      onError: vi.fn(),
    })
    await overlay.start()

    const chatKeyListener = mocks.listeners.get('game-chat-key-released')
    chatKeyListener?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.dismissCompact()
    await flushTransitions()

    chatKeyListener?.({ payload: gameForeground })
    await flushTransitions()

    expect(mocks.window.hide).toHaveBeenCalledOnce()
    expect(mocks.window.show).toHaveBeenCalledTimes(2)
    expect(overlay.isComposerFocused.value).toBe(true)
    expect(onComposerFocused).toHaveBeenCalledTimes(2)
  })

  it('does not restore the full assistant to a minimized shell position', async () => {
    mocks.window.isMinimized.mockResolvedValue(true)
    mocks.window.outerPosition.mockResolvedValue({ x: -32_000, y: -32_000 })
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.expandFull(true)
    await flushTransitions()

    expect(overlay.isCompact.value).toBe(false)
    expect(mocks.window.setSize).toHaveBeenLastCalledWith({ width: 980, height: 680 })
    expect(mocks.window.setPosition).toHaveBeenLastCalledWith({ x: 470, y: 200 })
    expect(mocks.window.setPosition).not.toHaveBeenCalledWith({ x: -32_000, y: -32_000 })
    expect(mocks.window.setFocus).toHaveBeenCalled()
  })

  it('expands the full assistant from the dragged compact position', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.startDragging()
    await flushTransitions()
    mocks.movedListener?.({ payload: { x: 640, y: 260 } })
    await flushTransitions()
    await overlay.expandFull(true)
    await flushTransitions()

    expect(localStorage.getItem('hd2cn.overlay.position.v1')).toBe('{"x":640,"y":260}')
    expect(mocks.window.setPosition).toHaveBeenLastCalledWith({ x: 640, y: 260 })
    expect(mocks.window.setPosition).not.toHaveBeenLastCalledWith({ x: 100, y: 100 })
  })

  it('keeps the dragged full assistant position as the next restore position', async () => {
    const overlay = useGameOverlayWindow({
      enabled: ref(true),
      busy: ref(false),
      onGameForeground: vi.fn(),
      onComposerFocused: vi.fn(),
      onError: vi.fn(),
    })
    await overlay.start()

    mocks.movedListener?.({ payload: { x: 280, y: 180 } })
    await flushTransitions()
    mocks.listeners.get('game-chat-key-released')?.({ payload: gameForeground })
    await flushTransitions()
    await overlay.expandFull(true)
    await flushTransitions()

    expect(mocks.window.setPosition).toHaveBeenLastCalledWith({ x: 280, y: 180 })
  })
})
