import { idempotencyHeaders, request } from './client'

export const listSubscriptions = (params = {}) => request({ method: 'GET', url: '/subscriptions', params })
export const getSubscription = (id) => request({ method: 'GET', url: `/subscriptions/${id}` })
export const listSubscriptionUsage = (id, params = {}) => request({ method: 'GET', url: `/subscriptions/${id}/usage`, params })
export const createSubscription = (data, idempotencyKey) => request({ method: 'POST', url: '/subscriptions', data, headers: idempotencyHeaders(idempotencyKey) })
export const stopSubscription = (id, data, key) => request({ method: 'POST', url: `/subscriptions/${id}/stop`, data, headers: idempotencyHeaders(key) })
export const reactivateSubscription = (id, data, key) => request({ method: 'POST', url: `/subscriptions/${id}/reactivate`, data, headers: idempotencyHeaders(key) })
export const getSubscriptionCancellationPreview = (id) => request({ method: 'GET', url: `/subscriptions/${id}/cancel-preview` })
export const cancelSubscription = (id, data, key) => request({ method: 'POST', url: `/subscriptions/${id}/cancel`, data, headers: idempotencyHeaders(key) })
export const correctSubscription = (id, data, key) => request({ method: 'POST', url: `/subscriptions/${id}/correct`, data, headers: idempotencyHeaders(key) })
