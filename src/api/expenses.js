import { request } from './client'

export const listExpenses = (params = {}) => request({ method: 'GET', url: '/expenses', params })
export const createExpense = (data) => request({ method: 'POST', url: '/expenses', data })
export const correctExpense = (id, data) => request({ method: 'POST', url: `/expenses/${id}/correct`, data })
export const cancelExpense = (id, data) => request({ method: 'POST', url: `/expenses/${id}/cancel`, data })
