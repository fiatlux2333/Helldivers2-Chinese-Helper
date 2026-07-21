import { readonly, ref } from 'vue'

export interface CompositionKeyEvent {
  key: string
  isComposing?: boolean
  keyCode?: number
}

function hasImeSignal(event: CompositionKeyEvent): boolean {
  return event.isComposing === true || event.keyCode === 229
}

export function useCompositionLatch() {
  const compositionActive = ref(false)
  const awaitingEnterRelease = ref(false)
  const lastCompositionData = ref('')

  function onCompositionStart(): void {
    compositionActive.value = true
    lastCompositionData.value = ''
  }

  function onCompositionUpdate(event: CompositionEvent): void {
    compositionActive.value = true
    lastCompositionData.value = event.data ?? ''
  }

  function onCompositionEnd(event: CompositionEvent): void {
    compositionActive.value = false
    lastCompositionData.value = event.data ?? lastCompositionData.value
  }

  function shouldBlockKeydown(event: CompositionKeyEvent): boolean {
    const imeSignaled = hasImeSignal(event)

    if (event.key === 'Enter' && (compositionActive.value || imeSignaled)) {
      awaitingEnterRelease.value = true
    }

    return compositionActive.value || imeSignaled || awaitingEnterRelease.value
  }

  function shouldBlockKeyup(event: CompositionKeyEvent): boolean {
    if (
      awaitingEnterRelease.value &&
      (event.key === 'Enter' || event.key === 'Process' || event.keyCode === 229)
    ) {
      awaitingEnterRelease.value = false
      return true
    }

    return compositionActive.value || hasImeSignal(event)
  }

  function onBlur(): void {
    reset()
  }

  function reset(): void {
    compositionActive.value = false
    awaitingEnterRelease.value = false
    lastCompositionData.value = ''
  }

  return {
    isComposing: readonly(compositionActive),
    isLatched: readonly(awaitingEnterRelease),
    lastCompositionData: readonly(lastCompositionData),
    onCompositionStart,
    onCompositionUpdate,
    onCompositionEnd,
    shouldBlockKeydown,
    shouldBlockKeyup,
    onBlur,
    reset,
  }
}
