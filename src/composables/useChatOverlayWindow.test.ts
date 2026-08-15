import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import {
  CHAT_OVERLAY_ACTION_EVENT,
  CHAT_OVERLAY_FOCUS_REQUEST_EVENT,
  CHAT_OVERLAY_FOCUS_RESULT_EVENT,
  CHAT_OVERLAY_INPUT_EVENT,
  CHAT_OVERLAY_POSITION_EVENT,
  CHAT_OVERLAY_WINDOW_LABEL,
  type ChatOverlayFocusRequest,
} from '@/types/chatOverlay'

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  emitted: [] as Array<{ label: string; event: string; payload: unknown }>,
  domFocusSucceeds: true,
  overlayFocused: false,
  overlayPosition: { x: 100, y: 100 },
  overlaySize: { width: 500, height: 58 },
  mainFocusChangedListener: undefined as ((event: { payload: boolean }) => void) | undefined,
  focusChangedListener: undefined as ((event: { payload: boolean }) => void) | undefined,
  movedListener: undefined as ((event: { payload: { x: number; y: number } }) => void) | undefined,
  mainWindow: {
    setFocusable: vi.fn(),
    setSkipTaskbar: vi.fn(),
    show: vi.fn(),
    hide: vi.fn(),
    isMinimized: vi.fn(),
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
    isMinimized: vi.fn(),
    unminimize: vi.fn(),
    isFocused: vi.fn(),
    setFocus: vi.fn(),
    outerPosition: vi.fn(),
    outerSize: vi.fn(),
    startDragging: vi.fn(),
    onMoved: vi.fn(),
    onFocusChanged: vi.fn(),
  },
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, listener: (event: { payload: unknown }) => void) => {
    mocks.listeners.set(name, listener)
    return () => mocks.listeners.delete(name)
  }),
  emitTo: vi.fn(async (label: string, event: string, payload: unknown) => {
    mocks.emitted.push({ label, event, payload })
    if (label === CHAT_OVERLAY_WINDOW_LABEL && event === CHAT_OVERLAY_FOCUS_REQUEST_EVENT) {
      const request = payload as ChatOverlayFocusRequest
      queueMicrotask(() => {
        mocks.listeners.get(CHAT_OVERLAY_FOCUS_RESULT_EVENT)?.({
          payload: {
            requestId: request.requestId,
            focused: mocks.domFocusSucceeds,
            error: mocks.domFocusSucceeds ? null : 'dom focus rejected',
          },
        })
      })
    }
  }),
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => mocks.mainWindow,
  Window: class {
    static async getByLabel(label: string) {
      return label === CHAT_OVERLAY_WINDOW_LABEL ? mocks.overlayWindow : null
    }
  },
  PhysicalPosition: class {
    constructor(public x: number, public y: number) {}
  },
  PhysicalSize: class {
    constructor(public width: number, public height: number) {}
  },
}))

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

function resetMocks(): void {
  mocks.listeners.clear()
  mocks.emitted.length = 0
  mocks.domFocusSucceeds = true
  mocks.overlayFocused = false
  mocks.overlayPosition = { x: 100, y: 100 }
  mocks.overlaySize = { width: 500, height: 58 }
  mocks.mainFocusChangedListener = undefined
  mocks.focusChangedListener = undefined
  mocks.movedListener = undefined
  for (const window of [mocks.mainWindow, mocks.overlayWindow]) {
    for (const method of Object.values(window)) method.mockReset()
  }
  mocks.mainWindow.setFocusable.mockResolvedValue(undefined)
  mocks.mainWindow.setSkipTaskbar.mockResolvedValue(undefined)
  mocks.mainWindow.show.mockResolvedValue(undefined)
  mocks.mainWindow.hide.mockResolvedValue(undefined)
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
  mocks.overlayWindow.isMinimized.mockResolvedValue(false)
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

function createOverlay(overrides?: { onAction?: ReturnType<typeof vi.fn> }) {
  return useChatOverlayWindow({
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
    onError: vi.fn(),
  })
}

describe('useChatOverlayWindow', () => {
  beforeEach(() => {
    localStorage.clear()
    resetMocks()
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
