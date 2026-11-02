import { reactive } from 'vue'

export function useConfirm() {
  const state = reactive({ open: false, title: '', message: '', requireReason: false, reason: '', resolve: null })
  const ask = ({ title, message, requireReason = false }) => new Promise((resolve) => {
    Object.assign(state, { open: true, title, message, requireReason, reason: '', resolve })
  })
  const answer = (value) => {
    state.resolve?.(value ? { confirmed: true, reason: state.reason } : { confirmed: false, reason: '' })
    state.open = false
  }
  return { state, ask, answer }
}

