import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { StratagemMacro } from '@/types/ipc'

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

import { useStratagemHotkeys } from './useStratagemHotkeys'

function macro(label: string, hotkey: string): StratagemMacro {
  return {
    label,
    hotkey,
    menuKey: 'ControlLeft',
    menuMode: 'hold',
    sequence: ['KeyW', 'KeyA', 'KeyS', 'KeyD'],
    menuOpenDelayMs: 120,
    pressDelayMs: 35,
    intervalDelayMs: 35,
  }
}

describe('useStratagemHotkeys', () => {
  beforeEach(() => {
    mocks.registered.clear()
    mocks.handlers.clear()
    mocks.isRegistered.mockClear()
    mocks.register.mockClear()
    mocks.unregister.mockClear()
  })

  it('keeps unchanged registered macros when a new shortcut registration fails', async () => {
    const onError = vi.fn()
    const hotkeys = useStratagemHotkeys({
      disabled: ref(false),
      allowBareNumberKeys: ref(false),
      conflictsWith: () => [],
      onTriggered: vi.fn(),
      onError,
    })

    await hotkeys.sync([
      macro('补给', 'F3'),
    ])

    mocks.register.mockClear()
    mocks.unregister.mockClear()
    mocks.register.mockImplementation(async (shortcut, handler) => {
      if (shortcut === 'F4') throw new Error('HotKey already registered')
      mocks.registered.add(shortcut)
      mocks.handlers.set(shortcut, handler)
    })

    await hotkeys.sync([
      macro('补给', 'F3'),
      macro('机枪', 'F4'),
    ])

    expect(mocks.unregister).not.toHaveBeenCalledWith('F3')
    expect(mocks.registered.has('F3')).toBe(true)
    expect(mocks.registered.has('F4')).toBe(false)
    expect(onError).toHaveBeenCalledWith(
      '部分战备热键未启用',
      expect.stringContaining('F4 注册失败'),
    )
  })

  it('re-registers a shortcut when the macro sequence changes', async () => {
    const onTriggered = vi.fn()
    const hotkeys = useStratagemHotkeys({
      disabled: ref(false),
      allowBareNumberKeys: ref(false),
      conflictsWith: () => [],
      onTriggered,
      onError: vi.fn(),
    })
    const oldMacro = macro('补给', 'F3')
    const newMacro = { ...oldMacro, sequence: ['KeyS', 'KeyS', 'KeyW', 'KeyD'] }

    await hotkeys.sync([oldMacro])
    await hotkeys.sync([newMacro])
    await mocks.handlers.get('F3')?.({ state: 'Pressed' })

    expect(mocks.unregister).toHaveBeenCalledWith('F3')
    expect(onTriggered).toHaveBeenCalledWith(newMacro)
  })

  it('registers bare number shortcuts only when the advanced switch is enabled', async () => {
    const onError = vi.fn()
    const hotkeys = useStratagemHotkeys({
      disabled: ref(false),
      allowBareNumberKeys: ref(false),
      conflictsWith: () => [],
      onTriggered: vi.fn(),
      onError,
    })

    await hotkeys.sync([macro('补给', '1')])

    expect(mocks.register).not.toHaveBeenCalledWith('1', expect.any(Function))
    expect(onError).toHaveBeenCalledWith(
      '部分战备热键未启用',
      expect.stringContaining('热键格式无效'),
    )

    const advancedHotkeys = useStratagemHotkeys({
      disabled: ref(false),
      allowBareNumberKeys: ref(true),
      conflictsWith: () => [],
      onTriggered: vi.fn(),
      onError: vi.fn(),
    })

    await advancedHotkeys.sync([macro('补给', '1')])

    expect(mocks.register).toHaveBeenCalledWith('1', expect.any(Function))

    await advancedHotkeys.sync([macro('补给', 'Numpad1')])

    expect(mocks.register).toHaveBeenCalledWith('Numpad1', expect.any(Function))
  })
})
