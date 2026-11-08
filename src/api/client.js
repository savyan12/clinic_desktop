import axios from 'axios'

export class ApiError extends Error {
  constructor(message, status = 500, errors = {}) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.errors = errors
  }
}

const http = axios.create({
  baseURL: '/api/v1',
  headers: { Accept: 'application/json' },
})

export function configureApiBaseUrl(baseURL) {
  http.defaults.baseURL = baseURL || '/api/v1'
}

let errorHandlers = {}

function storedToken() {
  try {
    return JSON.parse(localStorage.getItem('clinic-auth'))?.token || null
  } catch {
    return null
  }
}

http.interceptors.request.use((config) => {
  const token = storedToken()
  if (token) config.headers.Authorization = `Bearer ${token}`
  return config
})

async function normalizeError(error) {
  if (error instanceof ApiError) return error
  const status = error.response?.status || error.status || 500
  let payload = error.response?.data

  if (payload instanceof Blob) {
    try {
      payload = JSON.parse(await payload.text())
    } catch {
      payload = null
    }
  }

  const message = payload?.message || error.message || 'تعذر إتمام الطلب. حاول مرة أخرى.'
  const errors = payload?.errors || error.errors || {}
  return new ApiError(message, status, errors)
}

async function handleError(error, config = {}) {
  if (error.status === 401 && !config.skipUnauthorizedHandler) await errorHandlers.unauthorized?.(error)
  if (error.status === 403) await errorHandlers.forbidden?.(error)
}

function responseFilename(headers, fallback) {
  const disposition = headers?.['content-disposition'] || ''
  const utf8Match = disposition.match(/filename\*=UTF-8''([^;]+)/i)
  const plainMatch = disposition.match(/filename="?([^";]+)"?/i)
  const encoded = utf8Match?.[1] || plainMatch?.[1]

  if (!encoded) return fallback

  try {
    return decodeURIComponent(encoded)
  } catch {
    return encoded
  }
}

export function configureApiErrorHandlers(handlers = {}) {
  errorHandlers = handlers
}

export function idempotencyHeaders(key) {
  return key ? { 'Idempotency-Key': key } : {}
}

export async function request(config) {
  try {
    if (import.meta.env.VITE_USE_MOCK === 'true') {
      const { mockRequest } = await import('@/mocks/handlers')
      return await mockRequest({
        ...config,
        headers: {
          ...config.headers,
          ...(storedToken() ? { Authorization: `Bearer ${storedToken()}` } : {}),
        },
      })
    }
    const response = await http.request(config)
    return response.data
  } catch (error) {
    const normalized = await normalizeError(error)
    await handleError(normalized, config)
    throw normalized
  }
}

export async function downloadRequest(config, fallbackFilename = 'download') {
  try {
    const response = await http.request({ ...config, responseType: 'blob' })
    return {
      blob: response.data,
      filename: responseFilename(response.headers, fallbackFilename),
      contentType: response.headers?.['content-type'] || response.data?.type,
    }
  } catch (error) {
    const normalized = await normalizeError(error)
    await handleError(normalized, config)
    throw normalized
  }
}

export default http
