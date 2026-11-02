import { request } from './client'

export const listUsers = (params = {}) => request({ method: 'GET', url: '/users', params })
export const createUser = (data) => request({ method: 'POST', url: '/users', data })
export const updateUser = (id, data) => request({ method: 'PATCH', url: `/users/${id}`, data })
export const resetUserPassword = (id, data) => request({ method: 'POST', url: `/users/${id}/reset-password`, data })

