import { computed, onUnmounted, ref, type Ref } from 'vue'

import {
  acceleratorFromKeyboardEvent,
  formatHotkeyLabel,
  hotkeyLabelParts,
  isValidAccelerator,
} from '@/utils/hotkey'

type CaptureHotkeyHandlers = {
  disabled: Ref<boolean>
  conflictsWith: () => string | null
  onTriggered: () => void | Promise<void>
  onChanged: (accelerator: string, label: string) => void | Promise<void>
  onError: (title: string, message: string) => void
}

export function useCaptureHotkey(initial: string, handlers: CaptureHotkeyHandlers) {
  const accelerator = ref(initial)
  const registeredAccelerator = ref<string | null>(null)
  const isRecording = ref(false)
  const isApplying = ref(false)

  const hotkeyLabel = computed(() => formatHotkeyLabel(accelerator.value))
  const hotkeyParts = computed(() => hotkeyLabelParts(accelerator.value))

  async function withPlugin<T>(
    run: (api: {
      isRegistered: (shortcut: string) => Promise<boolean>
      register: (
        shortcut: string,
        handler: (event: { state: string }) => void | Promise<void>,
      ) => Promise<void>
      unregister: (shortcut: string) => Promise<void>
    }) => Promise<T>,
  ): Promise<T> {
    const api = await import('@tauri-apps/plugin-global-shortcut')
    return run(api)
  }

  async function unregisterOwned(shortcut: string | null): Promise<void> {
    if (!shortcut) return
    await withPlugin(async (api) => {
      if (await api.isRegistered(shortcut)) await api.unregister(shortcut)
    })
  }

  async function applyAccelerator(next: string, announce = false): Promise<boolean> {
    if (!isValidAccelerator(next)) {
      handlers.onError('截图热键无效', '请至少使用 Ctrl、Alt 或 Win 中的一个修饰键。')
      return false
    }
    if (handlers.conflictsWith() === next) {
      handlers.onError('快捷键冲突', '截图翻译热键不能与助手唤回热键相同。')
      return false
    }
    isApplying.value = true
    try {
      await withPlugin(async (api) => {
        if (registeredAccelerator.value && registeredAccelerator.value !== next) {
          await unregisterOwned(registeredAccelerator.value)
        }
        if (await api.isRegistered(next)) await api.unregister(next)
        await api.register(next, async (event) => {
          if (event.state !== 'Pressed' || handlers.disabled.value || isRecording.value) return
          try {
            await handlers.onTriggered()
          } catch (error) {
            handlers.onError('截图翻译失败', error instanceof Error ? error.message : String(error))
          }
        })
      })
      registeredAccelerator.value = next
      accelerator.value = next
      if (announce) await handlers.onChanged(next, formatHotkeyLabel(next))
      return true
    } catch (error) {
      handlers.onError('截图热键注册失败', error instanceof Error ? error.message : String(error))
      return false
    } finally {
      isApplying.value = false
    }
  }

  function startRecording(): void {
    if (handlers.disabled.value || isApplying.value) return
    isRecording.value = true
    window.addEventListener('keydown', onRecordKeydown, true)
  }

  function cancelRecording(): void {
    isRecording.value = false
    window.removeEventListener('keydown', onRecordKeydown, true)
  }

  function onRecordKeydown(event: KeyboardEvent): void {
    if (!isRecording.value) return
    event.preventDefault()
    event.stopPropagation()
    if (event.code === 'Escape') {
      cancelRecording()
      return
    }
    const next = acceleratorFromKeyboardEvent(event)
    if (!next) return
    cancelRecording()
    void applyAccelerator(next, true)
  }

  onUnmounted(() => {
    cancelRecording()
    void unregisterOwned(registeredAccelerator.value)
  })

  return {
    accelerator,
    hotkeyLabel,
    hotkeyParts,
    isRecording,
    isApplying,
    applyAccelerator,
    startRecording,
    cancelRecording,
  }
}
