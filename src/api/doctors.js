import { idempotencyHeaders, request } from './client'
import { cachedLookup, invalidateLookupAfter } from './lookupCache'

export const listDoctors = (params = {}) => request({ method: 'GET', url: '/doctors', params })
export const getDoctorOptions = () => cachedLookup('doctors', async () => (await listDoctors({ per_page: 100 })).data.map(({ id, name, is_active }) => ({ id, name, is_active })))
export const getDoctor = (id) => request({ method: 'GET', url: `/doctors/${id}` })
export const getDoctorDues = (id, params = {}) => request({ method: 'GET', url: `/doctors/${id}/dues`, params })
export const createDoctor = (data) => invalidateLookupAfter('doctors', request({ method: 'POST', url: '/doctors', data }))
export const updateDoctor = (id, data) => invalidateLookupAfter('doctors', request({ method: 'PATCH', url: `/doctors/${id}`, data }))
export const createPayout = (id, data, key) => request({ method: 'POST', url: `/doctors/${id}/payouts`, data, headers: idempotencyHeaders(key) })
