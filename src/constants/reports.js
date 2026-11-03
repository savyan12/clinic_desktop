import { labelFor } from '@/constants/labels'

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
    description: 'الاشتراكات الشهرية ومددها وقيمها وحالاتها.',
    icon: 'mdi-wallet-membership',
    columns: [
      { key: 'patient', label: 'المريض', value: (row) => nameOf(row.patient), width: 20 },
      { key: 'plan_type', label: 'النوع', kind: 'enum', labelGroup: 'subscriptionPlan', width: 13 },
      { key: 'start_date', label: 'البداية', kind: 'date', width: 14 },
      { key: 'end_date', label: 'النهاية', kind: 'date', value: (row) => row.end_date || row.expected_end_date, width: 14 },
      {
        key: 'remaining_display',
        label: 'المتبقي',
        value: (row) => row.plan_type === 'monthly'
          ? `${row.remaining_days ?? 0} يوم`
          : `${row.remaining ?? 0} جلسة`,
        width: 15,
      },
      { key: 'net_total', label: 'القيمة', kind: 'money', width: 12 },
      { key: 'status', label: 'الحالة', kind: 'status', statusType: 'subscription', width: 12 },
    ],
    totals: [
      { key: 'count', label: 'عدد الاشتراكات' },
      { key: 'active', label: 'الاشتراكات النشطة' },
      { key: 'remaining_sessions', label: 'الجلسات المتبقية' },
      { key: 'net_total', label: 'قيمة الاشتراكات', money: true },
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
