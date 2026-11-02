export function isDesktopRuntime() {
  return Boolean(globalThis.__TAURI_INTERNALS__)
}

export async function invokeDesktop(command, payload = {}) {
  if (!isDesktopRuntime()) throw new Error('هذه العملية متاحة في تطبيق سطح المكتب فقط.')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke(command, payload)
}

export function desktopErrorMessage(cause, fallback = 'تعذر إتمام العملية المحلية.') {
  if (typeof cause === 'string' && cause.trim()) return cause
  if (cause?.message) return cause.message
  return fallback
}
