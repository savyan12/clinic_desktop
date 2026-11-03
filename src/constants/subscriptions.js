export const SUBSCRIPTION_WEEKDAYS = Object.freeze([
  { title: 'الأحد', value: 0 },
  { title: 'الإثنين', value: 1 },
  { title: 'الثلاثاء', value: 2 },
  { title: 'الأربعاء', value: 3 },
  { title: 'الخميس', value: 4 },
])

export const DEFAULT_SUBSCRIPTION_WEEKDAYS = Object.freeze(SUBSCRIPTION_WEEKDAYS.map((day) => day.value))

export function normalizeSubscriptionWeekdays(values) {
  const allowed = new Set(DEFAULT_SUBSCRIPTION_WEEKDAYS)
  return [...new Set((Array.isArray(values) ? values : []).map(Number).filter((value) => allowed.has(value)))].sort((a, b) => a - b)
}

export function formatSubscriptionSchedule(values) {
  const normalized = normalizeSubscriptionWeekdays(values)
  if (!normalized.length) return 'أيام الحضور غير محددة'
  if (normalized.length === DEFAULT_SUBSCRIPTION_WEEKDAYS.length) return 'كل أيام الدوام (الأحد إلى الخميس)'
  return normalized.map((value) => SUBSCRIPTION_WEEKDAYS.find((day) => day.value === value)?.title).filter(Boolean).join('، ')
}
