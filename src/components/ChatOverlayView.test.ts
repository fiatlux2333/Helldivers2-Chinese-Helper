import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import ChatOverlayView from '@/components/ChatOverlayView.vue'
import { CHAT_OVERLAY_ACTION_EVENT } from '@/types/chatOverlay'
import type { ChatOverlayStatePayload } from '@/types/chatOverlay'

// The keydown ORDER in ChatOverlayView is a safety contract: IME composition
// must win over the app's Escape handling (a composing Escape belongs to the
// IME), while a busy Escape must still reach the cancel action. The regress-
// ion this pins happened precisely because those two checks swapped places.
const mocks = vi.hoisted(() => ({
  emitTo: vi.fn(async (...args: unknown[]) => undefined),
  stateListeners: [] as Array<(event: { payload: ChatOverlayStatePayload }) => void>,
}))

vi.mock('@/services/tauriApi', () => ({
  isTauriRuntime: () => true,
  recordClientDiagnostic: vi.fn(async () => undefined),
}))

vi.mock('@tauri-apps/api/event', () => ({
  emitTo: mocks.emitTo,
  listen: async (event: string, handler: (e: { payload: ChatOverlayStatePayload }) => void) => {
    if (event === 'chat-overlay:state') mocks.stateListeners.push(handler)
    return () => undefined
  },
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    setSkipTaskbar: async () => undefined,
    setAlwaysOnTop: async () => undefined,
    setDecorations: async () => undefined,
    setShadow: async () => undefined,
    setResizable: async () => undefined,
    isFocused: async () => false,
    setFocus: async () => undefined,
  }),
}))

function baseState(overrides: Partial<ChatOverlayStatePayload> = {}): ChatOverlayStatePayload {
  return {
    text: '',
    mode: 'direct',
    busy: false,
    canSubmit: false,
    characterCount: 0,
    characterLimit: 100,
    counterTone: 'normal',
    ...overrides,
  }
}

async function mountOverlay(): Promise<VueWrapper> {
  const wrapper = mount(ChatOverlayView)
  await flushPromises()
  return wrapper
}

const cancelCalls = () =>
  mocks.emitTo.mock.calls.filter(
    ([, event, action]) => event === 'chat-overlay:action' && action === 'cancel',
  )

describe('ChatOverlayView escape routing', () => {
  beforeEach(() => {
    mocks.emitTo.mockClear()
    mocks.stateListeners.splice(0)
  })

  it('does not cancel while IME composition is active: Escape belongs to the IME', async () => {
    const wrapper = await mountOverlay()
    const input = wrapper.find('input')
    // Pinyin composing: keydown is latched (shouldBlockKeydown), so the app
    // never arms the cancel flag and the IME undoes its own composition.
    await input.trigger('compositionstart')
    await input.trigger('keydown', { key: 'Escape', isComposing: true })
    // The IME fires compositionend BEFORE the Escape keyup arrives — this is
    // the exact timing the old ordering bug exploited.
    await input.trigger('compositionend', { data: '' })
    await input.trigger('keyup', { key: 'Escape' })
    expect(cancelCalls()).toHaveLength(0)
    wrapper.unmount()
  })

  it('routes a busy-state Escape to the cancel action', async () => {
    const wrapper = await mountOverlay()
    for (const handler of mocks.stateListeners) handler({ payload: baseState({ busy: true }) })
    await flushPromises()
    const input = wrapper.find('input')
    await input.trigger('keydown', { key: 'Escape' })
    await input.trigger('keyup', { key: 'Escape' })
    expect(cancelCalls()).toHaveLength(1)
    expect(mocks.emitTo).toHaveBeenCalledWith(
      'main',
      CHAT_OVERLAY_ACTION_EVENT,
      'cancel',
    )
    wrapper.unmount()
  })

  it('cancels on a plain Escape when idle (draft-clearing cancel flow)', async () => {
    const wrapper = await mountOverlay()
    const input = wrapper.find('input')
    await input.trigger('keydown', { key: 'Escape' })
    await input.trigger('keyup', { key: 'Escape' })
    expect(cancelCalls()).toHaveLength(1)
    wrapper.unmount()
  })
})
