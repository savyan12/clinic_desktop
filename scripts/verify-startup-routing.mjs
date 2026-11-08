import { writeFile } from 'node:fs/promises'

const endpoint = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9223'
const appUrl = process.env.APP_URL || 'http://127.0.0.1:5173'
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
const pending = new Map()
const issues = []
const failedRequests = []

socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data)
  if (message.id && pending.has(message.id)) {
    const request = pending.get(message.id)
    pending.delete(message.id)
    if (message.error) request.reject(new Error(message.error.message))
    else request.resolve(message.result)
    return
  }
  if (message.method === 'Runtime.exceptionThrown') {
    issues.push(message.params.exceptionDetails.text)
  }
  if (message.method === 'Runtime.consoleAPICalled' && message.params.type === 'error') {
    issues.push(message.params.args.map((argument) => argument.value || argument.description || '').join(' '))
  }
  if (message.method === 'Network.loadingFailed') {
    if (!message.params.canceled) failedRequests.push(message.params.errorText)
  }
})

const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = nextId++
  pending.set(id, { resolve, reject })
  socket.send(JSON.stringify({ id, method, params }))
})

async function evaluate(expression) {
  const result = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.text)
  return result.result.value
}

async function state() {
  return JSON.parse(await evaluate(`JSON.stringify({
    path: location.pathname,
    search: location.search,
    text: document.body.innerText,
    appText: document.querySelector('#app')?.innerText || ''
  })`))
}

async function navigate(path, { expectedPath = path, text, search } = {}) {
  await send('Page.navigate', { url: `${appUrl}${path}` })
  await sleep(900)
  const current = await state()
  if (current.path !== expectedPath) throw new Error(`${path}: expected ${expectedPath}, received ${current.path}`)
  if (search !== undefined && current.search !== search) throw new Error(`${path}: expected query ${search}, received ${current.search}`)
  if (text && !current.text.includes(text)) throw new Error(`${path}: expected visible text “${text}”`)
  process.stdout.write(`PASS ${path} -> ${current.path}${current.search}\n`)
  return current
}

async function storeSession(token, user) {
  await evaluate(`localStorage.setItem('clinic-auth', ${JSON.stringify(JSON.stringify({ token, user }))})`)
}

async function verifyMetricCoverage(expectedCount, role) {
  let metrics
  for (let attempt = 0; attempt < 20; attempt += 1) {
    metrics = JSON.parse(await evaluate(`(() => {
      const container = document.querySelector('.dashboard-metrics')
      const cards = [...(container?.querySelectorAll('.stat-card') || [])]
      if (!container || !cards.length) return JSON.stringify({ count: cards.length })
      const containerRect = container.getBoundingClientRect()
      const cardRects = cards.map((card) => card.getBoundingClientRect())
      return JSON.stringify({
        count: cards.length,
        leftGap: Math.abs(Math.min(...cardRects.map((rect) => rect.left)) - containerRect.left),
        rightGap: Math.abs(Math.max(...cardRects.map((rect) => rect.right)) - containerRect.right),
        columns: getComputedStyle(container).gridTemplateColumns.split(' ').filter(Boolean).length
      })
    })()`))
    if (metrics.count) break
    await sleep(100)
  }
  if (metrics.count !== expectedCount || metrics.columns !== expectedCount || metrics.leftGap > 1 || metrics.rightGap > 1) {
    throw new Error(`${role} metric cards do not cover the full dashboard width: ${JSON.stringify(metrics)}`)
  }
  process.stdout.write(`PASS ${role} metric cards cover the complete dashboard width\n`)
}

async function verifyStatusChips() {
  let result
  for (let attempt = 0; attempt < 20; attempt += 1) {
    result = JSON.parse(await evaluate(`(() => {
      const chips = [...document.querySelectorAll('.status-chip:not(.status-chip-classic)')]
      const tones = [...new Set(chips.flatMap((chip) => [...chip.classList].filter((name) => name.startsWith('status-'))))]
      return JSON.stringify({
        count: chips.length,
        tones,
        valid: chips.every((chip) => {
          const style = getComputedStyle(chip)
          return style.borderTopWidth === '2px'
            && style.borderTopStyle === 'solid'
            && !chip.querySelector('.status-icon, .v-icon')
            && chip.scrollWidth <= chip.clientWidth
        })
      })
    })()`))
    if (result.count) break
    await sleep(100)
  }
  const requiredTones = ['status-info', 'status-success', 'status-danger', 'status-violet']
  if (!result.count || !result.valid || requiredTones.some((tone) => !result.tones.includes(tone))) {
    throw new Error(`Shared status chips do not match the reference treatment: ${JSON.stringify(result)}`)
  }
  process.stdout.write('PASS shared status chips use complete borders, no icons, and unclipped labels\n')
}

await Promise.all([
  send('Page.enable'),
  send('Runtime.enable'),
  send('Network.enable'),
])
await send('Network.setCacheDisabled', { cacheDisabled: true })
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false })

await send('Page.navigate', { url: `${appUrl}/login` })
await sleep(900)
await evaluate('localStorage.clear()')
await send('Page.navigate', { url: `${appUrl}/settings` })
let current
for (let attempt = 0; attempt < 20; attempt += 1) {
  await sleep(100)
  current = await state()
  if (current.appText.trim()) break
}
if (!current.appText.trim()) throw new Error('Cold startup remained empty instead of rendering a loading or route state')
process.stdout.write('PASS cold startup rendered without a manual reload\n')
await navigate('/settings', { expectedPath: '/login', text: 'تسجيل الدخول', search: '?redirect=/settings' })

const admin = { id: 1, name: 'مدير النظام', username: 'admin', role: 'admin', is_active: true }
await storeSession('mock-admin-token', admin)
await navigate('/', { text: 'تسجيل جلسة' })
await verifyMetricCoverage(4, 'admin')
const dashboardScreenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true })
await writeFile('/tmp/clinic-dashboard-metrics.png', Buffer.from(dashboardScreenshot.data, 'base64'))
await navigate('/sessions', { text: 'الجلسات' })
await verifyStatusChips()
const statusChipScreenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true })
await writeFile('/tmp/clinic-status-chips.png', Buffer.from(statusChipScreenshot.data, 'base64'))

const clicked = await evaluate(`(() => {
  const link = document.querySelector('a[href="/settings"]')
  if (!link) return false
  link.click()
  return true
})()`)
if (!clicked) throw new Error('Settings sidebar link was not found')
await sleep(900)
current = await state()
if (current.path !== '/settings' || !current.text.includes('إعدادات النظام')) {
  throw new Error(`First-click navigation failed: ${current.path}`)
}
process.stdout.write('PASS admin sidebar navigation opened settings on the first click\n')

await send('Page.reload', { ignoreCache: true })
await sleep(900)
current = await state()
if (current.path !== '/settings' || !current.text.includes('إعدادات النظام')) {
  throw new Error('Refreshing the nested settings route did not restore the page')
}
process.stdout.write('PASS nested route refresh\n')

const receptionist = { id: 2, name: 'موظف الاستقبال', username: 'reception', role: 'receptionist', is_active: true }
await storeSession('mock-reception-token', receptionist)
await navigate('/settings', { expectedPath: '/', text: 'هذه الصفحة متاحة لمدير النظام فقط.', search: '?forbidden=1' })
await verifyMetricCoverage(3, 'receptionist')

await storeSession('invalid-token', { ...admin, id: 999 })
await navigate('/settings', { expectedPath: '/login', text: 'تسجيل الدخول', search: '?redirect=/settings' })

await storeSession('mock-admin-token', admin)
await navigate('/settings', { text: 'إعدادات النظام' })

const screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true })
await writeFile('/tmp/clinic-startup-routing.png', Buffer.from(screenshot.data, 'base64'))

const typographyControlsUpdated = await evaluate(`(() => {
  const selectors = [
    'body_font_size-20', 'body_font_weight-600',
    'table_font_size-18', 'table_font_weight-600',
    'heading_font_size-36', 'heading_font_weight-400',
    'section_font_size-28', 'section_font_weight-400'
  ]
  return selectors.every((id) => {
    const button = document.querySelector('[data-segment-option="' + id + '"]')
    if (!button) return false
    button.click()
    return true
  })
})()`)
if (!typographyControlsUpdated) throw new Error('Typography controls could not be updated')

const typographySaved = await evaluate(`(() => {
  const button = [...document.querySelectorAll('button')]
    .find((element) => element.textContent.includes('حفظ الإعدادات'))
  if (!button) return false
  button.click()
  return true
})()`)
if (!typographySaved) throw new Error('Settings save button was not found')
await sleep(900)

const savedTokens = await evaluate(`JSON.stringify({
  body: document.documentElement.style.getPropertyValue('--fs-body'),
  table: document.documentElement.style.getPropertyValue('--fs-table'),
  page: document.documentElement.style.getPropertyValue('--fs-page'),
  pageWeight: document.documentElement.style.getPropertyValue('--fw-page'),
  section: document.documentElement.style.getPropertyValue('--fs-section'),
  sectionWeight: document.documentElement.style.getPropertyValue('--fw-section')
})`)
if (savedTokens !== JSON.stringify({ body: '20px', table: '18px', page: '36px', pageWeight: '400', section: '28px', sectionWeight: '400' })) {
  throw new Error(`Saved typography did not update the root design tokens: ${savedTokens}`)
}

await evaluate(`document.querySelector('a[href="/"]')?.click()`)
await sleep(900)
const appliedTypography = JSON.parse(await evaluate(`JSON.stringify({
  path: location.pathname,
  headingSize: getComputedStyle(document.querySelector('.page-header h1')).fontSize,
  headingWeight: getComputedStyle(document.querySelector('.page-header h1')).fontWeight,
  sectionSize: getComputedStyle(document.querySelector('.financial-card-heading h2')).fontSize,
  sectionWeight: getComputedStyle(document.querySelector('.financial-card-heading h2')).fontWeight
})`))
if (appliedTypography.path !== '/' || appliedTypography.headingSize !== '36px' || appliedTypography.headingWeight !== '400' || appliedTypography.sectionSize !== '28px' || appliedTypography.sectionWeight !== '400') {
  throw new Error(`Saved typography was not applied to shared application components: ${JSON.stringify(appliedTypography)}`)
}
process.stdout.write('PASS saved typography applied immediately to shared page and section headings\n')

socket.close()

const actionableIssues = issues.filter((issue) => !issue.includes('Download the Vue Devtools extension'))
if (actionableIssues.length) throw new Error(`Console errors: ${JSON.stringify(actionableIssues)}`)
if (failedRequests.length) throw new Error(`Failed browser requests: ${JSON.stringify(failedRequests)}`)

process.stdout.write('Startup and routing verification passed with no console errors or failed requests. Screenshots: /tmp/clinic-dashboard-metrics.png, /tmp/clinic-status-chips.png, /tmp/clinic-startup-routing.png\n')
