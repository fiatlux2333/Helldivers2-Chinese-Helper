export interface TargetIdentity {
  hwnd: string
  processId: number
  threadId: number
  processCreationTime: string
}

export interface RustTargetDiagnostic {
  supported: boolean
  identity: TargetIdentity | null
  title: string
  titleMatches: boolean
  isWindow: boolean
  visible: boolean
  minimized: boolean
  cloaked: boolean
}

export interface IntegrityDiagnostic {
  supported: boolean
  currentLevel: string | null
  targetLevel: string | null
  compatible: boolean | null
}

export interface TextPreview {
  cleanedText: string
  scalarCount: number
  utf16Batches: number[][]
}

export interface ProbeSession {
  generation: string
  diagnostic: RustTargetDiagnostic
  integrity: IntegrityDiagnostic
}

export interface InjectionReport {
  attemptedBatches: number
  successfulEvents: number
  failedBatchIndex: number | null
  partialPrefixPossible: boolean
  keyStateUncertain: boolean
  submitAttempted: boolean
  submitCompleted: boolean
}

export interface DiagnosticLogsView {
  path: string
  content: string
}

export interface NormalizedRegion {
  x: number
  y: number
  width: number
  height: number
}

export interface QuickShout {
  label: string
  message: string
  hotkey: string
}

export interface TranslationSettingsView {
  apiUrl: string
  proxyUrl: string
  apiKeyConfigured: boolean
  model: string
  ocrLanguage: string
  captureHotkey: string
  chatRegion: NormalizedRegion | null
  incomingPrompt: string
  outgoingPrompt: string
  quickShoutFocusDelayMs: number
  quickShouts: QuickShout[]
}

export interface TranslationSettingsUpdate {
  apiUrl: string
  proxyUrl: string
  apiKey?: string
  model: string
  ocrLanguage: string
  captureHotkey: string
  chatRegion: NormalizedRegion | null
  incomingPrompt: string
  outgoingPrompt: string
  quickShoutFocusDelayMs: number
  quickShouts: QuickShout[]
}

export interface OcrLanguage {
  tag: string
  displayName: string
  nativeName: string
}

export interface CalibrationPreview {
  dataUrl: string
  width: number
  height: number
}

export interface ChatTranslationResult {
  lines: ChatTranslationLine[]
  messageOcrLanguage: string
  speakerOcrLanguage: string
}

export interface ChatTranslationLine {
  speaker: string
  originalMessage: string
  translatedMessage: string
}

export type SessionPhase =
  | 'idle'
  | 'waitingGameEnterRelease'
  | 'openingPanel'
  | 'editing'
  | 'waitingSubmitKeyRelease'
  | 'restoringTarget'
  | 'injecting'
  | 'failed'

export interface SessionSnapshot {
  generation: string
  phase: SessionPhase
  target: TargetIdentity | null
  draft: string
  lastError: string | null
}

export type IpcErrorCode =
  | 'UNSUPPORTED_PLATFORM'
  | 'TARGET_NOT_MATCHED'
  | 'TARGET_CHANGED'
  | 'WINDOW_UNAVAILABLE'
  | 'WINDOW_NOT_VISIBLE'
  | 'WINDOW_MINIMIZED'
  | 'WINDOW_CLOAKED'
  | 'INTEGRITY_INCOMPATIBLE'
  | 'TEXT_EMPTY'
  | 'TEXT_TOO_LONG'
  | 'SEND_INPUT_PARTIAL'
  | 'FINAL_SUBMIT_FAILED'
  | 'API_CONFIGURATION'
  | 'API_REQUEST_FAILED'
  | 'API_RESPONSE_INVALID'
  | 'SETTINGS_STORAGE_FAILED'
  | 'CAPTURE_FAILED'
  | 'OCR_UNAVAILABLE'
  | 'OCR_EMPTY'
  | 'INVALID_CAPTURE_REGION'
  | 'INVALID_SESSION'
  | 'SUBMIT_KEY_STILL_DOWN'
  | 'INPUT_STATE_UNCERTAIN'
  | 'INTERNAL_STATE'

export interface IpcError {
  code: IpcErrorCode
  message: string
  partialPrefixPossible: boolean
  report: InjectionReport | null
}

export type PlatformKind = 'windows' | 'browser' | 'unknown'
export type TargetStatus =
  | 'ready'
  | 'not_found'
  | 'title_mismatch'
  | 'not_visible'
  | 'minimized'
  | 'cloaked'
  | 'permission_mismatch'
  | 'permission_unknown'
  | 'unsupported_platform'
  | 'error'

export interface TargetDiagnostic {
  platform: PlatformKind
  status: TargetStatus
  valid: boolean
  matched: boolean
  title: string | null
  visible: boolean
  minimized: boolean
  cloaked: boolean
  identity: TargetIdentity | null
  integrity: IntegrityDiagnostic | null
  message: string
  checkedAt: number
}

export interface InjectionResult {
  ok: boolean
  message: string
  report: InjectionReport | null
  error: IpcError | null
}
