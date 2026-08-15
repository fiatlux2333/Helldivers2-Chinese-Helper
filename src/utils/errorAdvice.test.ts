import { errorMessageWithAdvice, errorTextWithAdvice, getErrorActions, getErrorAdvice, getErrorMessage } from './errorAdvice'
import type { IpcError } from '@/types/ipc'

function ipcError(code: IpcError['code'], message: string): IpcError {
  return {
    code,
    message,
    partialPrefixPossible: false,
    report: null,
  }
}

describe('error advice', () => {
  it('keeps the original error message', () => {
    expect(getErrorMessage(new Error('原始错误'))).toBe('原始错误')
    expect(getErrorMessage({ message: 'IPC 错误' })).toBe('IPC 错误')
  })

  it('adds direct permission mismatch advice', () => {
    const text = errorMessageWithAdvice(ipcError('INTEGRITY_INCOMPATIBLE', '权限不兼容'))

    expect(text).toContain('权限不兼容')
    expect(text).toContain('优先都不要管理员运行')
    expect(text).toContain('助手也必须管理员运行')
  })

  it('adds target recapture advice for target errors', () => {
    const text = errorMessageWithAdvice(ipcError('TARGET_CHANGED', '目标游戏窗口已变化'))

    expect(text).toContain('重新捕获目标')
    expect(text).toContain('刚切屏时等 1 秒')
  })

  it('adds OCR region advice for empty OCR results', () => {
    const text = errorMessageWithAdvice(ipcError('OCR_EMPTY', '没有识别到聊天文字'))

    expect(text).toContain('重新框选')
    expect(text).toContain('英文聊天文字')
    expect(getErrorActions(ipcError('OCR_EMPTY', '没有识别到聊天文字'))).toContain('calibrateRegion')
  })

  it('detects hotkey conflicts from text-only errors', () => {
    const advice = getErrorAdvice('HotKey already registered: Ctrl+Alt+1', { title: '喊话热键注册失败' })

    expect(advice).toContain('换一个热键')
    expect(getErrorActions('HotKey already registered: Ctrl+Alt+1', { title: '喊话热键注册失败' })).toContain('openSettings')
  })

  it('prefers target advice when chat translation fails because the game is not foreground', () => {
    const advice = getErrorAdvice('目标游戏窗口必须位于前台', { title: '聊天翻译失败' })

    expect(advice).toContain('重新捕获目标')
    expect(advice).not.toContain('重新框选')
  })

  it('does not duplicate existing suggestion text', () => {
    const text = errorTextWithAdvice('失败。建议：先重试。', { title: '发送失败' })

    expect(text).toBe('失败。建议：先重试。')
  })

  it('uses a default fallback for unknown structured errors', () => {
    const error = { message: '未知失败' }
    const text = errorMessageWithAdvice(error)

    expect(text).toContain('未知失败')
    expect(text).toContain('导出诊断日志')
    expect(getErrorActions(error)).toEqual(['exportLogs'])
  })

  it('special-cases partial text injection so users do not resend the whole line', () => {
    const error = ipcError('SEND_INPUT_PARTIAL', '文字可能只发送了部分前缀，请检查游戏输入框')
    error.partialPrefixPossible = true

    const text = errorMessageWithAdvice(error)

    expect(text).toContain('不要直接重发整段')
    expect(getErrorActions(error)).toEqual(['recaptureTarget', 'exportLogs'])
  })

  it('special-cases final submit failures as manual-enter checks', () => {
    const text = errorMessageWithAdvice(ipcError('FINAL_SUBMIT_FAILED', '最终 Enter 未完整注入'))

    expect(text).toContain('手动按 Enter')
    expect(getErrorActions(ipcError('FINAL_SUBMIT_FAILED', '最终 Enter 未完整注入'))).toContain('focusComposer')
  })

  it('guides stratagem direction mode failures to the stratagem page', () => {
    const advice = getErrorAdvice('方向键无法触发战备，WASD 可以', { title: '战备触发失败' })

    expect(advice).toContain('WASD')
    expect(advice).toContain('↑↓←→')
    expect(getErrorActions('方向键无法触发战备，WASD 可以', { title: '战备触发失败' })).toContain('openStratagem')
  })

  it('explains leftover background process reports in player language', () => {
    const advice = getErrorAdvice('退出工具后进程却依然留在后台，双击打开没反应')

    expect(advice).toContain('任务管理器')
    expect(advice).toContain('重新打开')
  })
})
