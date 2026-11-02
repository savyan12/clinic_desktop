import { request } from './client'
import { cachedLookup, invalidateLookupAfter } from './lookupCache'

export const listSponsors = (params = {}) => request({ method: 'GET', url: '/sponsors', params })
export const getSponsorOptions = () => cachedLookup('sponsors', async () => (await listSponsors({ per_page: 100 })).data.map(({ id, name, is_active }) => ({ id, name, is_active })))
export const getSponsor = (id) => request({ method: 'GET', url: `/sponsors/${id}` })
export const getSponsorStatement = (id, params = {}) => request({ method: 'GET', url: `/sponsors/${id}/statement`, params })
export const createSponsor = (data) => invalidateLookupAfter('sponsors', request({ method: 'POST', url: '/sponsors', data }))
export const updateSponsor = (id, data) => invalidateLookupAfter('sponsors', request({ method: 'PATCH', url: `/sponsors/${id}`, data }))
