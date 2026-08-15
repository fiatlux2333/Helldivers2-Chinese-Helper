import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('vue', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue')>()
  return { ...actual, onUnmounted: vi.fn() }
})

const mocks = vi.hoisted(() => ({
  registered: new Set<string>(),
  handlers: new Map<string, (event: { state: string }) => void | Promise<void>>(),
  isRegistered: vi.fn(async (shortcut: string) => mocks.registered.has(shortcut)),
  register: vi.fn(async (
    shortcut: string,
    handler: (event: { state: string }) => void | Promise<void>,
  ) => {
    mocks.registered.add(shortcut)
    mocks.handlers.set(shortcut, handler)
  }),
  unregister: vi.fn(async (shortcut: string) => {
    mocks.registered.delete(shortcut)
    mocks.handlers.delete(shortcut)
  }),
}))

vi.mock('@tauri-apps/plugin-global-shortcut', () => ({
  isRegistered: mocks.isRegistered,
  register: mocks.register,
  unregister: mocks.unregister,
}))

import { useQuickShoutHotkeys } from './useQuickShoutHotkeys'

describe('useQuickShoutHotkeys', () => {
  beforeEach(() => {
    mocks.registered.clear()
    mocks.handlers.clear()
    mocks.isRegistered.mockClear()
    mocks.register.mockClear()
    mocks.unregister.mockClear()
  })

  it('continues registering later shouts when one shortcut registration fails', async () => {
    mocks.register.mockImplementation(async (shortcut, handler) => {
      if (shortcut === 'CommandOrControl+Alt+1') throw new Error('HotKey already registered')
      mocks.registered.add(shortcut)
      mocks.handlers.set(shortcut, handler)
    })
    const onError = vi.fn()
    const hotkeys = useQuickShoutHotkeys({
      disabled: ref(false),
      conflictsWith: () => [],
      onTriggered: vi.fn(),
      onError,
    })

    await hotkeys.sync([
      { label: '跟我来', message: 'follow me', hotkey: 'CommandOrControl+Alt+1' },
      { label: '撤退', message: 'fall back', hotkey: 'CommandOrControl+Alt+2' },
    ])

    expect(mocks.registered.has('CommandOrControl+Alt+2')).toBe(true)
    expect(onError).toHaveBeenCalledWith(
      '部分喊话热键未启用',
      expect.stringContaining('Ctrl+Alt+1 注册失败'),
    )
  })

  it('reports an occupied shortcut without blocking unoccupied shouts', async () => {
    mocks.registered.add('CommandOrControl+Alt+1')
    const onError = vi.fn()
    const hotkeys = useQuickShoutHotkeys({
      disabled: ref(false),
      conflictsWith: () => [],
      onTriggered: vi.fn(),
      onError,
    })

    await hotkeys.sync([
      { label: '跟我来', message: 'follow me', hotkey: 'CommandOrControl+Alt+1' },
      { label: '撤退', message: 'fall back', hotkey: 'CommandOrControl+Alt+2' },
    ])

    expect(mocks.registered.has('CommandOrControl+Alt+2')).toBe(true)
    expect(onError).toHaveBeenCalledWith(
      '部分喊话热键未启用',
      expect.stringContaining('Ctrl+Alt+1 已被占用'),
    )
  })

  it('keeps unchanged registered shouts when a new shortcut registration fails', async () => {
    const onError = vi.fn()
    const hotkeys = useQuickShoutHotkeys({
      disabled: ref(false),
      conflictsWith: () => [],
      onTriggered: vi.fn(),
      onError,
    })

    await hotkeys.sync([
      { label: '跟我来', message: 'follow me', hotkey: 'CommandOrControl+Alt+1' },
    ])

    mocks.register.mockClear()
    mocks.unregister.mockClear()
    mocks.register.mockImplementation(async (shortcut, handler) => {
      if (shortcut === 'CommandOrControl+Alt+2') throw new Error('HotKey already registered')
      mocks.registered.add(shortcut)
      mocks.handlers.set(shortcut, handler)
    })

    await hotkeys.sync([
      { label: '跟我来', message: 'follow me', hotkey: 'CommandOrControl+Alt+1' },
      { label: '撤退', message: 'fall back', hotkey: 'CommandOrControl+Alt+2' },
    ])

    expect(mocks.unregister).not.toHaveBeenCalledWith('CommandOrControl+Alt+1')
    expect(mocks.registered.has('CommandOrControl+Alt+1')).toBe(true)
    expect(mocks.registered.has('CommandOrControl+Alt+2')).toBe(false)
    expect(onError).toHaveBeenCalledWith(
      '部分喊话热键未启用',
      expect.stringContaining('Ctrl+Alt+2 注册失败'),
    )
  })

  it('re-registers a shortcut when the shout text changes', async () => {
    const onTriggered = vi.fn()
    const hotkeys = useQuickShoutHotkeys({
      disabled: ref(false),
      conflictsWith: () => [],
      onTriggered,
      onError: vi.fn(),
    })

    await hotkeys.sync([
      { label: '战术', message: 'old message', hotkey: 'CommandOrControl+Alt+1' },
    ])
    await hotkeys.sync([
      { label: '战术', message: 'new message', hotkey: 'CommandOrControl+Alt+1' },
    ])

    await mocks.handlers.get('CommandOrControl+Alt+1')?.({ state: 'Pressed' })

    expect(mocks.unregister).toHaveBeenCalledWith('CommandOrControl+Alt+1')
    expect(onTriggered).toHaveBeenCalledWith({
      label: '战术',
      message: 'new message',
      hotkey: 'CommandOrControl+Alt+1',
    })
  })
})
