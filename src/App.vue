<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'

import TargetDiagnosticPanel from '@/components/TargetDiagnosticPanel.vue'
import { useCompositionLatch } from '@/composables/useCompositionLatch'
import { useInputHistory } from '@/composables/useInputHistory'
import { useRestoreHotkey } from '@/composables/useRestoreHotkey'
import {
  beginProbeSession,
  cancelSession,
  getTargetDiagnostic,
  injectProbeText,
  isTauriRuntime,
  normalizeTarget,
  previewText,
} from '@/services/tauriApi'
import type { TargetDiagnostic } from '@/types/ipc'

const CHARACTER_LIMIT = 100
const desktopRuntime = isTauriRuntime()

type NoticeTone = 'idle' | 'working' | 'success' | 'error'

const inputRef = ref<HTMLInputElement | null>(null)
const text = ref('')
const target = ref<TargetDiagnostic | null>(null)
const isRefreshing = ref(false)
const isCapturing = ref(false)
const isSending = ref(false)
const activeGeneration = ref<string | null>(null)
const captureToken = ref(0)
const submitOnEnterRelease = ref(false)
const noticeTone = ref<NoticeTone>('idle')
const noticeTitle = ref('等待输入')
const noticeMessage = ref(
  '先在游戏中打开聊天框，再回到这里输入。助手只填入文字，不会代替你发送。最小化后可用自定义热键唤回。',
)

const composition = useCompositionLatch()
const history = useInputHistory(50)

const restoreHotkey = useRestoreHotkey({
  isSending,
  isCapturing,
  onRestored: async (label) => {
    await restoreAssistantWindow({ focus: true })
    if (noticeTone.value !== 'error') {
      setNotice(
        'idle',
        '助手已唤回',
        `已用 ${label} 恢复前台。再连按一次同一热键可最小化交还游戏；不会自动发送。`,
      )
    }
  },
  onYielded: async (label) => {
    await yieldAssistantWindow()
    if (noticeTone.value !== 'error') {
      setNotice(
        'idle',
        '助手已收起',
        `已连按 ${label} 两次，助手已最小化。在游戏中操作后可再按一次热键唤回。`,
      )
    }
  },
  onError: (title, message) => {
    setNotice('error', title, message)
  },
  onHotkeyChanged: (label) => {
    setNotice(
      'success',
      '热键已更新',
      `热键已设为 ${label}：按一次唤回，约 0.5 秒内连按两次最小化。不会自动捕获或发送。`,
    )
  },
})

const characterCount = computed(() => Array.from(text.value).length)
const hasText = computed(() => text.value.trim().length > 0)
const isOverLimit = computed(() => characterCount.value > CHARACTER_LIMIT)
const canCapture = computed(
  () => desktopRuntime && !isSending.value && !composition.isComposing.value && !composition.isLatched.value,
)
const canSubmit = computed(
  () =>
    hasText.value &&
    !isOverLimit.value &&
    !isCapturing.value &&
    !isSending.value &&
    !composition.isComposing.value &&
    !composition.isLatched.value &&
    activeGeneration.value !== null &&
    target.value?.valid === true,
)
const counterTone = computed(() => {
  if (isOverLimit.value) return 'error'
  if (characterCount.value >= CHARACTER_LIMIT * 0.8) return 'warning'
  return 'normal'
})

function focusInput(): void {
  void nextTick(() => inputRef.value?.focus())
}

/** Bring the assistant back. Pass focus=false only when HD2 must keep keyboard focus. */
async function restoreAssistantWindow(options?: { focus?: boolean }): Promise<void> {
  if (!isTauriRuntime()) return

  const shouldFocus = options?.focus !== false
  const appWindow = getCurrentWindow()
  await appWindow.unminimize()
  if (shouldFocus) {
    await appWindow.setFocus()
    focusInput()
  }
}

/** Minimize the assistant so the game (or previous app) can reclaim foreground. */
async function yieldAssistantWindow(): Promise<void> {
  if (!isTauriRuntime()) return
  await getCurrentWindow().minimize()
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    return String((error as { message: unknown }).message)
  }
  return String(error)
}

function setNotice(tone: NoticeTone, title: string, message: string): void {
  noticeTone.value = tone
  noticeTitle.value = title
  noticeMessage.value = message
}

function setText(value: string): void {
  text.value = value
  history.resetBrowsing()
  if (noticeTone.value === 'error') {
    setNotice('idle', '草稿已保留', '你可以修改内容后再次尝试。')
  }
}

function onInput(event: Event): void {
  const element = event.target as HTMLInputElement
  setText(element.value)
}

async function clearDraft(): Promise<void> {
  if (isSending.value) return

  const generation = activeGeneration.value
  captureToken.value += 1
  text.value = ''
  submitOnEnterRelease.value = false
  history.resetBrowsing()
  composition.reset()
  activeGeneration.value = null
  target.value = null

  try {
    await cancelSession(generation ?? undefined)
  } catch {
    // A capture command that has not created a session yet needs no backend cleanup.
  }
  setNotice('idle', '已取消', '草稿和目标会话已清空，没有开始新的填字事务。')
  focusInput()
}

async function refreshTarget(): Promise<TargetDiagnostic> {
  isRefreshing.value = true
  try {
    const diagnostic = await getTargetDiagnostic()
    target.value = diagnostic
    if (!diagnostic.valid) {
      setNotice(
        diagnostic.status === 'unsupported_platform' ? 'idle' : 'error',
        diagnostic.status === 'unsupported_platform' ? '浏览器预览模式' : '目标尚不可用',
        diagnostic.message,
      )
    }
    return diagnostic
  } catch (error) {
    const message = errorMessage(error)
    setNotice('error', '目标检测失败', message)
    throw error
  } finally {
    isRefreshing.value = false
  }
}

async function captureTarget(): Promise<void> {
  if (
    isCapturing.value ||
    isSending.value ||
    composition.isComposing.value ||
    composition.isLatched.value
  )
    return

  if (!isTauriRuntime()) {
    await refreshTarget()
    return
  }

  const token = captureToken.value + 1
  const previousGeneration = activeGeneration.value
  captureToken.value = token
  isCapturing.value = true
  activeGeneration.value = null
  target.value = null
  try {
    await cancelSession(previousGeneration ?? undefined)
  } catch {
    // No active session is a valid starting state for a new capture.
  }
  if (captureToken.value !== token) {
    isCapturing.value = false
    return
  }
  setNotice('working', '准备捕获游戏窗口', '助手将最小化。请在 4 秒内切回已打开聊天框的 HD2。')

  try {
    await new Promise((resolve) => window.setTimeout(resolve, 900))
    if (captureToken.value !== token) return

    const appWindow = getCurrentWindow()
    await appWindow.minimize()
    await new Promise((resolve) => window.setTimeout(resolve, 3_500))
    if (captureToken.value !== token) return

    const session = await beginProbeSession()
    if (captureToken.value !== token) {
      try {
        await cancelSession(session.generation)
      } catch {
        // A newer session must not be cancelled by this stale capture.
      }
      return
    }
    activeGeneration.value = session.generation
    target.value = normalizeTarget(session.diagnostic, session.integrity)

    await restoreAssistantWindow()
    setNotice(
      'success',
      '目标已锁定',
      '请在助手中编辑文字。填入成功后可连续输入多句，无需重复捕获；仅目标失效时需重新捕获。',
    )
    focusInput()
  } catch (error) {
    if (captureToken.value !== token) {
      try {
        await restoreAssistantWindow()
      } catch {
        // A cancelled capture must not replace the user's current status.
      }
      return
    }

    activeGeneration.value = null
    target.value = null
    try {
      await restoreAssistantWindow()
    } catch {
      // The original capture error is more useful than a secondary window error.
    }
    const message = errorMessage(error)
    setNotice('error', '目标捕获失败', `${message} 请重新打开游戏聊天框后再试。`)
  } finally {
    isCapturing.value = false
  }
}

async function submit(): Promise<void> {
  if (!canSubmit.value || composition.isComposing.value || composition.isLatched.value) return

  const generation = activeGeneration.value
  if (generation === null) return

  const stableText = text.value
  isSending.value = true
  setNotice(
    'working',
    '正在准备填入',
    '助手将最小化并把焦点交还已锁定的 HD2；填入后请在游戏中手动按 Enter 发送。',
  )

  try {
    const preview = await previewText(stableText)
    if (!preview.cleanedText.trim()) {
      setNotice('error', '没有可填入的文字', '请移除控制字符并输入至少一个可见字符。')
      return
    }
    if (preview.scalarCount > CHARACTER_LIMIT) {
      setNotice('error', '文字过长', `当前 ${preview.scalarCount} 个字符，最多允许 ${CHARACTER_LIMIT} 个。`)
      return
    }

    if (isTauriRuntime()) {
      await getCurrentWindow().minimize()
      await new Promise((resolve) => window.setTimeout(resolve, 180))
    }

    const result = await injectProbeText(generation, preview.cleanedText)

    if (!result.ok) {
      const partialHint = result.error?.partialPrefixPossible
        ? '游戏聊天框中可能已有部分前缀，请先检查并手动清理。'
        : '完整草稿已保留，请重新捕获游戏窗口后重试。'
      activeGeneration.value = null
      target.value = null
      try {
        await restoreAssistantWindow()
      } catch {
        // The injection result remains the primary error.
      }
      setNotice('error', '填入未完成', `${result.message} ${partialHint}`)
      return
    }

    history.add(preview.cleanedText)
    text.value = ''
    // Keep the locked target session so the next sentence does not require recapture.
    // Inject still revalidates the same foreground window before every batch.
    // Do NOT restore/focus the assistant after a successful fill: that would steal
    // keyboard focus from the game chat and block the user from pressing Enter to send.
    // Leave the helper minimized; return via the custom hotkey (or taskbar) for the next sentence.
    setNotice(
      'success',
      '文字已填入游戏',
      `请直接在游戏中按 Enter 发送（助手不会自动发送）。发送后按 ${restoreHotkey.hotkeyLabel.value} 唤回助手继续下一句；连按两次可再收起。无需重新捕获。`,
    )
  } catch (error) {
    activeGeneration.value = null
    target.value = null
    try {
      await restoreAssistantWindow()
    } catch {
      // Preserve the injection error even if the assistant window cannot be restored.
    }
    const message = errorMessage(error)
    setNotice('error', '操作失败', `${message} 完整草稿已保留，请重新捕获目标。`)
  } finally {
    isSending.value = false
  }
}

function onKeydown(event: KeyboardEvent): void {
  // While rebinding the global hotkey, keep Esc for cancel and avoid drafting side-effects.
  if (restoreHotkey.isRecording.value) {
    if (event.key === 'Escape') {
      event.preventDefault()
      restoreHotkey.cancelRecording()
    }
    return
  }

  if (isSending.value) {
    event.preventDefault()
    return
  }

  if (isCapturing.value) {
    event.preventDefault()
    if (event.key === 'Escape') void clearDraft()
    return
  }

  if (composition.shouldBlockKeydown(event)) return

  if (event.key === 'Escape') {
    event.preventDefault()
    void clearDraft()
    return
  }

  if (event.key === 'ArrowUp') {
    event.preventDefault()
    text.value = history.browseOlder(text.value)
    return
  }

  if (event.key === 'ArrowDown') {
    event.preventDefault()
    text.value = history.browseNewer(text.value)
    return
  }

  if (event.key === 'Enter' && !event.repeat) {
    event.preventDefault()
    submitOnEnterRelease.value = !(event.ctrlKey || event.altKey || event.shiftKey || event.metaKey)
  }
}

function onKeyup(event: KeyboardEvent): void {
  const blocked = composition.shouldBlockKeyup(event)
  if (event.key === 'Enter' && submitOnEnterRelease.value) {
    submitOnEnterRelease.value = false
    if (!blocked) void submit()
  }
}

onMounted(() => {
  focusInput()
  if (!isTauriRuntime()) {
    void refreshTarget()
    return
  }
  void restoreHotkey.ensureRegistered()
})
</script>

<template>
  <main class="app-shell">
    <div class="background-grid" aria-hidden="true"></div>

    <section class="workspace" aria-labelledby="app-title">
      <header class="app-header">
        <div class="brand-mark" aria-hidden="true">
          <span>H2</span>
        </div>
        <div class="brand-copy">
          <p class="eyebrow">HELLDIVERS 2 · INPUT PROBE</p>
          <h1 id="app-title">中文输入助手</h1>
        </div>
        <span class="build-tag">安全探针</span>
      </header>

      <div class="content-grid">
        <section class="composer-panel" aria-labelledby="composer-heading">
          <div class="section-heading composer-heading">
            <div>
              <p class="eyebrow">MESSAGE BUFFER</p>
              <h2 id="composer-heading">准备文字</h2>
            </div>
            <span class="shortcut-hint">
              <template v-for="(part, index) in restoreHotkey.hotkeyParts.value" :key="`hint-${part}-${index}`">
                <kbd>{{ part }}</kbd>
                <span v-if="index < restoreHotkey.hotkeyParts.value.length - 1">+</span>
              </template>
              唤回 / 连按收起
              ·
              <kbd>Esc</kbd> 清空
            </span>
          </div>

          <label class="input-label" for="message-input">单行中文输入</label>
          <div
            class="input-frame"
            :class="{
              'is-composing': composition.isComposing.value,
              'has-error': isOverLimit,
            }"
          >
            <input
              id="message-input"
              ref="inputRef"
              :value="text"
              type="text"
              inputmode="text"
              autocomplete="off"
              spellcheck="false"
              :readonly="isSending || isCapturing"
              :aria-invalid="isOverLimit"
              :aria-errormessage="isOverLimit ? 'input-error' : undefined"
              placeholder="输入要填入游戏聊天框的中文……"
              aria-describedby="input-help character-counter hotkey-help"
              @input="onInput"
              @keydown="onKeydown"
              @keyup="onKeyup"
              @compositionstart="composition.onCompositionStart"
              @compositionupdate="composition.onCompositionUpdate"
              @compositionend="composition.onCompositionEnd"
              @blur="composition.onBlur"
            />
            <span v-if="composition.isComposing.value" class="composition-badge">候选中</span>
            <span id="character-counter" class="character-counter" :data-tone="counterTone">
              {{ characterCount }} / {{ CHARACTER_LIMIT }}
            </span>
          </div>
          <p id="input-help" class="input-help">
            <span><kbd>Enter</kbd> 填入游戏</span>
            <span>
              <template v-for="(part, index) in restoreHotkey.hotkeyParts.value" :key="`help-${part}-${index}`">
                <kbd>{{ part }}</kbd>
                <span v-if="index < restoreHotkey.hotkeyParts.value.length - 1">+</span>
              </template>
              唤回 · 连按两次收起
            </span>
            <span><kbd>↑</kbd><kbd>↓</kbd> 浏览最近 {{ history.entries.value.length }}/50 条</span>
          </p>

          <div class="hotkey-settings" aria-labelledby="hotkey-heading">
            <div class="hotkey-settings-row">
              <div>
                <p class="eyebrow" id="hotkey-heading">RESTORE HOTKEY</p>
                <p id="hotkey-help" class="hotkey-current">
                  当前唤回：
                  <template v-for="(part, index) in restoreHotkey.hotkeyParts.value" :key="`cur-${part}-${index}`">
                    <kbd>{{ part }}</kbd>
                    <span v-if="index < restoreHotkey.hotkeyParts.value.length - 1">+</span>
                  </template>
                </p>
              </div>
              <div class="hotkey-actions">
                <button
                  class="secondary-button hotkey-button"
                  type="button"
                  :disabled="isSending || isCapturing || restoreHotkey.isApplying.value"
                  :aria-pressed="restoreHotkey.isRecording.value"
                  @click="
                    restoreHotkey.isRecording.value
                      ? restoreHotkey.cancelRecording()
                      : restoreHotkey.startRecording()
                  "
                >
                  {{
                    restoreHotkey.isRecording.value
                      ? '取消录制'
                      : restoreHotkey.isApplying.value
                        ? '注册中…'
                        : '自定义热键'
                  }}
                </button>
                <button
                  class="ghost-button hotkey-button"
                  type="button"
                  :disabled="isSending || isCapturing || restoreHotkey.isApplying.value"
                  @click="void restoreHotkey.resetDefault()"
                >
                  恢复默认
                </button>
              </div>
            </div>
            <p class="hotkey-record-hint" :data-recording="restoreHotkey.isRecording.value">
              <template v-if="restoreHotkey.isRecording.value">
                正在录制：请按住 Ctrl / Alt / Win + 主键（Esc 取消）
                <span v-if="restoreHotkey.pendingLabel.value"> · 已识别 {{ restoreHotkey.pendingLabel.value }}</span>
              </template>
              <template v-else>
                按一次热键唤回前台；约 0.5 秒内连按两次则最小化交还游戏。热键不捕获、不填字、不发送。
              </template>
            </p>
          </div>

          <div class="notice" :data-tone="noticeTone" role="status" aria-live="polite">
            <span class="notice-signal" aria-hidden="true"></span>
            <div>
              <strong>{{ noticeTitle }}</strong>
              <p>{{ noticeMessage }}</p>
            </div>
          </div>

          <button class="primary-button" type="button" :disabled="!canSubmit" @click="submit">
            <span>{{ isSending ? '正在填入…' : '填入游戏' }}</span>
            <svg aria-hidden="true" viewBox="0 0 24 24">
              <path d="M5 12h13M13 6l6 6-6 6" />
            </svg>
          </button>

          <p id="input-error" v-if="isOverLimit" class="field-error" role="alert">
            已超过 {{ CHARACTER_LIMIT }} 字限制，请删去 {{ characterCount - CHARACTER_LIMIT }} 个字符。
          </p>
        </section>

        <TargetDiagnosticPanel
          :diagnostic="target"
          :loading="isCapturing"
          :available="canCapture"
          @capture="captureTarget"
        />
      </div>

      <footer class="safety-note">
        <span class="safety-line" aria-hidden="true"></span>
        <p>只向已验证的前台目标填入 Unicode 文字；不会自动注入 Enter 或 Esc。</p>
      </footer>
    </section>
  </main>
</template>
