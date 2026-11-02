import { defineStore } from 'pinia'
import { configureApiBaseUrl } from '@/api/client'
import { desktopErrorMessage, invokeDesktop, isDesktopRuntime } from '@/desktop/bridge'

export const useDesktopStore = defineStore('desktop', {
  state: () => ({
    initialized: false,
    loading: false,
    runtime: {
      desktop: isDesktopRuntime(),
      ready: !isDesktopRuntime(),
      activated: !isDesktopRuntime(),
      api_base_url: '/api/v1',
      error: null,
    },
    drives: [],
    backups: [],
  }),
  getters: {
    isDesktop: (state) => state.runtime.desktop,
    ready: (state) => state.runtime.ready,
    activated: (state) => state.runtime.activated,
  },
  actions: {
    applyRuntime(runtime) {
      this.runtime = { ...this.runtime, ...runtime }
      configureApiBaseUrl(this.runtime.desktop ? this.runtime.api_base_url : '/api/v1')
    },
    async initialize(force = false) {
      if (this.initialized && !force) return this.runtime
      if (!isDesktopRuntime()) {
        this.applyRuntime({ desktop: false, ready: true, activated: true, api_base_url: '/api/v1', error: null })
        this.initialized = true
        return this.runtime
      }
      this.loading = true
      try {
        this.applyRuntime(await invokeDesktop('get_runtime_info'))
        this.initialized = true
        return this.runtime
      } catch (cause) {
        this.applyRuntime({
          desktop: true,
          ready: false,
          error: desktopErrorMessage(cause, 'تعذر قراءة حالة الخدمات المحلية.'),
        })
        this.initialized = true
        return this.runtime
      } finally {
        this.loading = false
      }
    },
    async activateNew(request) {
      this.loading = true
      try {
        this.applyRuntime(await invokeDesktop('activate_new_installation', { request }))
        this.initialized = true
        return this.runtime
      } finally {
        this.loading = false
      }
    },
    async activateFromBackup(driveId, backupId) {
      this.loading = true
      try {
        this.applyRuntime(await invokeDesktop('activate_from_backup', { driveId, backupId }))
        this.initialized = true
        return this.runtime
      } finally {
        this.loading = false
      }
    },
    async restartRuntime() {
      this.loading = true
      try {
        this.applyRuntime(await invokeDesktop('restart_local_runtime'))
        return this.runtime
      } finally {
        this.loading = false
      }
    },
    async refreshDrives() {
      this.drives = this.isDesktop ? await invokeDesktop('list_removable_drives') : []
      return this.drives
    },
    async loadBackups(driveId) {
      this.backups = driveId ? await invokeDesktop('list_drive_backups', { driveId }) : []
      return this.backups
    },
    async createBackup(driveId) {
      return invokeDesktop('create_usb_backup', { driveId })
    },
  },
})
