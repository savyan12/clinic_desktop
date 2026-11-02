import { defineStore } from 'pinia'
import { login as loginRequest, logout as logoutRequest, me as meRequest } from '@/api/auth'

const STORAGE_KEY = 'clinic-auth'

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
        this.persist()
        return this.user
      } finally {
        this.loading = false
      }
    },
    async restore() {
      if (!this.token) return false
      try {
        const response = await meRequest()
        this.user = response.data
        this.persist()
        return true
      } catch {
        this.clear()
        return false
      }
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
      localStorage.removeItem(STORAGE_KEY)
    },
    switchMockRole(role) {
      if (import.meta.env.VITE_USE_MOCK !== 'true') return
      const username = role === 'admin' ? 'admin' : 'reception'
      return this.login({ username, password: 'demo' })
    },
  },
})

