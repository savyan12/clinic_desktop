import { labelFor } from '@/constants/labels'
import { formatMoney } from '@/composables/useMoney'

const dateFormatter = new Intl.DateTimeFormat('ar-LY-u-nu-latn', {
  dateStyle: 'medium',
  timeZone: 'Africa/Tripoli',
})

const dateTimeFormatter = new Intl.DateTimeFormat('ar-LY-u-nu-latn', {
  dateStyle: 'long',
  timeStyle: 'short',
  timeZone: 'Africa/Tripoli',
})

const nameOf = (value) => value?.name || '—'
const serviceNames = (row) => (row.services || []).map((service) => service.name).filter(Boolean).join('، ') || '—'
const supervisedSessions = (row) => (row.supervised_sessions || [])
  .map((session) => `${session.code || `#${session.id}`} — ${nameOf(session.doctor)}`)
  .join('؛ ') || '—'

export const reportTypes = [
  {
    value: 'sessions',
    title: 'تقرير الجلسات',
    description: 'الجلسات والخدمات والأطباء وقيم الخدمات خلال الفترة.',
    icon: 'mdi-clipboard-pulse-outline',
    columns: [
      { key: 'opened_at', label: 'التاريخ', kind: 'date', width: 15 },
      { key: 'patient', label: 'المريض', value: (row) => nameOf(row.patient), width: 17 },
      { key: 'doctor', label: 'الطبيب', value: (row) => nameOf(row.doctor), width: 15 },
      { key: 'services', label: 'الخدمات', value: serviceNames, width: 25 },
      { key: 'net_total', label: 'القيمة', kind: 'money', width: 15 },
      { key: 'status', label: 'الحالة', kind: 'status', statusType: 'session', width: 13 },
    ],
    totals: [
      { key: 'count', label: 'عدد الجلسات' },
      { key: 'service_value', label: 'قيمة الخدمات', money: true },
    ],
  },
  {
    value: 'collections',
    title: 'تقرير المقبوضات',
    description: 'الدفعات والاسترجاعات وصافي حركة المقبوضات.',
    icon: 'mdi-cash-multiple',
    columns: [
      { key: 'created_at', label: 'التاريخ', kind: 'date', width: 18 },
      { key: 'type', label: 'النوع', kind: 'enum', labelGroup: 'paymentType', width: 20 },
      { key: 'patient', label: 'المريض', value: (row) => nameOf(row.patient), width: 25 },
      { key: 'method', label: 'الطريقة', kind: 'enum', labelGroup: 'paymentMethod', width: 20 },
      { key: 'amount', label: 'المبلغ', kind: 'money', width: 17 },
    ],
    totals: [
      { key: 'received', label: 'إجمالي المقبوضات', money: true },
      { key: 'refunds', label: 'إجمالي الاسترجاعات', money: true },
      { key: 'net', label: 'صافي المقبوضات', money: true },
    ],
  },
  {
    value: 'debts',
    title: 'تقرير الديون',
    description: 'المبالغ الأصلية والمسدد والمتبقي حسب المريض والجهة.',
    icon: 'mdi-file-clock-outline',
    columns: [
      { key: 'id', label: 'المرجع', kind: 'reference', width: 10 },
      { key: 'patient', label: 'المريض', value: (row) => nameOf(row.patient), width: 18 },
      { key: 'sponsor', label: 'المسؤول', value: (row) => row.sponsor?.name || 'المريض', width: 16 },
      { key: 'amount', label: 'الأصل', kind: 'money', width: 14 },
      { key: 'paid_amount', label: 'المسدد', kind: 'money', width: 14 },
      { key: 'remaining', label: 'المتبقي', kind: 'money', width: 14 },
      { key: 'status', label: 'الحالة', kind: 'status', statusType: 'debt', width: 14 },
    ],
    totals: [
      { key: 'original', label: 'إجمالي الديون', money: true },
      { key: 'paid', label: 'إجمالي المسدد', money: true },
      { key: 'remaining', label: 'إجمالي المتبقي', money: true },
    ],
  },
  {
    value: 'doctor-dues',
    title: 'تقرير مستحقات الأطباء',
    description: 'المستحقات المكتسبة والمصروفة والمتبقية لكل طبيب.',
    icon: 'mdi-doctor',
    columns: [
      { key: 'name', label: 'الطبيب', width: 28 },
      { key: 'sessions_count', label: 'الجلسات', kind: 'number', width: 14 },
      { key: 'earned', label: 'المكتسب', kind: 'money', width: 19 },
      { key: 'paid', label: 'المصروف', kind: 'money', width: 19 },
      { key: 'remaining', label: 'المتبقي', kind: 'money', width: 20 },
    ],
    totals: [
      { key: 'earned', label: 'إجمالي المستحق المكتسب', money: true },
      { key: 'paid', label: 'إجمالي المصروف', money: true },
      { key: 'remaining', label: 'إجمالي المتبقي', money: true },
    ],
  },
  {
    value: 'subscriptions',
    title: 'تقرير الاشتراكات',
    description: 'الاشتراكات وقيمها والجلسات المستفاد منها والطبيب المشرف على كل جلسة.',
    icon: 'mdi-wallet-membership',
    columns: [
      { key: 'patient', label: 'المريض', value: (row) => nameOf(row.patient), width: 14 },
      { key: 'name', label: 'الاشتراك', width: 14 },
      { key: 'start_date', label: 'البداية', kind: 'date', width: 11 },
      { key: 'remaining_display', label: 'المتبقي', value: (row) => `${row.remaining ?? 0} جلسة`, width: 10 },
      { key: 'supervised_sessions', label: 'الجلسات والطبيب المشرف', value: supervisedSessions, width: 22 },
      { key: 'total_price', label: 'سعر الاشتراك', kind: 'money', width: 20 },
      { key: 'status', label: 'الحالة', kind: 'status', statusType: 'subscription', width: 12 },
    ],
    totals: [
      { key: 'count', label: 'عدد الاشتراكات' },
      { key: 'active', label: 'الاشتراكات النشطة' },
      { key: 'completed', label: 'الاشتراكات المكتملة' },
      { key: 'cancelled', label: 'الاشتراكات الملغاة' },
      { key: 'remaining_sessions', label: 'الجلسات المتبقية' },
      { key: 'received_amount', label: 'إجمالي المحصل', money: true },
      { key: 'refunded_amount', label: 'إجمالي المسترجع', money: true },
      { key: 'outstanding_amount', label: 'إجمالي المستحق', money: true },
    ],
  },
  {
    value: 'expenses',
    title: 'تقرير المشتريات والإيجار',
    description: 'المشتريات ودفعات الإيجار المدفوعة فعليًا مع حالة كل سجل.',
    icon: 'mdi-receipt-text-minus-outline',
    columns: [
      { key: 'payment_date', label: 'التاريخ', kind: 'date', width: 12 },
      { key: 'type', label: 'النوع', kind: 'enum', labelGroup: 'expenseType', width: 10 },
      { key: 'description', label: 'الوصف', width: 19 },
      { key: 'recipient', label: 'المستلم', width: 15 },
      { key: 'reference_number', label: 'المرجع', width: 12 },
      { key: 'payment_method', label: 'طريقة الدفع', kind: 'enum', labelGroup: 'paymentMethod', width: 12 },
      { key: 'amount', label: 'المبلغ', kind: 'money', width: 12 },
      { key: 'status', label: 'الحالة', kind: 'status', statusType: 'expense', width: 8 },
    ],
    totals: [
      { key: 'count', label: 'عدد العمليات' },
      { key: 'total_purchases', label: 'إجمالي المشتريات', money: true },
      { key: 'total_rent', label: 'إجمالي الإيجار', money: true },
      { key: 'total_expenses', label: 'إجمالي المصروفات', money: true },
    ],
  },
]

export function getReportDefinition(type) {
  return reportTypes.find((report) => report.value === type) || reportTypes[0]
}

export function reportCellRawValue(column, row) {
  return column.value ? column.value(row) : row?.[column.key]
}

export function reportCellLabel(column, row) {
  const value = reportCellRawValue(column, row)
  if (value === null || value === undefined || value === '') return '—'
  if (column.kind === 'date') return formatReportDate(value)
  if (column.kind === 'enum') return labelFor(column.labelGroup, value)
  if (column.kind === 'reference') return `#${value}`
  if (column.kind === 'money') return formatMoney(value)
  return String(value)
}

export function reportSummaryItems(type, totals = {}) {
  return getReportDefinition(type).totals
    .filter(({ key }) => Object.prototype.hasOwnProperty.call(totals, key))
    .map((item) => ({ ...item, value: totals[item.key] }))
}

export function formatReportDate(value) {
  if (!value) return '—'
  const source = /^\d{4}-\d{2}-\d{2}$/.test(String(value)) ? `${value}T00:00:00` : value
  const date = new Date(source)
  return Number.isNaN(date.getTime()) ? String(value) : dateFormatter.format(date)
}

export function formatReportDateTime(value) {
  const date = value ? new Date(value) : new Date()
  return dateTimeFormatter.format(Number.isNaN(date.getTime()) ? new Date() : date)
}

export function reportPeriodLabel(from, to) {
  if (from && to) return `من ${formatReportDate(from)} إلى ${formatReportDate(to)}`
  if (from) return `من ${formatReportDate(from)}`
  if (to) return `حتى ${formatReportDate(to)}`
  return 'كل الفترات'
}
