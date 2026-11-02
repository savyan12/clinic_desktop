import { defineStore } from 'pinia'

export const useUiStore = defineStore('ui', {
  state: () => ({
    drawerOpen: false,
    snackbar: {
      visible: false,
      message: '',
      color: 'success',
      timeout: 4500,
    },
  }),
  actions: {
    showSnackbar(message, color = 'success', timeout = 4500) {
      this.snackbar = { visible: true, message, color, timeout }
    },
    hideSnackbar() {
      this.snackbar.visible = false
    },
    toggleDrawer() {
      this.drawerOpen = !this.drawerOpen
    },
  },
})

