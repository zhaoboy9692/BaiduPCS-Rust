import { afterEach, describe, expect, it, vi } from 'vitest'
import { copyText, tokenStatus } from '../directlinkClipboard'
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals() })
describe('copyText', () => {
  it('uses native clipboard when available', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    await expect(copyText('secret')).resolves.toBe(true)
    expect(writeText).toHaveBeenCalledWith('secret')
  })
  it('falls back on HTTP and removes secret textarea', async () => {
    vi.stubGlobal('navigator', {})
    Object.defineProperty(document, 'execCommand', { configurable: true, value: vi.fn(() => true) })
    await expect(copyText('secret')).resolves.toBe(true)
    expect(document.querySelector('textarea')).toBeNull()
  })
  it('does not claim success when fallback fails', async () => {
    vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn().mockRejectedValue(new Error()) } })
    Object.defineProperty(document, 'execCommand', { configurable: true, value: vi.fn(() => false) })
    await expect(copyText('secret')).resolves.toBe(false)
  })
})
describe('tokenStatus', () => {
  it('expiry boundary is expired, not permanent', () => {
    expect(tokenStatus({ enabled: true, revoked_at: null, expires_at: 100 }, 100)).toBe('已过期')
    expect(tokenStatus({ enabled: true, revoked_at: null, expires_at: null }, 100)).toBe('启用')
  })
  it('revoked takes precedence and disabled stays disabled', () => {
    expect(tokenStatus({ enabled: false, revoked_at: 99, expires_at: null }, 100)).toBe('已撤销')
    expect(tokenStatus({ enabled: false, revoked_at: null, expires_at: null }, 100)).toBe('已禁用')
  })
})
