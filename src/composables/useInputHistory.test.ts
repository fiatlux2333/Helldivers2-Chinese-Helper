import { useInputHistory } from './useInputHistory'

describe('useInputHistory', () => {
  it('browses older entries and restores the temporary draft', () => {
    const history = useInputHistory()
    history.add('第一条')
    history.add('第二条')

    expect(history.browseOlder('临时草稿')).toBe('第二条')
    expect(history.browseOlder('第二条')).toBe('第一条')
    expect(history.browseNewer('第一条')).toBe('第二条')
    expect(history.browseNewer('第二条')).toBe('临时草稿')
    expect(history.cursor.value).toBeNull()
  })

  it('keeps only the latest 50 entries', () => {
    const history = useInputHistory(50)
    for (let index = 1; index <= 55; index += 1) history.add(`消息 ${index}`)

    expect(history.entries.value).toHaveLength(50)
    expect(history.entries.value[0]).toBe('消息 6')
    expect(history.entries.value.at(-1)).toBe('消息 55')
  })

  it('does not add adjacent duplicates', () => {
    const history = useInputHistory()
    history.add('重复')
    history.add('重复')

    expect(history.entries.value).toEqual(['重复'])
  })
})
