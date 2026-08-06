import { emitTo } from '@tauri-apps/api/event'
import { PhysicalPosition, PhysicalSize, Window } from '@tauri-apps/api/window'
import { watch, type Ref } from 'vue'

import type { GameForegroundEvent } from '@/composables/useGameOverlayWindow'
import type { NormalizedPosition, NormalizedRegion } from '@/types/ipc'
import {
  TRANSLATION_HUD_HIDE_EVENT,
  TRANSLATION_HUD_UPDATE_EVENT,
  TRANSLATION_HUD_WINDOW_LABEL,
  type TranslationHudItem,
  type TranslationHudPayload,
} from '@/types/translationHud'

const HUD_AUTO_HIDE_MS = 12_000
const HUD_VISIBLE_LINE_LIMIT = 4
const HUD_BASE_WIDTH = 720
const HUD_MIN_WIDTH = 440
const HUD_MAX_WIDTH = 860
const HUD_BASE_TOP_PADDING = 12
const HUD_LINE_HEIGHT = 44
const HUD_SCREEN_MARGIN = 12
const DEFAULT_CHAT_REGION: NormalizedRegion = { x: 0.02, y: 0.55, width: 0.55, height: 0.4 }

interface TranslationHudOptions {
  enabled: Ref<boolean>
  onError: (message: string) => void
}

export interface TranslationHudContext {
  foreground: GameForegroundEvent | null
  chatRegion: NormalizedRegion | null
  position: NormalizedPosition | null
}

export interface TranslationHudBounds {
  x: number
  y: number
  width: number
  height: number
}

function clamp(value: number, min: number, max: number): number {
  if (max < min) return min
  return Math.min(Math.max(value, min), max)
}

export function translationHudVisibleLineCount(items: TranslationHudItem[]): number {
  return Math.min(
    HUD_VISIBLE_LINE_LIMIT,
    Math.max(1, items.reduce((total, item) => total + item.lines.length, 0)),
  )
}

export function computeTranslationHudBounds(
  context: TranslationHudContext,
  visibleLineCount: number,
): TranslationHudBounds | null {
  const workArea = context.foreground?.state === 'game' ? context.foreground.workArea : null
  if (!workArea) return null

  const scale = Math.max(1, context.foreground?.scaleFactor ?? 1)
  const margin = Math.round(HUD_SCREEN_MARGIN * scale)
  const maxAvailableWidth = Math.max(Math.round(HUD_MIN_WIDTH * scale), workArea.width - margin * 2)
  const width = Math.round(
    clamp(
      HUD_BASE_WIDTH * scale,
      HUD_MIN_WIDTH * scale,
      Math.min(HUD_MAX_WIDTH * scale, maxAvailableWidth),
    ),
  )
  const height = Math.round((HUD_BASE_TOP_PADDING + HUD_LINE_HEIGHT * visibleLineCount) * scale)
  let x: number
  let y: number
  if (context.position) {
    const availableWidth = Math.max(0, workArea.width - width - margin * 2)
    const availableHeight = Math.max(0, workArea.height - height - margin * 2)
    x = workArea.x + margin + Math.round(availableWidth * clamp(context.position.x, 0, 1))
    y = workArea.y + margin + Math.round(availableHeight * clamp(context.position.y, 0, 1))
  } else {
    const region = context.chatRegion ?? DEFAULT_CHAT_REGION
    const preferredX = workArea.x + Math.round(workArea.width * region.x)
    const chatTop = workArea.y + Math.round(workArea.height * region.y)
    const preferredY = chatTop - height - margin
    x = Math.round(clamp(preferredX, workArea.x + margin, workArea.x + workArea.width - width - margin))
    y = Math.round(
      preferredY >= workArea.y + margin
        ? preferredY
        : clamp(chatTop + margin, workArea.y + margin, workArea.y + workArea.height - height - margin),
    )
  }

  return { x, y, width, height }
}

export function useTranslationHudWindow(options: TranslationHudOptions) {
  let hideTimer: number | null = null

  function clearHideTimer(): void {
    if (hideTimer === null) return
    window.clearTimeout(hideTimer)
    hideTimer = null
  }

  function scheduleHide(): void {
    clearHideTimer()
    hideTimer = window.setTimeout(() => {
      void hide()
    }, HUD_AUTO_HIDE_MS)
  }

  async function hudWindow(): Promise<Window | null> {
    return Window.getByLabel(TRANSLATION_HUD_WINDOW_LABEL)
  }

  async function show(items: TranslationHudItem[], context: TranslationHudContext): Promise<void> {
    if (!options.enabled.value || items.length === 0) {
      await hide()
      return
    }

    const bounds = computeTranslationHudBounds(context, translationHudVisibleLineCount(items))
    if (!bounds) {
      await hide()
      return
    }

    try {
      const window = await hudWindow()
      if (!window) return
      const payload: TranslationHudPayload = {
        items,
        visibleLineLimit: HUD_VISIBLE_LINE_LIMIT,
      }
      await emitTo(TRANSLATION_HUD_WINDOW_LABEL, TRANSLATION_HUD_UPDATE_EVENT, payload)
      await window.setFocusable(false)
      await window.setSkipTaskbar(true)
      await window.setAlwaysOnTop(true)
      await window.setIgnoreCursorEvents(true)
      await window.setSize(new PhysicalSize(bounds.width, bounds.height))
      await window.setPosition(new PhysicalPosition(bounds.x, bounds.y))
      await window.show()
      await window.unminimize()
      scheduleHide()
    } catch (error) {
      options.onError(error instanceof Error ? error.message : String(error))
    }
  }

  async function hide(): Promise<void> {
    clearHideTimer()
    try {
      await emitTo(TRANSLATION_HUD_WINDOW_LABEL, TRANSLATION_HUD_HIDE_EVENT)
      const window = await hudWindow()
      await window?.hide()
    } catch (error) {
      options.onError(error instanceof Error ? error.message : String(error))
    }
  }

  function dispose(): void {
    clearHideTimer()
    void hide()
  }

  watch(options.enabled, (enabled) => {
    if (!enabled) void hide()
  })

  return { show, hide, dispose }
}
