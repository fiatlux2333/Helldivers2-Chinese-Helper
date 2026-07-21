import {
  getTargetDiagnostic,
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

  it('returns a visible platform diagnostic without invoking Rust', async () => {
    const diagnostic = await getTargetDiagnostic()

    expect(diagnostic.status).toBe('unsupported_platform')
    expect(diagnostic.valid).toBe(false)
    expect(diagnostic.message).toContain('浏览器预览环境')
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
    const result = await injectProbeText('1', '测试')

    expect(result.ok).toBe(false)
    expect(result.error?.code).toBe('UNSUPPORTED_PLATFORM')
    expect(result.message).toContain('Windows Tauri')
  })
})
