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
