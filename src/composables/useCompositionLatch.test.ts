import { useCompositionLatch } from './useCompositionLatch'

describe('useCompositionLatch', () => {
  it('tracks the full composition lifecycle', () => {
    const latch = useCompositionLatch()

    latch.onCompositionStart()
    expect(latch.isComposing.value).toBe(true)

    latch.onCompositionUpdate({ data: '中' } as CompositionEvent)
    expect(latch.lastCompositionData.value).toBe('中')

    latch.onCompositionEnd({ data: '中文' } as CompositionEvent)
    expect(latch.isComposing.value).toBe(false)
    expect(latch.lastCompositionData.value).toBe('中文')
  })

  it('blocks keyCode 229 even when isComposing is missing', () => {
    const latch = useCompositionLatch()

    expect(latch.shouldBlockKeydown({ key: 'Process', keyCode: 229 })).toBe(true)
    expect(latch.shouldBlockKeyup({ key: 'Process', keyCode: 229 })).toBe(true)
  })

  it('keeps candidate confirmation Enter latched until keyup', () => {
    const latch = useCompositionLatch()

    latch.onCompositionStart()
    expect(latch.shouldBlockKeydown({ key: 'Enter', isComposing: true, keyCode: 229 })).toBe(true)
    latch.onCompositionEnd({ data: '中文' } as CompositionEvent)

    expect(latch.isComposing.value).toBe(false)
    expect(latch.isLatched.value).toBe(true)
    expect(latch.shouldBlockKeydown({ key: 'ArrowUp' })).toBe(true)
    expect(latch.shouldBlockKeyup({ key: 'Enter' })).toBe(true)
    expect(latch.isLatched.value).toBe(false)
    expect(latch.shouldBlockKeydown({ key: 'Enter' })).toBe(false)
  })

  it('releases a candidate latch for Process keyup and blur fallback', () => {
    const latch = useCompositionLatch()

    latch.onCompositionStart()
    latch.shouldBlockKeydown({ key: 'Enter', isComposing: true, keyCode: 229 })
    latch.onCompositionEnd({ data: '中' } as CompositionEvent)
    expect(latch.shouldBlockKeyup({ key: 'Process', keyCode: 229 })).toBe(true)
    expect(latch.isLatched.value).toBe(false)

    latch.onCompositionStart()
    latch.shouldBlockKeydown({ key: 'Enter', isComposing: true, keyCode: 229 })
    latch.onBlur()
    expect(latch.isComposing.value).toBe(false)
    expect(latch.isLatched.value).toBe(false)
  })

  it('blocks history keys throughout composition', () => {
    const latch = useCompositionLatch()
    latch.onCompositionStart()

    expect(latch.shouldBlockKeydown({ key: 'ArrowUp' })).toBe(true)
    expect(latch.shouldBlockKeydown({ key: 'ArrowDown' })).toBe(true)
  })
})
