import { onUnmounted, ref, type Ref } from 'vue'

import type { QuickShout } from '@/types/ipc'
import { formatHotkeyLabel, isValidAccelerator } from '@/utils/hotkey'

type QuickShoutHotkeyHandlers = {
  disabled: Ref<boolean>
  conflictsWith: () => string[]
  onTriggered: (shout: QuickShout) => void | Promise<void>
  onError: (title: string, message: string) => void
}

export function useQuickShoutHotkeys(handlers: QuickShoutHotkeyHandlers) {
  const registered = new Set<string>()
  const isSyncing = ref(false)

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

  async function unregisterAll(): Promise<void> {
    if (registered.size === 0) return
    await withPlugin(async (api) => {
      for (const shortcut of [...registered]) {
        try {
          if (await api.isRegistered(shortcut)) await api.unregister(shortcut)
        } finally {
          registered.delete(shortcut)
        }
      }
    })
  }

  async function sync(shouts: QuickShout[]): Promise<void> {
    isSyncing.value = true
    const errors: string[] = []
    try {
      await unregisterAll()
      const conflicts = new Set(handlers.conflictsWith().filter(Boolean))
      const seen = new Set<string>()

      await withPlugin(async (api) => {
        for (const shout of shouts) {
          const shortcut = shout.hotkey.trim()
          if (!shortcut || !shout.message.trim()) continue
          if (!isValidAccelerator(shortcut)) {
            errors.push(`${shout.label}：热键格式无效`)
            continue
          }
          if (conflicts.has(shortcut) || seen.has(shortcut)) {
            errors.push(`${shout.label}：${formatHotkeyLabel(shortcut)} 与其他功能冲突`)
            continue
          }
          try {
            if (await api.isRegistered(shortcut)) {
              errors.push(`${shout.label}：${formatHotkeyLabel(shortcut)} 已被占用`)
              continue
            }

            await api.register(shortcut, async (event) => {
              if (event.state !== 'Pressed' || handlers.disabled.value) return
              await handlers.onTriggered({ ...shout })
            })
            seen.add(shortcut)
            registered.add(shortcut)
          } catch (error) {
            errors.push(
              `${shout.label}：${formatHotkeyLabel(shortcut)} 注册失败（${error instanceof Error ? error.message : String(error)}）`,
            )
          }
        }
      })

      if (errors.length > 0) {
        handlers.onError('部分喊话热键未启用', errors.slice(0, 3).join('；'))
      }
    } catch (error) {
      handlers.onError('喊话热键注册失败', error instanceof Error ? error.message : String(error))
    } finally {
      isSyncing.value = false
    }
  }

  onUnmounted(() => {
    void unregisterAll()
  })

  return { isSyncing, sync, unregisterAll }
}
