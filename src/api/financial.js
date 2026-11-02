import { idempotencyHeaders, request } from './client'

export const settleDebts = (data, key) => request({ method: 'POST', url: '/debts/settlements', data, headers: idempotencyHeaders(key) })
export const listDebts = (params = {}) => request({ method: 'GET', url: '/debts', params })
export const listPayments = (params = {}) => request({ method: 'GET', url: '/payments', params })
export const createRefund = (data, key) => request({ method: 'POST', url: '/refunds', data, headers: idempotencyHeaders(key) })
export const createOpeningDebt = (patientId, data, key) => request({ method: 'POST', url: `/patients/${patientId}/opening-debts`, data, headers: idempotencyHeaders(key) })
