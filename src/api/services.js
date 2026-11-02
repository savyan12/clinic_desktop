import { request } from './client'
import { cachedLookup, invalidateLookupAfter } from './lookupCache'

export const listServices = (params = {}) => request({ method: 'GET', url: '/services', params })
export const getServiceOptions = () => cachedLookup('services', async () => (await listServices({ is_active: true, per_page: 100 })).data)
export const createService = (data) => invalidateLookupAfter('services', request({ method: 'POST', url: '/services', data }))
export const updateService = (id, data) => invalidateLookupAfter('services', request({ method: 'PATCH', url: `/services/${id}`, data }))
