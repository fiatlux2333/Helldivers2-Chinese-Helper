import { describe, expect, it } from 'vitest'

import {
  computeTranslationHudBounds,
  translationHudVisibleLineCount,
  type TranslationHudContext,
} from '@/composables/useTranslationHudWindow'
import type { TranslationHudItem } from '@/types/translationHud'

const gameContext: TranslationHudContext = {
  foreground: {
    state: 'game',
    workArea: { x: 0, y: 0, width: 1920, height: 1080 },
    scaleFactor: 1,
  },
  chatRegion: { x: 0.02, y: 0.55, width: 0.55, height: 0.4 },
  position: null,
}

describe('translation HUD window', () => {
  it('places the HUD above the calibrated chat region when game work area is known', () => {
    const bounds = computeTranslationHudBounds(gameContext, 2)

    expect(bounds).toEqual({ x: 38, y: 482, width: 720, height: 100 })
  })

  it('clamps the HUD inside the game work area on small displays', () => {
    const bounds = computeTranslationHudBounds(
      {
        foreground: {
          state: 'game',
          workArea: { x: 100, y: 50, width: 500, height: 360 },
          scaleFactor: 1,
        },
        chatRegion: { x: 0.8, y: 0.1, width: 0.2, height: 0.2 },
        position: null,
      },
      4,
    )

    expect(bounds).toEqual({ x: 112, y: 98, width: 476, height: 188 })
  })

  it('does not compute a visible HUD position outside game foreground', () => {
    expect(
      computeTranslationHudBounds(
        {
          foreground: { state: 'assistant', workArea: null, scaleFactor: null },
          chatRegion: null,
          position: null,
        },
        1,
      ),
    ).toBeNull()
  })

  it('limits visible HUD lines to four', () => {
    const items: TranslationHudItem[] = [
      {
        id: 1,
        translatedAt: 1,
        lines: [
          { speaker: 'A', translatedMessage: '1' },
          { speaker: 'B', translatedMessage: '2' },
          { speaker: 'C', translatedMessage: '3' },
          { speaker: 'D', translatedMessage: '4' },
          { speaker: 'E', translatedMessage: '5' },
        ],
      },
    ]

    expect(translationHudVisibleLineCount(items)).toBe(4)
  })

  it('maps a custom normalized position inside the available work area', () => {
    const bounds = computeTranslationHudBounds(
      { ...gameContext, position: { x: 1, y: 1 } },
      2,
    )

    expect(bounds).toEqual({ x: 1188, y: 968, width: 720, height: 100 })
  })
})
