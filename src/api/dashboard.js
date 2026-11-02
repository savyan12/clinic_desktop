import { request } from './client'

export const getDashboard = (params = {}) => request({ method: 'GET', url: '/dashboard', params })
