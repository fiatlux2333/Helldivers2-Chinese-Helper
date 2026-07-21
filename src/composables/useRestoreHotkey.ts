import { computed, onUnmounted, ref, type Ref } from 'vue'

import {
  DEFAULT_RESTORE_HOTKEY,
  acceleratorFromKeyboardEvent,
  formatHotkeyLabel,
  hotkeyLabelParts,
  isValidAccelerator,
  loadStoredHotkey,
  resolveHotkeyTap,
  saveHotkey,
} from '@/utils/hotkey'

type RestoreHandlers = {
  isSending: Ref<boolean>
  isCapturing: Ref<boolean>
  onRestored: (label: string) => void | Promise<void>
  /** Second press within the double-tap window: minimize / yield foreground. */
  onYielded: (label: string) => void | Promise<void>
  onError: (title: string, message: string) => void
  onHotkeyChanged?: (label: string) => void
}

/**
 * Global restore hotkey: load/save preference, register with Tauri, support live rebinding.
 * Single press restores; a rapid second press yields (minimizes) the assistant.
 */
export function useRestoreHotkey(handlers: RestoreHandlers) {
  const accelerator = ref(loadStoredHotkey())
  const registeredAccelerator = ref<string | null>(null)
  const isRecording = ref(false)
  const pendingAccelerator = ref<string | null>(null)
  const isApplying = ref(false)
  /** Timestamp of the last hotkey press used for double-tap detection. */
  let lastPressAtMs = 0

  const hotkeyLabel = computed(() => formatHotkeyLabel(accelerator.value))
  const hotkeyParts = computed(() => hotkeyLabelParts(accelerator.value))
  const pendingLabel = computed(() =>
    pendingAccelerator.value ? formatHotkeyLabel(pendingAccelerator.value) : '',
  )

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

  async function unregisterIfNeeded(shortcut: string | null): Promise<void> {
    if (!shortcut) return
    try {
      await withPlugin(async ({ isRegistered, unregister }) => {
        if (await isRegistered(shortcut)) {
          await unregister(shortcut)
        }
      })
    } catch {
      // Best-effort cleanup; a later register may still succeed.
    }
  }

  async function registerAccelerator(next: string): Promise<void> {
    if (!isValidAccelerator(next)) {
      throw new Error('热键无效：请至少包含 Ctrl / Alt / Win 中的一个修饰键，再加上主键。')
    }

    await withPlugin(async ({ isRegistered, register, unregister }) => {
      // Drop the previously owned binding first so rebinding cannot stack shortcuts.
      if (registeredAccelerator.value && registeredAccelerator.value !== next) {
        if (await isRegistered(registeredAccelerator.value)) {
          await unregister(registeredAccelerator.value)
        }
      }
      if (await isRegistered(next)) {
        await unregister(next)
      }

      await register(next, async (event) => {
        if (event.state !== 'Pressed') return
        if (handlers.isSending.value || handlers.isCapturing.value) return
        if (isRecording.value) return

        const label = formatHotkeyLabel(next)
        const decision = resolveHotkeyTap(Date.now(), lastPressAtMs)
        lastPressAtMs = decision.nextLastPressAtMs

        try {
          if (decision.action === 'yield') {
            await handlers.onYielded(label)
          } else {
            await handlers.onRestored(label)
          }
        } catch (error) {
          const title = decision.action === 'yield' ? '热键收起失败' : '热键唤回失败'
          handlers.onError(title, error instanceof Error ? error.message : String(error))
        }
      })

      registeredAccelerator.value = next
      // Rebinding starts a fresh single/double-tap sequence.
      lastPressAtMs = 0
    })
  }

  function detachRecordListener(): void {
    window.removeEventListener('keydown', onRecordKeydown, true)
  }

  function attachRecordListener(): void {
    detachRecordListener()
    window.addEventListener('keydown', onRecordKeydown, true)
  }

  async function applyAccelerator(next: string, announce = true): Promise<boolean> {
    isApplying.value = true
    try {
      await registerAccelerator(next)
      accelerator.value = next
      saveHotkey(next)
      isRecording.value = false
      pendingAccelerator.value = null
      detachRecordListener()
      if (announce) {
        handlers.onHotkeyChanged?.(formatHotkeyLabel(next))
      }
      return true
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      handlers.onError(
        '全局热键注册失败',
        `${message} 仍可从任务栏点回助手。可尝试换一组热键。`,
      )
      return false
    } finally {
      isApplying.value = false
    }
  }

  async function ensureRegistered(): Promise<void> {
    try {
      await registerAccelerator(accelerator.value)
    } catch (error) {
      // Fall back once if the stored shortcut is already taken by another app.
      if (accelerator.value !== DEFAULT_RESTORE_HOTKEY) {
        try {
          await registerAccelerator(DEFAULT_RESTORE_HOTKEY)
          accelerator.value = DEFAULT_RESTORE_HOTKEY
          saveHotkey(DEFAULT_RESTORE_HOTKEY)
          handlers.onError(
            '自定义热键不可用',
            `已回退到默认热键 ${formatHotkeyLabel(DEFAULT_RESTORE_HOTKEY)}。原快捷键可能被其他程序占用。`,
          )
          return
        } catch {
          // continue to generic error
        }
      }
      const message = error instanceof Error ? error.message : String(error)
      handlers.onError(
        '全局热键注册失败',
        `${message} 仍可从任务栏点回助手。可在下方自定义热键重试。`,
      )
    }
  }

  function startRecording(): void {
    if (handlers.isSending.value || handlers.isCapturing.value || isApplying.value) return
    isRecording.value = true
    pendingAccelerator.value = null
    attachRecordListener()
  }

  function cancelRecording(): void {
    isRecording.value = false
    pendingAccelerator.value = null
    detachRecordListener()
  }

  function onRecordKeydown(event: KeyboardEvent): void {
    if (!isRecording.value) return

    // Esc cancels without changing the current hotkey.
    if (event.code === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      cancelRecording()
      return
    }

    // Ignore pure modifier presses while waiting for the final key.
    if (
      event.code === 'ControlLeft' ||
      event.code === 'ControlRight' ||
      event.code === 'ShiftLeft' ||
      event.code === 'ShiftRight' ||
      event.code === 'AltLeft' ||
      event.code === 'AltRight' ||
      event.code === 'MetaLeft' ||
      event.code === 'MetaRight'
    ) {
      event.preventDefault()
      return
    }

    event.preventDefault()
    event.stopPropagation()

    if (isApplying.value) return

    const next = acceleratorFromKeyboardEvent(event)
    if (!next) {
      handlers.onError(
        '热键无效',
        '请至少按住 Ctrl / Alt / Win 中的一个，再按主键（例如 Ctrl+Shift+H）。不能只用 Shift。',
      )
      return
    }

    pendingAccelerator.value = next
    void applyAccelerator(next, true)
  }

  async function resetDefault(): Promise<void> {
    cancelRecording()
    await applyAccelerator(DEFAULT_RESTORE_HOTKEY, true)
  }

  async function dispose(): Promise<void> {
    cancelRecording()
    await unregisterIfNeeded(registeredAccelerator.value)
    registeredAccelerator.value = null
  }

  onUnmounted(() => {
    void dispose()
  })

  return {
    accelerator,
    hotkeyLabel,
    hotkeyParts,
    isRecording,
    pendingLabel,
    isApplying,
    ensureRegistered,
    startRecording,
    cancelRecording,
    resetDefault,
    applyAccelerator,
  }
}
