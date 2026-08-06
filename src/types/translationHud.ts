export const TRANSLATION_HUD_WINDOW_LABEL = 'translation-hud'
export const TRANSLATION_HUD_UPDATE_EVENT = 'translation-hud:update'
export const TRANSLATION_HUD_HIDE_EVENT = 'translation-hud:hide'

export interface TranslationHudLine {
  speaker: string
  translatedMessage: string
}

export interface TranslationHudItem {
  id: number
  lines: TranslationHudLine[]
  translatedAt: number
}

export interface TranslationHudPayload {
  items: TranslationHudItem[]
  visibleLineLimit: number
}
