import { downloadRequest, request } from './client'

export const getReport = (type, params = {}) => request({ method: 'GET', url: `/reports/${type}`, params })
export const exportReport = (type, format, params = {}) => downloadRequest(
  { method: 'GET', url: `/reports/${type}/export`, params: { ...params, format } },
  `report-${type}.${format}`,
)
