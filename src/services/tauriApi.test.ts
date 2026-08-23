import { invoke } from '@tauri-apps/api/core'

import {
  checkForUpdates,
  exportDiagnosticLogs,
  getTargetDiagnostic,
  getTranslationSettings,
  injectProbeText,
  isTauriRuntime,
  normalizeTarget,
  previewText,
  resetInputState,
  sendQuickShout,
  sendStratagemMacro,
} from './tauriApi'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

describe('tauriApi browser fallback', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset()
    delete window.__TAURI__
    delete window.__TAURI_INTERNALS__
  })

  it('detects an ordinary browser environment', () => {
    expect(isTauriRuntime()).toBe(false)
  })

  it('uses the game overlay and Unicode defaults in browser settings', async () => {
    const settings = await getTranslationSettings()

    expect(settings.gameOverlayEnabled).toBe(true)
    expect(settings.overlayChatKey).toBe('Enter')
    expect(settings.autoLockCaps).toBe(true)
    expect(settings.autoRestoreGameplayInput).toBe(true)
    expect(settings.gameInputMethod).toBe('unicodeSendInput')
    expect(settings.gameInputDelayMs).toBe(15)
    expect(settings.incomingTranslationDisplayMode).toBe('chatTranslationPage')
    expect(settings.translationHudPosition).toBeNull()
    expect(settings.quickShoutFocusDelayMs).toBe(500)
    expect(settings.stratagemMacros).toEqual([])
    expect(settings.stratagemDirectionInputMode).toBe('wasd')
    expect(settings.stratagemAllowBareNumberHotkeys).toBe(false)
  })

  it('returns a visible platform diagnostic without invoking Rust', async () => {
    const diagnostic = await getTargetDiagnostic()

    expect(diagnostic.status).toBe('unsupported_platform')
    expect(diagnostic.valid).toBe(false)
    expect(diagnostic.message).toContain('浏览器预览环境')
  })

  it('does not report updates in the browser preview', async () => {
    const result = await checkForUpdates()

    expect(result.currentVersion).toBe('browser')
    expect(result.updateAvailable).toBe(false)
    expect(result.releaseUrl).toContain('/releases/latest')
  })

  it('does not export diagnostic logs in the browser preview', async () => {
    expect(await exportDiagnosticLogs()).toBe('')
  })

  it('previews unicode text by scalar chunks in the browser', async () => {
    const preview = await previewText('中文测试😀继续')

    expect(preview.scalarCount).toBe(7)
    expect(preview.cleanedText).toBe('中文测试😀继续')
    expect(preview.utf16Batches).toEqual([
      [20013, 25991, 27979, 35797, 55357, 56832],
      [32487, 32493],
    ])
  })

  it('does not mark a target ready until integrity is confirmed', () => {
    const raw = {
      supported: true,
      identity: {
        hwnd: '1234',
        processId: 42,
        threadId: 7,
        processCreationTime: '9999',
      },
      title: 'HELLDIVERS™ 2',
      titleMatches: true,
      isWindow: true,
      visible: true,
      minimized: false,
      cloaked: false,
    }

    expect(normalizeTarget(raw, null).status).toBe('permission_unknown')
    expect(normalizeTarget(raw, null).valid).toBe(false)
    expect(
      normalizeTarget(raw, {
        supported: true,
        currentLevel: 'medium',
        targetLevel: 'medium',
        compatible: true,
      }).valid,
    ).toBe(true)
  })

  it('explains permission mismatch with direct recovery steps', () => {
    const diagnostic = normalizeTarget(
      {
        supported: true,
        identity: {
          hwnd: '1234',
          processId: 42,
          threadId: 7,
          processCreationTime: '9999',
        },
        title: 'HELLDIVERS™ 2',
        titleMatches: true,
        isWindow: true,
        visible: true,
        minimized: false,
        cloaked: false,
      },
      {
        supported: true,
        currentLevel: 'medium',
        targetLevel: 'high',
        compatible: false,
      },
    )

    expect(diagnostic.status).toBe('permission_mismatch')
    expect(diagnostic.message).toContain('优先都普通运行')
    expect(diagnostic.message).toContain('助手也必须管理员运行')
  })

  it('returns an actionable platform error for injection', async () => {
    const result = await injectProbeText('1', '测试', true)

    expect(result.ok).toBe(false)
    expect(result.error?.code).toBe('UNSUPPORTED_PLATFORM')
    expect(result.message).toContain('Windows Tauri')
  })

  it('passes explicit chat preparation to the probe injection command', async () => {
    window.__TAURI_INTERNALS__ = {} as typeof window.__TAURI_INTERNALS__
    vi.mocked(invoke).mockResolvedValue({
      attemptedBatches: 1,
      successfulEvents: 4,
      deliveryTransport: 'SendInput',
      deliveryAcknowledged: true,
      inputCharacters: 2,
      inputDelayMs: 15,
      keyboardLayoutSwitched: false,
      keyboardLayoutBefore: null,
      keyboardLayoutRequested: null,
      keyboardLayoutRestored: null,
      numLockToggled: false,
      numLockRestored: null,
      failedBatchIndex: null,
      partialPrefixPossible: false,
      keyStateUncertain: false,
      submitAttempted: true,
      submitCompleted: true,
    })

    await injectProbeText('7', '测试', true, 'open')

    expect(invoke).toHaveBeenCalledWith('inject_probe_text', {
      generation: '7',
      text: '测试',
      submit: true,
      chatPreparation: 'open',
    })
  })

  it('passes explicit chat preparation to the Rust shout command', async () => {
    window.__TAURI_INTERNALS__ = {} as typeof window.__TAURI_INTERNALS__
    vi.mocked(invoke).mockResolvedValue({
      attemptedBatches: 1,
      successfulEvents: 4,
      deliveryTransport: 'SendInput',
      deliveryAcknowledged: true,
      inputCharacters: 2,
      inputDelayMs: 15,
      keyboardLayoutSwitched: false,
      keyboardLayoutBefore: null,
      keyboardLayoutRequested: null,
      keyboardLayoutRestored: null,
      numLockToggled: false,
      numLockRestored: null,
      failedBatchIndex: null,
      partialPrefixPossible: false,
      keyStateUncertain: false,
      submitAttempted: true,
      submitCompleted: true,
    })

    const result = await sendQuickShout('测试', undefined, 'keepOpen')

    expect(result.ok).toBe(true)
    expect(invoke).toHaveBeenCalledWith('send_quick_shout', {
      text: '测试',
      generation: null,
      chatPreparation: 'keepOpen',
    })
  })

  it('passes stratagem macros to the Rust command', async () => {
    window.__TAURI_INTERNALS__ = {} as typeof window.__TAURI_INTERNALS__
    vi.mocked(invoke).mockResolvedValue({
      attemptedBatches: 2,
      successfulEvents: 8,
      deliveryTransport: 'SendInputStratagem',
      deliveryAcknowledged: true,
      inputCharacters: 2,
      inputDelayMs: 35,
      keyboardLayoutSwitched: false,
      keyboardLayoutBefore: null,
      keyboardLayoutRequested: null,
      keyboardLayoutRestored: null,
      numLockToggled: false,
      numLockRestored: null,
      failedBatchIndex: null,
      partialPrefixPossible: false,
      keyStateUncertain: false,
      submitAttempted: false,
      submitCompleted: false,
    })

    const macroConfig = {
      label: '补给',
      hotkey: 'CommandOrControl+Alt+9',
      menuKey: 'ControlLeft',
      menuMode: 'hold' as const,
      sequence: ['KeyS', 'KeyS', 'KeyW', 'KeyD'],
      menuOpenDelayMs: 120,
      pressDelayMs: 35,
      intervalDelayMs: 35,
    }
    const result = await sendStratagemMacro(macroConfig, 'arrowKeys', '12')

    expect(result.ok).toBe(true)
    expect(invoke).toHaveBeenCalledWith('send_stratagem_macro', {
      macroConfig,
      directionInputMode: 'arrowKeys',
      generation: '12',
    })
  })

  it('passes the forced recovery flag to the Rust reset command', async () => {
    window.__TAURI_INTERNALS__ = {} as typeof window.__TAURI_INTERNALS__
    vi.mocked(invoke).mockResolvedValue({
      generation: '13',
      phase: 'editing',
      target: null,
      draft: '',
      lastError: null,
    })

    await resetInputState(true)

    expect(invoke).toHaveBeenCalledWith('reset_input_state', { force: true })
  })
})
