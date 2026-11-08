import { defineStore } from 'pinia'
import { useAuthStore } from '@/stores/auth'
import { useDesktopStore } from '@/stores/desktop'

let initializationPromise = null

function errorMessage(cause) {
  return cause?.message || 'تعذر تهيئة التطبيق. تحقق من اتصال الخدمات ثم أعد المحاولة.'
}

export const useStartupStore = defineStore('startup', {
  state: () => ({
    initialized: false,
    loading: false,
    error: null,
  }),
  actions: {
    async initialize({ force = false } = {}) {
      if (initializationPromise) return initializationPromise

      const desktop = useDesktopStore()
      const auth = useAuthStore()
      const authNeedsInitialization = desktop.activated && desktop.ready && (!auth.initialized || force)
      if (this.initialized && !force && !authNeedsInitialization) return true

      this.loading = true
      this.error = null
      initializationPromise = (async () => {
        try {
          await desktop.initialize(force)
          if (desktop.activated && desktop.ready) await auth.initialize(force)
          this.initialized = true
          return true
        } catch (cause) {
          this.initialized = false
          this.error = errorMessage(cause)
          console.error('Application initialization failed.', cause)
          return false
        } finally {
          this.loading = false
          initializationPromise = null
        }
      })()

      return initializationPromise
    },
    fail(cause) {
      this.initialized = false
      this.loading = false
      this.error = errorMessage(cause)
    },
  },
})
