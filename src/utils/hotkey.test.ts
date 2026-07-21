import {
  DEFAULT_RESTORE_HOTKEY,
  HOTKEY_DOUBLE_TAP_MS,
  acceleratorFromKeyboardEvent,
  formatHotkeyLabel,
  hotkeyLabelParts,
  isValidAccelerator,
  keyFromKeyboardEvent,
  resolveHotkeyTap,
} from './hotkey'

function keyEvent(partial: Partial<KeyboardEvent> & { code: string }): KeyboardEvent {
  return {
    repeat: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...partial,
  } as KeyboardEvent
}

describe('hotkey utils', () => {
  it('formats the default accelerator for Windows UI', () => {
    expect(formatHotkeyLabel(DEFAULT_RESTORE_HOTKEY)).toBe('Ctrl+Shift+H')
    expect(hotkeyLabelParts(DEFAULT_RESTORE_HOTKEY)).toEqual(['Ctrl', 'Shift', 'H'])
  })

  it('accepts only accelerators with a non-Shift modifier', () => {
    expect(isValidAccelerator('CommandOrControl+Shift+H')).toBe(true)
    expect(isValidAccelerator('Alt+1')).toBe(true)
    expect(isValidAccelerator('Shift+H')).toBe(false)
    expect(isValidAccelerator('H')).toBe(false)
  })

  it('maps letter/digit codes to accelerator keys', () => {
    expect(keyFromKeyboardEvent(keyEvent({ code: 'KeyH' }))).toBe('H')
    expect(keyFromKeyboardEvent(keyEvent({ code: 'Digit9' }))).toBe('9')
    expect(keyFromKeyboardEvent(keyEvent({ code: 'F5' }))).toBe('F5')
    expect(keyFromKeyboardEvent(keyEvent({ code: 'Escape' }))).toBeNull()
  })

  it('builds accelerators from keyboard events', () => {
    expect(
      acceleratorFromKeyboardEvent(
        keyEvent({ code: 'KeyH', ctrlKey: true, shiftKey: true }),
      ),
    ).toBe('CommandOrControl+Shift+H')

    expect(
      acceleratorFromKeyboardEvent(
        keyEvent({ code: 'Digit1', altKey: true }),
      ),
    ).toBe('Alt+1')

    expect(
      acceleratorFromKeyboardEvent(
        keyEvent({ code: 'KeyH', shiftKey: true }),
      ),
    ).toBeNull()
  })

  it('treats a rapid second press as yield and resets the tap pair', () => {
    const first = resolveHotkeyTap(1_000, 0)
    expect(first.action).toBe('restore')
    expect(first.nextLastPressAtMs).toBe(1_000)

    const second = resolveHotkeyTap(1_000 + HOTKEY_DOUBLE_TAP_MS - 1, first.nextLastPressAtMs)
    expect(second.action).toBe('yield')
    expect(second.nextLastPressAtMs).toBe(0)

    const third = resolveHotkeyTap(1_200, second.nextLastPressAtMs)
    expect(third.action).toBe('restore')
  })

  it('treats a slow second press as a fresh restore', () => {
    const first = resolveHotkeyTap(1_000, 0)
    const second = resolveHotkeyTap(1_000 + HOTKEY_DOUBLE_TAP_MS + 1, first.nextLastPressAtMs)
    expect(second.action).toBe('restore')
    expect(second.nextLastPressAtMs).toBe(1_000 + HOTKEY_DOUBLE_TAP_MS + 1)
  })
})
