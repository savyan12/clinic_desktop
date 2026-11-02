const numberFormatter = new Intl.NumberFormat('en-US-u-nu-latn', {
  useGrouping: false,
  minimumFractionDigits: 0,
  maximumFractionDigits: 3,
})

const arabicDigits = '٠١٢٣٤٥٦٧٨٩'

export function normalizeDecimalInput(value) {
  return String(value ?? '')
    .trim()
    .replace(/[٠-٩]/g, (digit) => String(arabicDigits.indexOf(digit)))
    .replace(/٬/g, '')
    .replace(/[٫,]/g, '.')
}

export function numeric(value) {
  const amount = Number.parseFloat(normalizeDecimalInput(value))
  return Number.isFinite(amount) ? amount : 0
}

export function formatNumber(value) {
  return numberFormatter.format(numeric(value))
}

export function formatMoney(value) {
  return `${formatNumber(value)} د.ل`
}

export function decimal(value) {
  return numeric(value).toFixed(3)
}

export function useMoney() {
  return { formatMoney, formatNumber, decimal, numeric, normalizeDecimalInput }
}
