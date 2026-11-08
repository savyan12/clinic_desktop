import '@fontsource/ibm-plex-sans-arabic/400.css'
import '@fontsource/ibm-plex-sans-arabic/500.css'
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
import { useStartupStore } from './stores/startup'
import { useUiStore } from './stores/ui'
import vuetify from './plugins/vuetify'

if (localStorage.getItem('clinic-accent') === 'emerald') document.documentElement.dataset.accent = 'emerald'

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
app.use(vuetify)

const auth = useAuthStore(pinia)
const startup = useStartupStore(pinia)
const ui = useUiStore(pinia)

app.config.errorHandler = (error, _instance, info) => {
  console.error(`Unhandled Vue error (${info}).`, error)
  if (!router.currentRoute.value.name) startup.fail(error)
}

let unauthorizedNavigation = null

configureApiErrorHandlers({
  unauthorized: async () => {
    if (unauthorizedNavigation) return unauthorizedNavigation
    unauthorizedNavigation = (async () => {
      const redirect = router.currentRoute.value.fullPath
      auth.clear()
      ui.showSnackbar('انتهت جلسة الدخول، يرجى تسجيل الدخول مجددًا.', 'error')
      if (router.currentRoute.value.name !== 'login') {
        await router.replace({ name: 'login', query: redirect && redirect !== '/' ? { redirect } : {} })
      }
    })().finally(() => {
      unauthorizedNavigation = null
    })
    return unauthorizedNavigation
  },
  forbidden: (error) => ui.showSnackbar(error.message || 'ليس لديك صلاحية لتنفيذ هذا الإجراء.', 'error'),
})

router.onError((error) => {
  console.error('Router navigation failed.', error)
  if (!router.currentRoute.value.name) startup.fail(error)
})

app.use(router)
app.mount('#app')
