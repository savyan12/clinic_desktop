import { request } from './client'

export const getSettings = () => request({ method: 'GET', url: '/settings' })
export const updateSettings = (data) => request({ method: 'PUT', url: '/settings', data })

