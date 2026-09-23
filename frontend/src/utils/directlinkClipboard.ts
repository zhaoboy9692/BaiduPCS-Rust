export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
      return true
    }
  } catch { /* HTTP / denied permission: try the user-gesture fallback. */ }
  const element = document.createElement('textarea')
  const previous = document.activeElement as HTMLElement | null
  element.value = text
  element.style.position = 'fixed'
  element.style.opacity = '0'
  document.body.appendChild(element)
  try {
    element.focus()
    element.select()
    return document.execCommand?.('copy') === true
  } catch {
    return false
  } finally {
    element.remove()
    previous?.focus()
  }
}

export function tokenStatus(token: { enabled: boolean; revoked_at: number | null; expires_at: number | null }, now = Date.now() / 1000): string {
  if (token.revoked_at !== null) return '已撤销'
  if (!token.enabled) return '已禁用'
  if (token.expires_at !== null && token.expires_at <= now) return '已过期'
  return '启用'
}
