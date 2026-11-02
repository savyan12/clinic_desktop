import { request } from './client'

export const listAuditLog = (params = {}) => request({ method: 'GET', url: '/audit-log', params })

