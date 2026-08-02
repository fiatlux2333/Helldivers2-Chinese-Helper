import { computed, onUnmounted, ref, type Ref } from 'vue'

import {
  acceleratorFromKeyboardEvent,
  formatHotkeyLabel,
  hotkeyLabelParts,
  isValidAccelerator,
} from '@/utils/hotkey'

type HotkeyConflict = string | { accelerator: string; label: string }
type HotkeyConflictSource = HotkeyConflict | HotkeyConflict[] | null | undefined

type CaptureHotkeyHandlers = {
  disabled: Ref<boolean>
  conflictsWith: () => HotkeyConflictSource
  onTriggered: () => void | Promise<void>
  onChanged: (accelerator: string, label: string) => void | Promise<void>
  onError: (title: string, message: string) => void
}

function normalizeConflicts(source: HotkeyConflictSource): Array<{ accelerator: string; label: string }> {
  const items = Array.isArray(source) ? source : source ? [source] : []
  return items
    .map((item) =>
      typeof item === 'string'
        ? { accelerator: item.trim(), label: '其他功能' }
        : { accelerator: item.accelerator.trim(), label: item.label },
    )
    .filter((item) => item.accelerator.trim().length > 0)
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
    const conflict = normalizeConflicts(handlers.conflictsWith()).find(
      (item) => item.accelerator === next,
    )
    if (conflict) {
      handlers.onError('快捷键冲突', `该快捷键已用于${conflict.label}。`)
      return false
    }
    isApplying.value = true
    try {
      await withPlugin(async (api) => {
        const nextAlreadyRegistered = await api.isRegistered(next)
        if (nextAlreadyRegistered) {
          if (registeredAccelerator.value === next) return
          throw new Error(`${formatHotkeyLabel(next)} 已被其他功能或程序占用`)
        }

        if (registeredAccelerator.value && registeredAccelerator.value !== next) {
          await unregisterOwned(registeredAccelerator.value)
        }
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
