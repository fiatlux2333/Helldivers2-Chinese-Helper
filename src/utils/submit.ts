export type SubmitIntent = 'send' | 'fill'

export interface SubmitKeyEvent {
  key: string
  repeat: boolean
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
  metaKey: boolean
}

export function submitIntentFromKeydown(event: SubmitKeyEvent): SubmitIntent | null {
  if (event.key !== 'Enter' || event.repeat || event.altKey || event.shiftKey || event.metaKey) {
    return null
  }
  return event.ctrlKey ? 'fill' : 'send'
}
