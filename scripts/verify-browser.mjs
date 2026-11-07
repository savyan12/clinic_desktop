import { writeFile } from 'node:fs/promises'

const endpoint = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9223'
const appUrl = process.env.APP_URL || 'http://127.0.0.1:5173'
const adminPassword = process.env.ADMIN_PASSWORD || 'Admin123!'
const receptionistPassword = process.env.RECEPTION_PASSWORD || 'Reception123!'
const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds))

const tabs = await fetch(`${endpoint}/json`).then((response) => response.json())
const page = tabs.find((tab) => tab.type === 'page')
if (!page) throw new Error('No Chrome page target is available')

const socket = new WebSocket(page.webSocketDebuggerUrl)
await new Promise((resolve, reject) => {
  socket.addEventListener('open', resolve, { once: true })
  socket.addEventListener('error', reject, { once: true })
})

let nextId = 1
let currentRoute = 'startup'
const pending = new Map()
const issues = []

socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data)
  if (message.id && pending.has(message.id)) {
    const { resolve, reject } = pending.get(message.id)
    pending.delete(message.id)
    if (message.error) reject(new Error(message.error.message))
    else resolve(message.result)
    return
  }
  if (message.method === 'Runtime.exceptionThrown') {
    issues.push({ route: currentRoute, type: 'exception', text: message.params.exceptionDetails.text })
  }
  if (message.method === 'Runtime.consoleAPICalled' && ['error', 'warning'].includes(message.params.type)) {
    const text = message.params.args.map((argument) => argument.value || argument.description || '').join(' ')
    issues.push({ route: currentRoute, type: message.params.type, text })
  }
  if (message.method === 'Log.entryAdded' && ['error', 'warning'].includes(message.params.entry.level)) {
    issues.push({ route: currentRoute, type: message.params.entry.level, text: message.params.entry.text })
  }
})

const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = nextId++
  pending.set(id, { resolve, reject })
  socket.send(JSON.stringify({ id, method, params }))
})

await Promise.all([send('Page.enable'), send('Runtime.enable'), send('Log.enable'), send('Network.enable')])
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1050, deviceScaleFactor: 1, mobile: false })

async function navigate(path, expectedText, expectedPath = path) {
  currentRoute = path
  await send('Page.navigate', { url: `${appUrl}${path}` })
  await sleep(1200)
  const result = await send('Runtime.evaluate', {
    expression: `JSON.stringify({ path: location.pathname, title: document.title, text: document.body.innerText.slice(0, 12000) })`,
    returnByValue: true,
  })
  const state = JSON.parse(result.result.value)
  if (expectedPath && state.path !== expectedPath) throw new Error(`${path}: expected path ${expectedPath}, received ${state.path}`)
  if (expectedText && !state.text.includes(expectedText)) throw new Error(`${path}: expected visible text “${expectedText}”`)
  process.stdout.write(`PASS ${path} → ${state.title}\n`)
}

async function login(username, password) {
  const result = await send('Runtime.evaluate', {
    expression: `(async () => {
      const response = await fetch('/api/v1/auth/login', {
        method: 'POST',
        headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
        body: JSON.stringify(${JSON.stringify({ username, password })}),
      })
      const body = await response.json()
      if (!response.ok) return { ok: false, status: response.status, body }
      localStorage.setItem('clinic-auth', JSON.stringify(body.data))
      return { ok: true, role: body.data.user.role }
    })()`,
    awaitPromise: true,
    returnByValue: true,
  })
  const state = result.result.value
  if (!state?.ok) throw new Error(`Login failed for ${username}: HTTP ${state?.status} ${JSON.stringify(state?.body)}`)
  process.stdout.write(`PASS login ${username} (${state.role})\n`)
}

await send('Storage.clearDataForOrigin', { origin: new URL(appUrl).origin, storageTypes: 'all' })
await navigate('/login', 'تسجيل الدخول')
issues.length = 0

await login('admin', adminPassword)

const adminRoutes = [
  ['/', 'فتح جلسة جديدة'],
  ['/sessions', 'الجلسات المفتوحة والمغلقة والملغاة'],
  ['/sessions/1', 'التسوية المالية'],
  ['/patients', 'البحث في السجلات'],
  ['/patients/1', 'الحساب المالي'],
  ['/subscriptions', 'متابعة الجلسات المتبقية'],
  ['/subscriptions/1', 'سجل الاستخدام'],
  ['/doctors', 'المستحق المكتسب منفصل'],
  ['/doctors/1', 'الجلسات المحتسبة'],
  ['/services', 'الأسعار الحالية'],
  ['/sponsors', 'المؤسسات المسؤولة'],
  ['/sponsors/1', 'كشف الجلسات والخدمات'],
  ['/users', 'إدارة حسابات موظفي الاستقبال'],
  ['/expenses', 'تسجيل المصروفات المدفوعة فعليًا'],
  ['/reports/sessions', 'تقرير الجلسات'],
  ['/reports/collections', 'تقرير المقبوضات'],
  ['/reports/debts', 'تقرير الديون'],
  ['/reports/doctor-dues', 'تقرير مستحقات الأطباء'],
  ['/reports/subscriptions', 'تقرير الاشتراكات'],
  ['/reports/expenses', 'تقرير المشتريات والإيجار'],
  ['/audit-log', 'سجل للقراءة فقط'],
  ['/settings', 'وضوح النص وأحجام الخطوط'],
  ['/print/session/1', 'إيصال جلسة'],
  ['/print/statement/patient/1', 'كشف حساب مريض'],
  ['/print/statement/sponsor/1', 'كشف حساب جهة راعية'],
  ['/print/report/sessions', 'تقرير الجلسات'],
]

for (const [path, text] of adminRoutes) await navigate(path, text)

await navigate('/', 'فتح جلسة جديدة')
const screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true })
await writeFile('/tmp/clinic-dashboard.png', Buffer.from(screenshot.data, 'base64'))

await login('reception', receptionistPassword)
await navigate('/', 'تحتاج متابعة قريباً')
await navigate('/patients', 'البحث في السجلات')
await navigate('/sessions', 'الجلسات المفتوحة والمغلقة والملغاة')
await navigate('/subscriptions', 'متابعة الجلسات المتبقية')
await navigate('/settings', 'فتح جلسة جديدة', '/')

socket.close()

const actionableIssues = issues.filter((issue) => !issue.text.includes('Download the Vue Devtools extension'))
if (actionableIssues.length) {
  process.stderr.write(`${JSON.stringify(actionableIssues, null, 2)}\n`)
  process.exitCode = 1
} else {
  process.stdout.write('Browser route verification passed with no console warnings or errors. Screenshot: /tmp/clinic-dashboard.png\n')
}
