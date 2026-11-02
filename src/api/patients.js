import { idempotencyHeaders, request } from './client'

export const listPatients = (params = {}, signal) => request({ method: 'GET', url: '/patients', params, signal })
export const getPatient = (id, signal) => request({ method: 'GET', url: `/patients/${id}`, signal })
export const getPatientStatement = (id, params = {}) => request({ method: 'GET', url: `/patients/${id}/statement`, params })
export const listWalletMovements = (id, params = {}) => request({ method: 'GET', url: `/patients/${id}/wallet/movements`, params })
export const listPatientDebts = (id, params = {}) => request({ method: 'GET', url: `/patients/${id}/debts`, params })
export const listPatientPayments = (id, params = {}) => request({ method: 'GET', url: `/patients/${id}/payments`, params })
export const createPatient = (data) => request({ method: 'POST', url: '/patients', data })
export const updatePatient = (id, data) => request({ method: 'PATCH', url: `/patients/${id}`, data })
export const topUpWallet = (id, data, idempotencyKey) => request({
  method: 'POST', url: `/patients/${id}/wallet/topups`, data, headers: idempotencyHeaders(idempotencyKey),
})
