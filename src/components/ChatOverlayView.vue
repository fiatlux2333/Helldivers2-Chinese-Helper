<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

import { useCompositionLatch } from '@/composables/useCompositionLatch'
import { isTauriRuntime, recordClientDiagnostic } from '@/services/tauriApi'
import {
  CHAT_OVERLAY_ACTION_EVENT,
  CHAT_OVERLAY_BLUR_INPUT_EVENT,
  CHAT_OVERLAY_FOCUS_REQUEST_EVENT,
  CHAT_OVERLAY_FOCUS_RESULT_EVENT,
  CHAT_OVERLAY_HEALTH_REQUEST_EVENT,
  CHAT_OVERLAY_HEALTH_RESULT_EVENT,
  CHAT_OVERLAY_INPUT_EVENT,
  CHAT_OVERLAY_MODE_EVENT,
  CHAT_OVERLAY_POSITION_EVENT,
  CHAT_OVERLAY_STATE_EVENT,
  type ChatOverlayAction,
  type ChatOverlayFocusRequest,
  type ChatOverlayFocusResult,
  type ChatOverlayHealthRequest,
  type ChatOverlayHealthResult,
  type ChatOverlayMode,
  type ChatOverlayStatePayload,
} from '@/types/chatOverlay'

const DOM_FOCUS_RETRY_COUNT = 3
const DOM_FOCUS_RETRY_DELAY_MS = 30
const desktopRuntime = isTauriRuntime()
const inputRef = ref<HTMLInputElement | null>(null)
const localText = ref('')
const submitOnEnterRelease = ref(false)
const cancelOnEscapeRelease = ref(false)
const state = ref<ChatOverlayStatePayload>({
  text: '',
  mode: 'direct',
  busy: false,
  canSubmit: false,
  characterCount: 0,
  characterLimit: 100,
  counterTone: 'normal',
})
const composition = useCompositionLatch()
const unlisteners: UnlistenFn[] = []

const canSubmit = computed(
  () => state.value.canSubmit && !composition.isComposing.value && !composition.isLatched.value,
)

// Local counterpart of canSubmit for the Enter-to-submit path: localText is
// updated synchronously by onInput, while state.canSubmit can lag one IME
// confirmation behind (see the Enter keydown comment). busy stays in the
// check so a mid-send Enter never even arms the submit.
const canSubmitLocally = computed(
  () =>
    !state.value.busy &&
    localText.value.trim().length > 0 &&
    Array.from(localText.value).length <= state.value.characterLimit &&
    !composition.isComposing.value &&
    !composition.isLatched.value,
)

async function emitMain<T>(event: string, payload?: T): Promise<void> {
  if (!desktopRuntime) return
  await emitTo('main', event, payload)
}

async function reportFocus(request: ChatOverlayFocusRequest): Promise<void> {
  let focused = false
  let error: string | null = null
  for (let attempt = 0; attempt < DOM_FOCUS_RETRY_COUNT; attempt += 1) {
    try {
      await nextTick()
      // Windows may not have handed keyboard focus to the overlay window yet,
      // in which case focusing the DOM input has no effect and
      // `document.activeElement` never becomes the input. Request OS-level
      // focus first, then verify the window is actually focused before
      // treating the DOM focus as successful.
      const overlayWindow = getCurrentWindow()
      if (!(await overlayWindow.isFocused())) {
        await overlayWindow.setFocus()
      }
      inputRef.value?.focus()
      const windowFocused = await overlayWindow.isFocused()
      focused =
        windowFocused &&
        inputRef.value !== null &&
        document.activeElement === inputRef.value
      if (focused) break
    } catch (focusError) {
      error = focusError instanceof Error ? focusError.message : String(focusError)
    }
    if (attempt + 1 < DOM_FOCUS_RETRY_COUNT) {
      await new Promise<void>((resolve) => window.setTimeout(resolve, DOM_FOCUS_RETRY_DELAY_MS))
    }
  }
  const result: ChatOverlayFocusResult = {
    requestId: request.requestId,
    focused,
    error: focused ? null : error ?? '输入框没有取得 DOM 焦点',
  }
  await emitMain(CHAT_OVERLAY_FOCUS_RESULT_EVENT, result)
}

async function reportHealth(request: ChatOverlayHealthRequest): Promise<void> {
  await nextTick()
  const inputReady = inputRef.value !== null
  const documentReady = document.readyState === 'interactive' || document.readyState === 'complete'
  const result: ChatOverlayHealthResult = {
    requestId: request.requestId,
    documentReady,
    inputReady,
    focused: inputReady && document.activeElement === inputRef.value,
    error: documentReady && inputReady ? null : '侧栏页面或输入框尚未准备好',
  }
  await emitMain(CHAT_OVERLAY_HEALTH_RESULT_EVENT, result)
}

function onInput(event: Event): void {
  localText.value = (event.target as HTMLInputElement).value
  void emitMain(CHAT_OVERLAY_INPUT_EVENT, localText.value).catch((error) => {
    // The overlay→main input hop failing silently is the leading suspect for
    // the "typed text vanishes" dead window: make it visible if it ever fires.
    void recordClientDiagnostic(
      'overlay_view',
      `stage=input_emit_failed error=${error instanceof Error ? error.message : String(error)}`,
    ).catch(() => undefined)
  })
}

function setMode(mode: ChatOverlayMode): void {
  if (state.value.busy || state.value.mode === mode) return
  state.value = { ...state.value, mode }
  void emitMain(CHAT_OVERLAY_MODE_EVENT, mode)
}

function emitAction(action: ChatOverlayAction): void {
  void emitMain(CHAT_OVERLAY_ACTION_EVENT, action)
}

function onKeydown(event: KeyboardEvent): void {
  if (state.value.busy) {
    event.preventDefault()
    // Escape stays live while busy: it means "abort the running send" and the
    // main window routes it to cancelSending.
    if (event.key === 'Escape') cancelOnEscapeRelease.value = true
    return
  }
  // IME composition first: during pinyin composition Escape belongs to the
  // IME (undo the composition), NOT to the app — intercepting it here would
  // clear the draft and fire a cancel into the game.
  if (composition.shouldBlockKeydown(event)) return
  if (event.key === 'Escape') {
    event.preventDefault()
    cancelOnEscapeRelease.value = true
    return
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault()
    emitAction('historyOlder')
    return
  }
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    emitAction('historyNewer')
    return
  }
  if (event.key === 'Enter' && !event.repeat) {
    event.preventDefault()
    // Enter-to-submit uses the LOCAL text, not the main-window snapshot:
    // state.canSubmit rides two IPC hops (input event → main window → state
    // push), so the Enter that confirms an IME candidate arrives while the
    // snapshot still says "empty text" — silently swallowing the next Enter
    // (observed: 29 suppressions, ~22% of sidebar sessions). The main window
    // re-validates on its side anyway (busy/target guards stay there).
    submitOnEnterRelease.value =
      !(event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) && canSubmitLocally.value
  }
}

function onKeyup(event: KeyboardEvent): void {
  const blocked = composition.shouldBlockKeyup(event)
  if (event.key === 'Escape') {
    const shouldCancel = cancelOnEscapeRelease.value
    cancelOnEscapeRelease.value = false
    if (!blocked && shouldCancel) emitAction('cancel')
    return
  }
  if (event.key !== 'Enter') return
  const shouldSubmit = submitOnEnterRelease.value
  submitOnEnterRelease.value = false
  // Field diagnostics for "Enter does not send" reports: the overlay-side
  // submit chain is otherwise invisible (the main window only logs submits
  // that actually started).
  if (blocked || !shouldSubmit) {
    void recordClientDiagnostic(
      'overlay_view',
      `stage=submit_suppressed key=${event.key} blocked=${blocked} armed=${shouldSubmit} can_submit=${canSubmit.value} composing=${composition.isComposing.value} latched=${composition.isLatched.value} state_can_submit=${state.value.canSubmit} local_len=${Array.from(localText.value).length}`,
    ).catch(() => undefined)
    return
  }
  void recordClientDiagnostic('overlay_view', 'stage=submit_emitted')
  emitAction('submit')
}

async function startDragging(): Promise<void> {
  if (!desktopRuntime || state.value.busy) return
  const overlayWindow = getCurrentWindow()
  await overlayWindow.startDragging()
  const reportPosition = async () => {
    const position = await overlayWindow.outerPosition()
    await emitMain(CHAT_OVERLAY_POSITION_EVENT, { x: position.x, y: position.y })
  }
  await reportPosition()
  window.setTimeout(() => {
    void reportPosition()
  }, 120)
}

onMounted(async () => {
  document.documentElement.classList.add('is-chat-overlay-document')
  if (!desktopRuntime) {
    state.value = { ...state.value, text: '测试消息', characterCount: 4, canSubmit: true }
    localText.value = state.value.text
    await nextTick()
    inputRef.value?.focus()
    return
  }
  const window = getCurrentWindow()
  await window.setSkipTaskbar(true)
  await window.setAlwaysOnTop(true)
  await window.setDecorations(false)
  await window.setShadow(false)
  await window.setResizable(false)
  unlisteners.push(
    await listen<ChatOverlayStatePayload>(CHAT_OVERLAY_STATE_EVENT, (event) => {
      state.value = event.payload
      if (!composition.isComposing.value) {
        // Wipe detector: a state push that clears a NON-empty local box means
        // the main window never saw our input (its text is still '') — the
        // "typed text vanishes" symptom. onInput-based logging cannot see
        // this: a pushed overwrite fires no input event.
        if (localText.value.length > 0 && event.payload.text.length === 0) {
          void recordClientDiagnostic(
            'overlay_view',
            `stage=local_wiped prev_len=${Array.from(localText.value).length} busy=${event.payload.busy}`,
          ).catch(() => undefined)
        }
        // Empty-push guard: the main window's draft snapshot can trail the
        // overlay's live text — replayed keys land HERE via SendInput, not
        // through the main window, and a stale text='' state push arriving
        // between the SendInput and its input event would wipe them (seen
        // once: local_wiped prev_len=1 right after replayed events=2).
        // Keep the local text on an empty push while not busy; the send path
        // clears the draft with busy=true, which still overwrites normally.
        // Known trade-off: the main window's 清空 button no longer clears a
        // non-empty overlay box (residue until the next send).
        if (!(event.payload.text.length === 0 && localText.value.length > 0 && !event.payload.busy)) {
          localText.value = event.payload.text
        }
      }
    }),
    await listen<ChatOverlayFocusRequest>(CHAT_OVERLAY_FOCUS_REQUEST_EVENT, (event) => {
      void reportFocus(event.payload)
    }),
    await listen<ChatOverlayHealthRequest>(CHAT_OVERLAY_HEALTH_REQUEST_EVENT, (event) => {
      void reportHealth(event.payload)
    }),
    await listen(CHAT_OVERLAY_BLUR_INPUT_EVENT, () => {
      // Release DOM focus before the overlay window is hidden so the IME
      // stops routing keystrokes to the (about-to-be-invisible) input.
      inputRef.value?.blur()
      composition.onBlur()
    }),
  )
})

onUnmounted(() => {
  document.documentElement.classList.remove('is-chat-overlay-document')
  for (const unlisten of unlisteners.splice(0)) unlisten()
})
</script>

<template>
  <main class="chat-overlay-view" aria-label="游戏内中文输入栏">
    <div class="chat-overlay-controls">
      <button class="overlay-drag-handle" type="button" title="拖动输入栏" aria-label="拖动输入栏" @pointerdown="startDragging">⋮</button>
      <div class="overlay-mode-segmented" aria-label="侧栏发言模式">
        <button type="button" :aria-pressed="state.mode === 'direct'" :disabled="state.busy" @click="setMode('direct')">直发</button>
        <button type="button" :aria-pressed="state.mode === 'translate'" :disabled="state.busy" @click="setMode('translate')">中译英</button>
      </div>
      <div class="overlay-input-frame" :class="{ 'is-composing': composition.isComposing.value, 'has-error': state.counterTone === 'error' }">
        <input
          ref="inputRef"
          :value="localText"
          type="text"
          autocomplete="off"
          spellcheck="false"
          :readonly="state.busy"
          :placeholder="state.mode === 'translate' ? '输入中文并翻译' : '输入中文消息'"
          @input="onInput"
          @keydown="onKeydown"
          @keyup="onKeyup"
          @compositionstart="composition.onCompositionStart"
          @compositionupdate="composition.onCompositionUpdate"
          @compositionend="composition.onCompositionEnd"
          @blur="composition.onBlur"
        />
        <span v-if="composition.isComposing.value" class="composition-badge">候选中</span>
        <span class="overlay-counter" :data-tone="state.counterTone">{{ state.characterCount }} / {{ state.characterLimit }}</span>
      </div>
      <button class="overlay-icon-button" type="button" :title="state.mode === 'translate' ? '翻译并发送' : '发送'" :aria-label="state.mode === 'translate' ? '翻译并发送' : '发送'" :disabled="!canSubmitLocally" @click="emitAction('submit')">↑</button>
      <!-- Cancel stays clickable while busy: during a send it aborts the transaction. -->
      <button class="overlay-icon-button" type="button" title="取消" aria-label="取消" @click="emitAction('cancel')">×</button>
      <button class="overlay-icon-button" type="button" title="展开助手" aria-label="展开助手" :disabled="state.busy" @click="emitAction('expand')">□</button>
    </div>
  </main>
</template>

<style scoped>
.chat-overlay-view { width: 100vw; height: 100vh; overflow: hidden; padding: 6px; border: 1px solid #59614e; border-radius: 6px; background: #10130f; color: var(--text); }
.chat-overlay-controls { display: grid; width: 100%; height: 100%; grid-template-columns: 14px 88px minmax(0, 1fr) repeat(3, 38px); align-items: center; gap: 6px; min-width: 0; }
.overlay-drag-handle { width: 14px; height: 38px; padding: 0; border: 0; border-radius: 3px; background: var(--text-muted); color: #10130f; cursor: move; font-size: 15px; line-height: 1; }
.overlay-drag-handle:hover { background: var(--accent); }
.overlay-mode-segmented { display: grid; width: 88px; height: 38px; grid-template-columns: repeat(2, minmax(0, 1fr)); padding: 3px; border: 1px solid var(--line-strong); border-radius: 5px; background: #171a15; }
.overlay-mode-segmented button { min-width: 0; padding: 0; border: 0; border-radius: 3px; background: transparent; color: var(--text-muted); cursor: pointer; font-size: 10px; font-weight: 700; letter-spacing: 0; }
.overlay-mode-segmented button[aria-pressed='true'] { background: var(--surface-raised); color: var(--accent); }
.overlay-mode-segmented button:disabled { cursor: default; opacity: .45; }
.overlay-input-frame { position: relative; min-width: 0; height: 44px; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 5px; background: #171a15; }
.overlay-input-frame:focus-within { border-color: var(--accent); box-shadow: inset 0 0 0 1px rgba(230, 200, 76, .28); }
.overlay-input-frame.is-composing { border-color: var(--info); }
.overlay-input-frame.has-error { border-color: var(--danger); }
.overlay-input-frame input { width: 100%; height: 100%; min-width: 0; padding: 0 104px 0 12px; border: 0; outline: 0; background: transparent; color: var(--text); caret-color: var(--accent); font-size: 15px; }
.overlay-input-frame input[readonly] { color: var(--text-secondary); }
.overlay-input-frame .composition-badge { position: absolute; top: 11px; right: 55px; padding: 2px 5px; border-radius: 4px; background: var(--info); color: #10130f; font-size: 9px; font-weight: 800; }
.overlay-counter { position: absolute; top: 15px; right: 8px; color: var(--text-muted); font-family: ui-monospace, Consolas, monospace; font-size: 9px; font-variant-numeric: tabular-nums; }
.overlay-counter[data-tone='warning'] { color: var(--accent); }
.overlay-counter[data-tone='error'] { color: var(--danger); }
.overlay-icon-button { display: grid; width: 38px; height: 38px; place-items: center; padding: 0; border: 1px solid var(--line-strong); border-radius: 5px; background: #1b1f18; color: var(--text-secondary); cursor: pointer; font-size: 17px; line-height: 1; }
.overlay-icon-button:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }
.overlay-icon-button:disabled { opacity: .38; }
</style>
