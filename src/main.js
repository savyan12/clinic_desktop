import '@fontsource/ibm-plex-sans-arabic/400.css'
import '@fontsource/ibm-plex-sans-arabic/600.css'
import '@mdi/font/css/materialdesignicons.css'
import 'vuetify/styles'
import '@/styles/tokens.css'
import '@/styles/main.css'
import '@/styles/print.css'

import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { configureApiErrorHandlers } from './api/client'
import router from './router'
import { useAuthStore } from './stores/auth'
import { useDesktopStore } from './stores/desktop'
import { useUiStore } from './stores/ui'
import vuetify from './plugins/vuetify'

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
app.use(vuetify)

const auth = useAuthStore(pinia)
const desktop = useDesktopStore(pinia)
const ui = useUiStore(pinia)

configureApiErrorHandlers({
  unauthorized: async () => {
    const redirect = router.currentRoute.value.fullPath
    auth.clear()
    ui.showSnackbar('انتهت جلسة الدخول، يرجى تسجيل الدخول مجددًا.', 'error')
    if (router.currentRoute.value.name !== 'login') {
      await router.replace({ name: 'login', query: redirect && redirect !== '/' ? { redirect } : {} })
    }
  },
  forbidden: (error) => ui.showSnackbar(error.message || 'ليس لديك صلاحية لتنفيذ هذا الإجراء.', 'error'),
})

await desktop.initialize()
if (desktop.activated && desktop.ready) await auth.restore()
app.use(router)
await router.isReady()
app.mount('#app')
