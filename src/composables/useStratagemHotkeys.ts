import { onUnmounted, ref, type Ref } from 'vue'

import type { StratagemMacro } from '@/types/ipc'
import { formatHotkeyLabel, isValidStratagemAccelerator } from '@/utils/hotkey'

type StratagemHotkeyHandlers = {
  disabled: Ref<boolean>
  allowBareNumberKeys: Ref<boolean>
  conflictsWith: () => string[]
  onTriggered: (macroConfig: StratagemMacro) => void | Promise<void>
  onError: (title: string, message: string) => void
}

export function useStratagemHotkeys(handlers: StratagemHotkeyHandlers) {
  const registered = new Map<string, string>()
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
      for (const shortcut of [...registered.keys()]) {
        if (await api.isRegistered(shortcut)) await api.unregister(shortcut)
        registered.delete(shortcut)
      }
    })
  }

  function macroSignature(macroConfig: StratagemMacro): string {
    return [
      macroConfig.label,
      macroConfig.menuKey,
      macroConfig.menuMode,
      macroConfig.sequence.join(','),
      macroConfig.menuOpenDelayMs,
      macroConfig.pressDelayMs,
      macroConfig.intervalDelayMs,
    ].join('\u0000')
  }

  async function sync(macros: StratagemMacro[]): Promise<void> {
    isSyncing.value = true
    const errors: string[] = []
    try {
      const conflicts = new Set(handlers.conflictsWith().filter(Boolean))
      const seen = new Set<string>()
      const desired = new Map<string, { macroConfig: StratagemMacro; signature: string }>()

      for (const macroConfig of macros) {
        const shortcut = macroConfig.hotkey.trim()
        if (!shortcut || macroConfig.sequence.length === 0) continue
        if (!isValidStratagemAccelerator(shortcut, handlers.allowBareNumberKeys.value)) {
          errors.push(`${macroConfig.label}：热键格式无效`)
          continue
        }
        if (conflicts.has(shortcut) || seen.has(shortcut)) {
          errors.push(`${macroConfig.label}：${formatHotkeyLabel(shortcut)} 与其他功能冲突`)
          continue
        }
        seen.add(shortcut)
        desired.set(shortcut, {
          macroConfig: { ...macroConfig, sequence: [...macroConfig.sequence] },
          signature: macroSignature(macroConfig),
        })
      }

      await withPlugin(async (api) => {
        for (const [shortcut, signature] of [...registered.entries()]) {
          const next = desired.get(shortcut)
          if (next && next.signature === signature) continue
          try {
            if (await api.isRegistered(shortcut)) await api.unregister(shortcut)
            registered.delete(shortcut)
          } catch (error) {
            errors.push(
              `${formatHotkeyLabel(shortcut)} 注销失败（${error instanceof Error ? error.message : String(error)}）`,
            )
          }
        }

        for (const [shortcut, { macroConfig, signature }] of desired.entries()) {
          if (registered.get(shortcut) === signature) continue
          try {
            if (await api.isRegistered(shortcut)) {
              errors.push(`${macroConfig.label}：${formatHotkeyLabel(shortcut)} 已被占用`)
              continue
            }

            await api.register(shortcut, async (event) => {
              if (event.state !== 'Pressed' || handlers.disabled.value) return
              await handlers.onTriggered({ ...macroConfig, sequence: [...macroConfig.sequence] })
            })
            registered.set(shortcut, signature)
          } catch (error) {
            errors.push(
              `${macroConfig.label}：${formatHotkeyLabel(shortcut)} 注册失败（${error instanceof Error ? error.message : String(error)}）`,
            )
          }
        }
      })

      if (errors.length > 0) {
        handlers.onError('部分战备热键未启用', errors.slice(0, 3).join('；'))
      }
    } catch (error) {
      handlers.onError('战备热键注册失败', error instanceof Error ? error.message : String(error))
    } finally {
      isSyncing.value = false
    }
  }

  onUnmounted(() => {
    void unregisterAll()
  })

  return { isSyncing, sync, unregisterAll }
}
