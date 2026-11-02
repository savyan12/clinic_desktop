const CLINIC_TIME_ZONE = 'Africa/Tripoli'

const datePartsFormatter = new Intl.DateTimeFormat('en', {
  timeZone: CLINIC_TIME_ZONE,
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
})

const hourFormatter = new Intl.DateTimeFormat('en', {
  timeZone: CLINIC_TIME_ZONE,
  hour: '2-digit',
  hourCycle: 'h23',
})

export function clinicDate(value = new Date()) {
  const parts = Object.fromEntries(datePartsFormatter.formatToParts(value).map((part) => [part.type, part.value]))
  return `${parts.year}-${parts.month}-${parts.day}`
}

export function clinicHour(value = new Date()) {
  return Number(hourFormatter.formatToParts(value).find((part) => part.type === 'hour')?.value || 0)
}

export { CLINIC_TIME_ZONE }
