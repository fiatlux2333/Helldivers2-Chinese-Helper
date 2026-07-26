import { submitIntentFromKeydown } from './submit'

const enter = {
  key: 'Enter',
  repeat: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
}

describe('submitIntentFromKeydown', () => {
  it('maps plain Enter to direct send', () => {
    expect(submitIntentFromKeydown(enter)).toBe('send')
  })

  it('maps Ctrl+Enter to fill only', () => {
    expect(submitIntentFromKeydown({ ...enter, ctrlKey: true })).toBe('fill')
  })

  it('ignores repeats and unsupported modifiers', () => {
    expect(submitIntentFromKeydown({ ...enter, repeat: true })).toBeNull()
    expect(submitIntentFromKeydown({ ...enter, shiftKey: true })).toBeNull()
    expect(submitIntentFromKeydown({ ...enter, altKey: true })).toBeNull()
    expect(submitIntentFromKeydown({ ...enter, metaKey: true })).toBeNull()
  })
})
