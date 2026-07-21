/** Default global accelerator used by the restore hotkey. */
export const DEFAULT_RESTORE_HOTKEY = 'CommandOrControl+Shift+H'

/** localStorage key for the user-chosen restore hotkey. */
export const HOTKEY_STORAGE_KEY = 'hd2cn.restoreHotkey'

/** Max interval between two presses to count as a double-tap (yield foreground). */
export const HOTKEY_DOUBLE_TAP_MS = 450

/**
 * Decide restore vs yield for consecutive hotkey presses.
 * First press → restore immediately; a second press within the window → yield.
 */
export function resolveHotkeyTap(
  nowMs: number,
  lastPressAtMs: number,
  windowMs = HOTKEY_DOUBLE_TAP_MS,
): { action: 'restore' | 'yield'; nextLastPressAtMs: number } {
  if (lastPressAtMs > 0 && nowMs - lastPressAtMs <= windowMs) {
    // Consume the pair so a third press starts a fresh single-tap restore.
    return { action: 'yield', nextLastPressAtMs: 0 }
  }
  return { action: 'restore', nextLastPressAtMs: nowMs }
}

const CODE_TO_KEY: Record<string, string> = {
  Space: 'Space',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  Minus: 'Minus',
  Equal: 'Equal',
  BracketLeft: 'BracketLeft',
  BracketRight: 'BracketRight',
  Backslash: 'Backslash',
  Semicolon: 'Semicolon',
  Quote: 'Quote',
  Backquote: 'Backquote',
  Comma: 'Comma',
  Period: 'Period',
  Slash: 'Slash',
}

/**
 * Convert a Tauri accelerator string into a short Windows-facing label.
 * Example: CommandOrControl+Shift+H → Ctrl+Shift+H
 */
export function formatHotkeyLabel(accelerator: string): string {
  return accelerator
    .split('+')
    .map((part) => {
      const token = part.trim()
      if (!token) return ''
      if (token === 'CommandOrControl' || token === 'Control' || token === 'Ctrl') return 'Ctrl'
      if (token === 'Command' || token === 'Super' || token === 'Meta') return 'Win'
      if (token === 'Option') return 'Alt'
      if (token === 'Shift' || token === 'Alt') return token
      if (token === 'equal') return '='
      if (token === 'Minus') return '-'
      return token
    })
    .filter(Boolean)
    .join('+')
}

/**
 * Build a Tauri accelerator from a keyboard event.
 * Requires Ctrl/Alt/Win (not Shift alone) so it does not eat plain typing keys.
 */
export function acceleratorFromKeyboardEvent(event: KeyboardEvent): string | null {
  if (event.repeat) return null

  const key = keyFromKeyboardEvent(event)
  if (!key) return null

  const parts: string[] = []
  // Prefer CommandOrControl so the saved accelerator stays cross-platform.
  if (event.ctrlKey || event.metaKey) parts.push('CommandOrControl')
  if (event.altKey) parts.push('Alt')
  if (event.shiftKey) parts.push('Shift')

  // Global restore hotkeys need at least one non-Shift modifier.
  if (parts.length === 0 || (parts.length === 1 && parts[0] === 'Shift')) {
    return null
  }

  parts.push(key)
  return parts.join('+')
}

/** Map KeyboardEvent → Tauri key token, or null for pure modifiers / unsupported keys. */
export function keyFromKeyboardEvent(event: KeyboardEvent): string | null {
  const code = event.code

  if (code.startsWith('Key') && code.length === 4) {
    return code.slice(3).toUpperCase()
  }
  if (code.startsWith('Digit') && code.length === 6) {
    return code.slice(5)
  }
  if (/^F([1-9]|1[0-2]|1[3-9]|2[0-4])$/.test(code)) {
    return code
  }
  if (code.startsWith('Numpad')) {
    const rest = code.slice(6)
    if (/^\d$/.test(rest)) return rest
    return null
  }

  // Escape cancels recording in the UI; never treat it as a bindable restore hotkey.
  if (code === 'Escape' || code === 'Enter' || code === 'NumpadEnter') {
    return null
  }

  return CODE_TO_KEY[code] ?? null
}

export function isValidAccelerator(accelerator: string): boolean {
  const parts = accelerator
    .split('+')
    .map((part) => part.trim())
    .filter(Boolean)
  if (parts.length < 2) return false

  const key = parts[parts.length - 1]
  const modifiers = parts.slice(0, -1)
  if (!key || modifiers.length === 0) return false

  const allowedMods = new Set(['CommandOrControl', 'Control', 'Ctrl', 'Alt', 'Shift', 'Super', 'Meta', 'Command', 'Option'])
  if (!modifiers.every((mod) => allowedMods.has(mod))) return false
  if (!modifiers.some((mod) => mod !== 'Shift')) return false
  return true
}

export function loadStoredHotkey(): string {
  try {
    const raw = localStorage.getItem(HOTKEY_STORAGE_KEY)
    if (raw && isValidAccelerator(raw)) return raw
  } catch {
    // localStorage may be unavailable in restricted contexts.
  }
  return DEFAULT_RESTORE_HOTKEY
}

export function saveHotkey(accelerator: string): void {
  try {
    localStorage.setItem(HOTKEY_STORAGE_KEY, accelerator)
  } catch {
    // Persistence is best-effort; registration can still work for the session.
  }
}

/** Split label into kbd parts for template rendering: Ctrl+Shift+H → ["Ctrl","Shift","H"] */
export function hotkeyLabelParts(accelerator: string): string[] {
  return formatHotkeyLabel(accelerator).split('+').filter(Boolean)
}
