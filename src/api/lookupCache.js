const entries = new Map()
const lifetime = 30_000
let scope

// Cache picker options only; balances, reports, and patient records stay fresh.
export function cachedLookup(name, loader) {
  const currentScope = localStorage.getItem('clinic-auth')
  if (scope !== currentScope) {
    entries.clear()
    scope = currentScope
  }

  const existing = entries.get(name)
  if (existing && (existing.pending || existing.expiresAt > Date.now())) return existing.promise

  const entry = { pending: true, expiresAt: 0 }
  entry.promise = Promise.resolve().then(loader).then((options) => {
    entry.pending = false
    entry.expiresAt = Date.now() + lifetime
    return options
  }).catch((error) => {
    if (entries.get(name) === entry) entries.delete(name)
    throw error
  })
  entries.set(name, entry)
  return entry.promise
}

export async function invalidateLookupAfter(name, request) {
  const response = await request
  entries.delete(name)
  return response
}
