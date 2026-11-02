import { idempotencyHeaders, request } from './client'

export const listSessions = (params = {}) => request({ method: 'GET', url: '/sessions', params })
export const getSession = (id) => request({ method: 'GET', url: `/sessions/${id}` })
export const createSession = (data, idempotencyKey) => request({ method: 'POST', url: '/sessions', data, headers: idempotencyHeaders(idempotencyKey) })
export const updateSession = (id, data) => request({ method: 'PATCH', url: `/sessions/${id}`, data })
export const closeSession = (id, data, idempotencyKey) => request({ method: 'POST', url: `/sessions/${id}/close`, data, headers: idempotencyHeaders(idempotencyKey) })
export const cancelSession = (id, data, idempotencyKey) => request({ method: 'POST', url: `/sessions/${id}/cancel`, data, headers: idempotencyHeaders(idempotencyKey) })
export const correctSession = (id, data, idempotencyKey) => request({ method: 'POST', url: `/sessions/${id}/correct`, data, headers: idempotencyHeaders(idempotencyKey) })
export const addSessionServices = (id, data, idempotencyKey) => request({ method: 'POST', url: `/sessions/${id}/services`, data, headers: idempotencyHeaders(idempotencyKey) })
