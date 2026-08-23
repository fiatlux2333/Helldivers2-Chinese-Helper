export const CHAT_OVERLAY_WINDOW_LABEL = 'chat-overlay'
export const CHAT_OVERLAY_STATE_EVENT = 'chat-overlay:state'
export const CHAT_OVERLAY_FOCUS_REQUEST_EVENT = 'chat-overlay:focus-request'
export const CHAT_OVERLAY_FOCUS_RESULT_EVENT = 'chat-overlay:focus-result'
export const CHAT_OVERLAY_HEALTH_REQUEST_EVENT = 'chat-overlay:health-request'
export const CHAT_OVERLAY_HEALTH_RESULT_EVENT = 'chat-overlay:health-result'
export const CHAT_OVERLAY_INPUT_EVENT = 'chat-overlay:input'
export const CHAT_OVERLAY_MODE_EVENT = 'chat-overlay:mode'
export const CHAT_OVERLAY_ACTION_EVENT = 'chat-overlay:action'
export const CHAT_OVERLAY_POSITION_EVENT = 'chat-overlay:position'
export const CHAT_OVERLAY_BLUR_INPUT_EVENT = 'chat-overlay:blur-input'
export const SINGLE_INSTANCE_RESTORE_EVENT = 'single-instance-restore'

export type ChatOverlayMode = 'direct' | 'translate'
export type ChatOverlayCounterTone = 'normal' | 'warning' | 'error'
export type ChatOverlayAction =
  | 'submit'
  | 'cancel'
  | 'expand'
  | 'historyOlder'
  | 'historyNewer'

export interface ChatOverlayStatePayload {
  text: string
  mode: ChatOverlayMode
  busy: boolean
  canSubmit: boolean
  characterCount: number
  characterLimit: number
  counterTone: ChatOverlayCounterTone
}

export interface ChatOverlayFocusRequest {
  requestId: number
}

export interface ChatOverlayFocusResult {
  requestId: number
  focused: boolean
  error: string | null
}

export interface ChatOverlayHealthRequest {
  requestId: number
}

export interface ChatOverlayHealthResult {
  requestId: number
  documentReady: boolean
  inputReady: boolean
  focused: boolean
  error: string | null
}

export interface ChatOverlayPosition {
  x: number
  y: number
}
