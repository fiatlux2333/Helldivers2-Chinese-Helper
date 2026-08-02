import {
  checkForUpdates,
  exportDiagnosticLogs,
  getTargetDiagnostic,
  getTranslationSettings,
  injectProbeText,
  isTauriRuntime,
  normalizeTarget,
  previewText,
} from './tauriApi'

describe('tauriApi browser fallback', () => {
  beforeEach(() => {
    delete window.__TAURI__
    delete window.__TAURI_INTERNALS__
  })

  it('detects an ordinary browser environment', () => {
    expect(isTauriRuntime()).toBe(false)
  })

  it('uses the game overlay and GBK defaults in browser settings', async () => {
    const settings = await getTranslationSettings()

    expect(settings.gameOverlayEnabled).toBe(true)
    expect(settings.overlayChatKey).toBe('Enter')
    expect(settings.autoLockCaps).toBe(true)
    expect(settings.gameInputMethod).toBe('gbkAltCode')
    expect(settings.quickShoutFocusDelayMs).toBe(500)
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

  it('returns an actionable platform error for injection', async () => {
    const result = await injectProbeText('1', '测试', true)

    expect(result.ok).toBe(false)
    expect(result.error?.code).toBe('UNSUPPORTED_PLATFORM')
    expect(result.message).toContain('Windows Tauri')
  })
})
