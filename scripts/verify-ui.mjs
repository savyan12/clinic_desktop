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
  const {
    auditActionLabel,
    auditEntityLabel,
    buildAuditChanges,
    formatAuditValue,
  } = await server.ssrLoadModule('/src/constants/audit.js')
  const {
    getReportDefinition,
    reportCellLabel,
    reportSummaryItems,
  } = await server.ssrLoadModule('/src/constants/reports.js')
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

  await check('Audit actions used by both backends have centralized Arabic labels', async () => {
    const actionKeys = [
      'debt_payment_recorded', 'debt_settled', 'doctor_created', 'doctor_paid',
      'doctor_payout_created', 'doctor_rate_changed',
      'expense_created', 'expense_corrected', 'expense_cancelled',
      'doctor_updated', 'opening_debt_created', 'patient_created', 'patient_updated',
      'payment_recorded', 'payment_refunded', 'service_created', 'service_price_changed',
      'service_updated', 'session_cancelled', 'session_closed', 'session_corrected',
      'session_created', 'session_services_added', 'session_updated', 'settings_updated',
      'sponsor_created', 'sponsor_updated', 'subscription_cancelled', 'subscription_corrected',
      'subscription_created', 'user_created',
      'user_password_reset', 'user_updated', 'wallet_topped_up', 'wallet_topup_refunded',
    ]
    actionKeys.forEach((key) => {
      const label = auditActionLabel(key)
      assert.match(label, /[\u0600-\u06ff]/)
      assert.equal(label.includes('_'), false)
    })
    assert.equal(auditActionLabel('invoice_created'), 'إنشاء سجل')
    assert.equal(auditActionLabel('unmapped_operation'), 'عملية أخرى')
    assert.equal(auditEntityLabel('unmapped_entity'), 'سجل')
  })

  await check('Audit comparison includes changed fields only and localizes their values', async () => {
    const changes = buildAuditChanges({
      action: 'service_updated',
      old_values: { name: 'علاج طبيعي', price: '10.000', is_active: true, status: 'open' },
      new_values: { name: 'علاج طبيعي', price: '12.500', is_active: false, status: 'closed' },
    })
    assert.equal(changes.showOld, true)
    assert.equal(changes.showNew, true)
    assert.deepEqual(changes.rows.map((row) => row.key), ['price', 'is_active', 'status'])
    assert.equal(changes.rows[0].oldValue, '10 د.ل')
    assert.equal(changes.rows[0].newValue, '12.5 د.ل')
    assert.equal(changes.rows[1].oldValue, 'نعم')
    assert.equal(changes.rows[1].newValue, 'لا')
    assert.equal(changes.rows[2].newValue, 'مغلقة')
  })

  await check('Audit comparison adapts creation, deletion, and nested services without JSON', async () => {
    const created = buildAuditChanges({ new_values: { name: 'مريض جديد', gender: 'male' } })
    assert.equal(created.showOld, false)
    assert.equal(created.showNew, true)
    assert.equal(created.rows[1].newValue, 'ذكر')

    const deleted = buildAuditChanges({ old_values: { name: 'خدمة قديمة' }, new_values: null })
    assert.equal(deleted.showOld, true)
    assert.equal(deleted.showNew, false)

    const nested = buildAuditChanges({
      old_values: { services: [{ service_id: 1, name: 'جلسة', price: '20.000' }] },
      new_values: { services: [{ service_id: 1, name: 'جلسة', price: '25.000' }] },
    })
    assert.deepEqual(nested.rows.map((row) => row.key), ['services.0.price'])
    assert.match(nested.rows[0].label, /الخدمات.*البند 1.*السعر/)
    const renderedValues = `${nested.rows[0].oldValue}${nested.rows[0].newValue}`
    assert.equal(['{', '}', '[', ']', '"'].some((character) => renderedValues.includes(character)), false)
    assert.equal(formatAuditValue('', 'note'), '—')
    assert.equal(formatAuditValue('unexpected', 'status'), 'قيمة أخرى')
    assert.equal(formatAuditValue('open', 'status', 'debt'), 'مفتوح')
  })

  await check('Report screens and print views share the required Arabic columns and formatters', async () => {
    const expectedColumns = {
      sessions: ['التاريخ', 'المريض', 'الطبيب', 'الخدمات', 'القيمة', 'الحالة'],
      collections: ['التاريخ', 'النوع', 'المريض', 'الطريقة', 'المبلغ'],
      debts: ['المرجع', 'المريض', 'المسؤول', 'الأصل', 'المسدد', 'المتبقي', 'الحالة'],
      'doctor-dues': ['الطبيب', 'الجلسات', 'المكتسب', 'المصروف', 'المتبقي'],
      subscriptions: ['المريض', 'الاشتراك', 'البداية', 'المتبقي', 'الجلسات والطبيب المشرف', 'سعر الاشتراك', 'الحالة'],
      expenses: ['التاريخ', 'النوع', 'الوصف', 'المستلم', 'المرجع', 'طريقة الدفع', 'المبلغ', 'الحالة'],
    }
    Object.entries(expectedColumns).forEach(([type, labels]) => {
      assert.deepEqual(getReportDefinition(type).columns.map((column) => column.label), labels)
    })

    const subscription = getReportDefinition('subscriptions')
    const row = { name: 'تأهيل متكامل', remaining: 8, total_price: '200.000' }
    assert.equal(reportCellLabel(subscription.columns[1], row), 'تأهيل متكامل')
    assert.equal(reportCellLabel(subscription.columns[3], row), '8 جلسة')
    assert.equal(reportCellLabel(subscription.columns[5], row), '200 د.ل')
    assert.deepEqual(
      reportSummaryItems('collections', { received: '80.000', refunds: '5.000', net: '75.000' })
        .map(({ label, money }) => [label, money]),
      [['إجمالي المقبوضات', true], ['إجمالي الاسترجاعات', true], ['صافي المقبوضات', true]],
    )
    assert.deepEqual(
      reportSummaryItems('expenses', { count: 2, total_purchases: '80.000', total_rent: '500.000', total_expenses: '580.000' })
        .map(({ label, money }) => [label, Boolean(money)]),
      [['عدد العمليات', false], ['إجمالي المشتريات', true], ['إجمالي الإيجار', true], ['إجمالي المصروفات', true]],
    )
  })

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
