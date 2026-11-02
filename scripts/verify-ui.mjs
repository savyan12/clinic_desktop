import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { createRenderer, h, nextTick, reactive, ref, ssrContextKey } from 'vue'
import { createPinia } from 'pinia'

// Exercise real Vue setup/watchers without a browser or clinic database.
const storage = new Map()
globalThis.localStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, value),
  removeItem: (key) => storage.delete(key),
}
const server = await createServer({
  define: { 'import.meta.env.VITE_USE_MOCK': JSON.stringify('false') },
  ssr: { noExternal: ['vuetify'] },
  server: { middlewareMode: true, hmr: false, watch: null },
  appType: 'custom',
  logLevel: 'silent',
})
const renderer = createRenderer({
  createElement: (type) => ({ type, children: [] }),
  createText: (text) => ({ text }),
  createComment: (text) => ({ text }),
  insert: (child, parent) => { child.parent = parent; parent.children.push(child) },
  remove: (child) => { child.parent.children = child.parent.children.filter((item) => item !== child) },
  setText: (node, text) => { node.text = text },
  setElementText: (node, text) => { node.text = text },
  parentNode: (node) => node.parent,
  nextSibling: () => null,
  patchProp: (node, name, previous, value) => { node[name] = value },
})
const apps = []
function mount(component, initialProps = {}) {
  const props = reactive(initialProps)
  const testComponent = { ...component, render: () => null }
  let vnode
  const app = renderer.createApp({ render: () => (vnode = h(testComponent, props)) })
  app.use(createPinia())
  app.provide(ssrContextKey, { modules: new Set() })
  app.mount({ children: [] })
  apps.push(app)
  return { props, get state() { return vnode.component.setupState }, unmount: () => app.unmount() }
}
const deferred = () => {
  let resolve, reject
  const promise = new Promise((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
async function flush() { for (let i = 0; i < 15; i++) await nextTick() }
let passed = 0
async function check(name, run) {
  await run()
  passed += 1
  process.stdout.write(`PASS ${name}\n`)
}

try {
  const { cachedLookup, invalidateLookupAfter } = await server.ssrLoadModule('/src/api/lookupCache.js')
  const { useDialogOptions } = await server.ssrLoadModule('/src/composables/useDialogOptions.js')
  const { default: http } = await server.ssrLoadModule('/src/api/client.js')
  const { default: SessionDialog } = await server.ssrLoadModule('/src/components/sessions/NewSessionDialog.vue')
  const { default: Autocomplete } = await server.ssrLoadModule('/src/components/common/PatientAutocomplete.vue')
  const patient = (id) => ({ id, name: `Patient ${id}`, phone: `090000${id}`, subscriptions: [], wallet_balance: '0.000', debt_total: '0.000' })
  const calls = []
  let handle = async () => ({ data: [] })
  http.defaults.adapter = async (config) => {
    calls.push(config)
    return { config, data: await handle(config), status: 200, statusText: 'OK', headers: {} }
  }

  await check('Concurrent option requests share one fetch', async () => {
    let fetches = 0
    const loader = async () => { fetches += 1; return ['doctor'] }
    const results = await Promise.all([cachedLookup('dedupe', loader), cachedLookup('dedupe', loader)])
    assert.equal(fetches, 1)
    assert.deepEqual(results[0], results[1])
  })
  await check('Successful edits invalidate picker options', async () => {
    assert.equal(await cachedLookup('edit', async () => 'old'), 'old')
    await invalidateLookupAfter('edit', Promise.resolve())
    assert.equal(await cachedLookup('edit', async () => 'new'), 'new')
  })
  await check('Failed option requests can be retried', async () => {
    await assert.rejects(cachedLookup('retry', async () => { throw new Error('Unavailable') }))
    assert.equal(await cachedLookup('retry', async () => 'recovered'), 'recovered')
  })
  await check('Expired options refresh after 30 seconds', async () => {
    await cachedLookup('expiry', async () => 'old')
    const now = Date.now
    Date.now = () => now() + 31_000
    try { assert.equal(await cachedLookup('expiry', async () => 'fresh'), 'fresh') } finally { Date.now = now }
  })
  await check('Changing the signed-in user discards old options', async () => {
    await cachedLookup('scope', async () => 'first user')
    localStorage.setItem('clinic-auth', JSON.stringify({ token: 'test-only-token', user: { role: 'admin' } }))
    assert.equal(await cachedLookup('scope', async () => 'second user'), 'second user')
  })
  await check('A late response cannot replace a reopened dialog', async () => {
    const requests = []
    const dialog = mount({ setup() {
      const open = ref(false)
      return { open, ...useDialogOptions(() => open.value, () => { const result = deferred(); requests.push(result); return result.promise }) }
    } })
    assert.equal(requests.length, 0)
    dialog.state.open = true
    await flush()
    dialog.state.open = false
    await flush()
    dialog.state.open = true
    await flush()
    requests[1].resolve('latest')
    await flush()
    requests[0].resolve('outdated')
    await flush()
    assert.equal(dialog.state.data, 'latest')
    assert.equal(dialog.state.loading, false)
    dialog.unmount()
  })
  await check('Closed session dialogs do not fetch options', async () => {
    const before = calls.length
    const dialog = mount(SessionDialog, { modelValue: false })
    await flush()
    assert.equal(calls.length, before)
    dialog.unmount()
  })
  await check('Changing patients cancels and ignores the previous account request', async () => {
    const requests = new Map()
    handle = (config) => {
      if (!config.url.startsWith('/patients/')) return { data: [] }
      const result = deferred()
      requests.set(config.url, { ...result, signal: config.signal })
      return result.promise
    }
    const dialog = mount(SessionDialog, { modelValue: false })
    dialog.props.modelValue = true
    await flush()
    dialog.state.form.patient_id = 1
    dialog.state.selectPatient(patient(1))
    await flush()
    dialog.state.form.patient_id = 2
    dialog.state.selectPatient(patient(2))
    await flush()
    assert.equal(requests.get('/patients/1').signal.aborted, true)
    requests.get('/patients/2').resolve({ data: patient(2) })
    await flush()
    requests.get('/patients/1').resolve({ data: patient(1) })
    await flush()
    assert.equal(dialog.state.patientDetails.id, 2)
    assert.equal(dialog.state.patientError, '')
    dialog.unmount()
  })
  await check('Reopening a session starts clean with a new idempotency key', async () => {
    handle = async () => ({ data: [] })
    const dialog = mount(SessionDialog, { modelValue: false })
    dialog.props.modelValue = true
    await flush()
    const previousKey = dialog.state.key
    dialog.state.step = 3
    dialog.state.patientDetails = patient(1)
    dialog.props.modelValue = false
    await flush()
    dialog.props.modelValue = true
    await flush()
    assert.equal(dialog.state.step, 1)
    assert.equal(dialog.state.patientDetails, null)
    assert.notEqual(dialog.state.key, previousKey)
    dialog.unmount()
  })
  await check('A preselected patient outside the first search page stays visible without a fetch', async () => {
    const before = calls.length
    const field = mount(Autocomplete, { modelValue: 999, initialPatient: patient(999) })
    await flush()
    assert.equal(field.state.selected.name, 'Patient 999')
    assert.equal(calls.length, before)
    field.unmount()
  })
  await check('Out-of-order search results cannot overwrite the latest patients', async () => {
    const requests = []
    handle = (config) => { const result = deferred(); requests.push({ ...result, signal: config.signal }); return result.promise }
    const field = mount(Autocomplete, { modelValue: null })
    await flush()
    field.state.search = 'latest'
    await flush()
    await new Promise((resolve) => setTimeout(resolve, 280))
    await flush()
    assert.equal(requests[0].signal.aborted, true)
    requests[1].resolve({ data: [patient(2)] })
    await flush()
    requests[0].resolve({ data: [patient(1)] })
    await flush()
    assert.equal(field.state.patients[0].id, 2)
    assert.equal(field.state.loadError, '')
    field.unmount()
  })
  process.stdout.write(`UI state verification passed: ${passed} checks. No browser or clinic database was used.\n`)
} finally {
  for (const app of apps) if (app._instance) app.unmount()
  await server.close()
}
