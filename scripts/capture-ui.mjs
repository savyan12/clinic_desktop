import { writeFile } from 'node:fs/promises'

const endpoint = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9223'
const appUrl = process.env.APP_URL || 'http://127.0.0.1:5173'
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
socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data)
  if (!message.id || !pending.has(message.id)) return
  const { resolve, reject } = pending.get(message.id)
  pending.delete(message.id)
  if (message.error) reject(new Error(message.error.message))
  else resolve(message.result)
})
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = nextId++
  pending.set(id, { resolve, reject })
  socket.send(JSON.stringify({ id, method, params }))
})
const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds))

await Promise.all([send('Page.enable'), send('Runtime.enable')])
await send('Runtime.evaluate', {
  expression: `localStorage.setItem('clinic-auth', ${JSON.stringify(JSON.stringify({ token: 'mock-admin-token', user: { id: 1, name: 'مدير النظام', username: 'admin', role: 'admin', is_active: true } }))})`,
})

async function capture(path, name, width = 1440, height = 1000) {
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false })
  await send('Page.navigate', { url: `${appUrl}${path}` })
  await sleep(1000)
  const state = await send('Runtime.evaluate', {
    expression: `JSON.stringify({ path: location.pathname, title: document.title, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, width: document.documentElement.scrollWidth, viewport: document.documentElement.clientWidth })`,
    returnByValue: true,
  })
  const screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true })
  await writeFile(`/tmp/${name}.png`, Buffer.from(screenshot.data, 'base64'))
  process.stdout.write(`${name}: ${state.result.value}\n`)
}

await capture('/', 'clinic-dashboard-v2')
await capture('/sessions', 'clinic-sessions-v2')
await capture('/patients', 'clinic-patients-v2')
await capture('/subscriptions', 'clinic-subscriptions-v2')
await capture('/doctors', 'clinic-doctors-v2')
await capture('/', 'clinic-dashboard-v2-1024', 1024, 768)
socket.close()
