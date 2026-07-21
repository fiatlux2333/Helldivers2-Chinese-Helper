import { readonly, ref } from 'vue'

export function useInputHistory(limit = 50) {
  const entries = ref<string[]>([])
  const cursor = ref<number | null>(null)
  const draft = ref('')

  function add(value: string): void {
    if (!value || entries.value[entries.value.length - 1] === value) {
      resetBrowsing()
      return
    }

    entries.value.push(value)
    if (entries.value.length > limit) {
      entries.value.splice(0, entries.value.length - limit)
    }
    resetBrowsing()
  }

  function browseOlder(currentValue: string): string {
    if (entries.value.length === 0) return currentValue

    if (cursor.value === null) {
      draft.value = currentValue
      cursor.value = entries.value.length - 1
    } else if (cursor.value > 0) {
      cursor.value -= 1
    }

    return entries.value[cursor.value] ?? currentValue
  }

  function browseNewer(currentValue: string): string {
    if (cursor.value === null) return currentValue

    if (cursor.value < entries.value.length - 1) {
      cursor.value += 1
      return entries.value[cursor.value] ?? currentValue
    }

    const restoredDraft = draft.value
    resetBrowsing()
    return restoredDraft
  }

  function resetBrowsing(): void {
    cursor.value = null
    draft.value = ''
  }

  function clear(): void {
    entries.value = []
    resetBrowsing()
  }

  return {
    entries: readonly(entries),
    cursor: readonly(cursor),
    add,
    browseOlder,
    browseNewer,
    resetBrowsing,
    clear,
  }
}
