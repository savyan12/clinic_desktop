import { request } from './client'

export const login = (credentials) => request({
  method: 'POST',
  url: '/auth/login',
  data: credentials,
  skipUnauthorizedHandler: true,
})
export const logout = () => request({ method: 'POST', url: '/auth/logout' })
export const me = () => request({ method: 'GET', url: '/auth/me', skipUnauthorizedHandler: true })
