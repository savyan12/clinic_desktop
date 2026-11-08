import { defineStore } from 'pinia'
import { login as loginRequest, logout as logoutRequest, me as meRequest } from '@/api/auth'

const STORAGE_KEY = 'clinic-auth'
let initializationPromise = null

function readStoredAuth() {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY)) || null
  } catch {
    return null
  }
}

export const useAuthStore = defineStore('auth', {
  state: () => ({
    token: readStoredAuth()?.token || null,
    user: readStoredAuth()?.user || null,
    loading: false,
    initialized: false,
    initializing: false,
    initializationError: null,
  }),
  getters: {
    isAuthenticated: (state) => Boolean(state.token && state.user),
    role: (state) => state.user?.role || null,
    isAdmin: (state) => state.user?.role === 'admin',
  },
  actions: {
    persist() {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ token: this.token, user: this.user }))
    },
    async login(credentials) {
      this.loading = true
      try {
        const response = await loginRequest(credentials)
        this.token = response.data.token
        this.user = response.data.user
        this.initialized = true
        this.initializationError = null
        this.persist()
        return this.user
      } finally {
        this.loading = false
      }
    },
    async initialize(force = false) {
      if (this.initialized && !force) return this.isAuthenticated
      if (initializationPromise) return initializationPromise

      this.initializing = true
      this.initializationError = null
      initializationPromise = (async () => {
        if (!this.token) {
          this.user = null
          this.initialized = true
          return false
        }

        try {
          const response = await meRequest()
          this.user = response.data
          this.initialized = true
          this.persist()
          return true
        } catch (error) {
          if (error.status === 401) {
            this.clear()
            return false
          }
          this.initializationError = error.message || 'تعذر التحقق من جلسة الدخول.'
          throw error
        }
      })().finally(() => {
        this.initializing = false
        initializationPromise = null
      })

      return initializationPromise
    },
    restore() {
      return this.initialize()
    },
    async logout() {
      try {
        if (this.token) await logoutRequest()
      } finally {
        this.clear()
      }
    },
    clear() {
      this.token = null
      this.user = null
      this.initialized = true
      this.initializationError = null
      localStorage.removeItem(STORAGE_KEY)
    },
    switchMockRole(role) {
      if (import.meta.env.VITE_USE_MOCK !== 'true') return
      const username = role === 'admin' ? 'admin' : 'reception'
      return this.login({ username, password: 'demo' })
    },
  },
})
