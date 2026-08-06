<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

import { isTauriRuntime } from '@/services/tauriApi'
import {
  TRANSLATION_HUD_HIDE_EVENT,
  TRANSLATION_HUD_UPDATE_EVENT,
  type TranslationHudItem,
  type TranslationHudPayload,
} from '@/types/translationHud'

const desktopRuntime = isTauriRuntime()
const items = ref<TranslationHudItem[]>(
  desktopRuntime
    ? []
    : [
      {
        id: 1,
        translatedAt: Date.now(),
        lines: [
          { speaker: 'Striker', translatedMessage: '虫群从东侧压过来了，先清重甲。' },
          { speaker: 'Eagle', translatedMessage: '补给还有 30 秒，别现在撤。' },
        ],
      },
    ],
)
const visibleLineLimit = ref(4)
const unlisteners: UnlistenFn[] = []

const visibleLines = computed(() =>
  items.value
    .flatMap((item) =>
      item.lines.map((line, lineIndex) => ({
        id: `${item.id}-${lineIndex}`,
        speaker: line.speaker,
        translatedMessage: line.translatedMessage,
      })),
    )
    .slice(0, visibleLineLimit.value),
)
onMounted(async () => {
  document.documentElement.classList.add('is-translation-hud-document')
  if (!desktopRuntime) return
  const hudWindow = getCurrentWindow()
  await hudWindow.setIgnoreCursorEvents(true).catch(() => undefined)
  await hudWindow.setFocusable(false).catch(() => undefined)
  await hudWindow.setSkipTaskbar(true).catch(() => undefined)
  await hudWindow.setAlwaysOnTop(true).catch(() => undefined)
  unlisteners.push(
    await listen<TranslationHudPayload>(TRANSLATION_HUD_UPDATE_EVENT, (event) => {
      items.value = event.payload.items
      visibleLineLimit.value = event.payload.visibleLineLimit
    }),
    await listen(TRANSLATION_HUD_HIDE_EVENT, () => {
      items.value = []
    }),
  )
})

onUnmounted(() => {
  document.documentElement.classList.remove('is-translation-hud-document')
  for (const unlisten of unlisteners.splice(0)) unlisten()
})
</script>

<template>
  <main class="translation-hud-view" aria-label="游戏聊天译文 HUD">
    <section v-if="visibleLines.length" class="translation-hud-stack">
      <article v-for="line in visibleLines" :key="line.id" class="translation-hud-line">
        <strong v-if="line.speaker">{{ line.speaker }}</strong>
        <p>{{ line.translatedMessage }}</p>
      </article>
    </section>
  </main>
</template>

<style scoped>
.translation-hud-view { display: grid; width: 100vw; height: 100vh; align-items: end; overflow: hidden; padding: 6px 8px; background: transparent; pointer-events: none; }
.translation-hud-stack { display: grid; min-width: 0; align-content: end; gap: 4px; overflow: hidden; }
.translation-hud-line { display: grid; min-width: 0; min-height: 40px; grid-template-columns: auto minmax(0, 1fr); align-items: start; gap: 9px; padding: 5px 12px 6px 10px; border-left: 3px solid rgba(235, 207, 80, .94); border-radius: 2px; background: rgba(4, 7, 6, .56); box-shadow: 0 3px 12px rgba(0, 0, 0, .24); }
.translation-hud-line strong { max-width: 112px; overflow: hidden; padding-top: 2px; color: #f0d35d; font-size: 13px; font-weight: 850; line-height: 1.35; text-overflow: ellipsis; white-space: nowrap; text-shadow: 0 1px 3px #000; }
.translation-hud-line p { display: -webkit-box; min-width: 0; margin: 0; overflow: hidden; color: #fffef2; font-size: 15px; font-weight: 760; line-height: 1.35; overflow-wrap: anywhere; text-shadow: 0 1px 3px #000, 0 0 8px rgba(0, 0, 0, .88); -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.translation-hud-line p:first-child { grid-column: 1 / -1; }
</style>
