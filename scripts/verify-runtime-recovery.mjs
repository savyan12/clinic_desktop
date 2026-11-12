import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { createSSRApp } from 'vue'
import { renderToString } from '@vue/server-renderer'
import { createPinia } from 'pinia'
import { createMemoryHistory, createRouter } from 'vue-router'

// Render the real views without opening Tauri or connecting to any database.
const server = await createServer({
  server: { middlewareMode: true, hmr: false },
  appType: 'custom',
  ssr: { noExternal: ['vuetify'] },
})

try {
  const { useDesktopStore } = await server.ssrLoadModule('/src/stores/desktop.js')
  const { default: ActivationView } = await server.ssrLoadModule('/src/views/activation/ActivationView.vue')
  const { default: LoginView } = await server.ssrLoadModule('/src/views/auth/LoginView.vue')
  const { createVuetify } = await server.ssrLoadModule('vuetify')
  const { default: http } = await server.ssrLoadModule('/src/api/client.js')
  http.defaults.adapter = () => { throw new Error('Recovery view verification must not make API requests') }

  async function render(view, runtime) {
    const pinia = createPinia()
    const desktop = useDesktopStore(pinia)
    desktop.applyRuntime({
      desktop: true, activated: false, ready: false,
      api_base_url: 'http://127.0.0.1:1/api/v1',
      error: null, recovery_required: false, recovery_notice: null,
      ...runtime,
    })
    const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/', component: view }] })
    await router.push('/')
    await router.isReady()
    const app = createSSRApp(view)
    app.use(pinia).use(router).use(createVuetify({ ssr: true }))
    return renderToString(app)
  }

  const newInstall = await render(ActivationView, {})
  assert.ok(newInstall.includes('إنشاء وتفعيل النظام'))
  assert.ok(newInstall.includes('تفعيل جديد'))

  for (const runtime of [
    { recovery_required: true, error: 'ملف إعدادات التشغيل المحلي غير صالح.' },
    { recovery_required: true, error: 'ملفات التشغيل مفقودة مع وجود بيانات MySQL.' },
    { error: 'نسخة أخرى من النظام تستخدم مجلد البيانات نفسه.' },
    { activated: true, error: 'تعذر تشغيل الخدمات المحلية.' },
    { activated: true },
  ]) {
    const html = await render(ActivationView, runtime)
    assert.ok(html.includes('إعادة المحاولة'))
    for (const action of ['تفعيل جديد', 'إنشاء وتفعيل النظام', 'التحقق والاستعادة والتفعيل']) {
      assert.ok(!html.includes(action), `Existing installation must hide action: ${action}`)
    }
  }

  const notice = 'استُعيدت إعدادات التشغيل من النسخة السليمة.'
  const login = await render(LoginView, { activated: true, ready: true, recovery_notice: notice })
  assert.ok(login.includes(notice))
  assert.ok(login.includes('تسجيل الدخول'))
  process.stdout.write('PASS fresh activation, five runtime failure states, and automatic recovery notice\n')
} finally {
  await server.close()
}
