import { formatMoney, formatNumber, normalizeDecimalInput } from '@/composables/useMoney'
import { CLINIC_TIME_ZONE } from '@/utils/date'

export const auditActionLabels = Object.freeze({
  session_created: 'إنشاء جلسة',
  session_updated: 'تعديل جلسة',
  session_services_added: 'إضافة خدمات إلى جلسة',
  session_closed: 'إغلاق جلسة',
  session_cancelled: 'إلغاء جلسة',
  session_corrected: 'تصحيح جلسة',
  patient_created: 'إضافة مريض',
  patient_updated: 'تعديل بيانات مريض',
  doctor_created: 'إضافة طبيب',
  doctor_updated: 'تعديل بيانات طبيب',
  doctor_rate_changed: 'تغيير نسبة الطبيب',
  doctor_payout_created: 'صرف مستحقات طبيب',
  doctor_paid: 'صرف مستحقات طبيب',
  service_created: 'إضافة خدمة',
  service_updated: 'تعديل خدمة',
  service_price_changed: 'تغيير سعر خدمة',
  subscription_created: 'إنشاء اشتراك',
  subscription_stopped: 'إيقاف اشتراك',
  subscription_reactivated: 'إعادة تفعيل اشتراك',
  subscription_cancelled: 'إلغاء اشتراك',
  subscription_corrected: 'تصحيح اشتراك',
  sponsor_created: 'إضافة جهة راعية',
  sponsor_updated: 'تعديل جهة راعية',
  user_created: 'إضافة مستخدم',
  user_updated: 'تعديل مستخدم',
  user_password_reset: 'إعادة تعيين كلمة مرور مستخدم',
  settings_updated: 'تعديل الإعدادات',
  payment_recorded: 'تسجيل دفعة',
  payment_refunded: 'استرجاع دفعة',
  wallet_topped_up: 'شحن رصيد',
  wallet_topup_refunded: 'استرجاع شحن رصيد',
  opening_debt_created: 'تسجيل دين افتتاحي',
  debt_payment_recorded: 'تسجيل دفعة دين',
  debt_settled: 'تسوية دين',
})

export const auditEntityLabels = Object.freeze({
  session: 'جلسة',
  session_addition: 'إضافة جلسة',
  session_service: 'خدمة جلسة',
  patient: 'مريض',
  doctor: 'طبيب',
  doctor_payout: 'دفعة طبيب',
  service: 'خدمة',
  subscription: 'اشتراك',
  subscription_service: 'خدمة اشتراك',
  subscription_usage: 'استخدام اشتراك',
  sponsor: 'جهة راعية',
  user: 'مستخدم',
  payment: 'دفعة',
  payment_allocation: 'تخصيص دفعة',
  wallet_transaction: 'حركة رصيد',
  debt: 'دين',
  setting: 'إعدادات',
  settings: 'إعدادات',
  audit_log: 'سجل تدقيق',
})

export const auditFieldLabels = Object.freeze({
  id: 'رقم السجل',
  action: 'العملية',
  entity_type: 'نوع السجل المتأثر',
  entity_id: 'رقم السجل المتأثر',
  old_values: 'القيم السابقة',
  new_values: 'القيم الجديدة',
  code: 'رمز الجلسة',
  name: 'الاسم',
  service_name: 'اسم الخدمة',
  username: 'اسم المستخدم',
  role: 'الدور',
  is_active: 'نشط',
  last_login_at: 'آخر تسجيل دخول',
  last_used_at: 'آخر استخدام',
  expires_at: 'تاريخ الانتهاء',
  phone: 'رقم الهاتف',
  address: 'العنوان',
  area: 'المنطقة',
  age: 'العمر',
  gender: 'الجنس',
  emergency_phone: 'هاتف الطوارئ',
  patient: 'بيانات المريض',
  patient_id: 'المريض',
  doctor: 'بيانات الطبيب',
  doctor_id: 'الطبيب',
  sponsor: 'بيانات الجهة الراعية',
  sponsor_id: 'الجهة الراعية',
  user_id: 'المستخدم',
  tokenable: 'صاحب رمز الدخول',
  tokenable_id: 'صاحب رمز الدخول',
  tokenable_type: 'نوع صاحب رمز الدخول',
  abilities: 'الصلاحيات',
  key: 'المفتاح',
  route: 'المسار',
  request_hash: 'بصمة الطلب',
  response: 'الاستجابة',
  service_id: 'الخدمة',
  service_ids: 'الخدمات',
  user: 'بيانات المستخدم',
  creator: 'بيانات منشئ السجل',
  subscription_id: 'الاشتراك',
  session_id: 'الجلسة',
  session_addition_id: 'إضافة الجلسة',
  addition_id: 'إضافة الجلسة',
  payment_id: 'الدفعة',
  refund_payment_id: 'دفعة الاسترجاع',
  reverses_payment_id: 'الدفعة الأصلية',
  debt_id: 'الدين',
  replacement_debt_id: 'الدين البديل',
  reverses_transaction_id: 'حركة الرصيد الأصلية',
  reverses_usage_id: 'استخدام الاشتراك الأصلي',
  created_by: 'أنشأه المستخدم',
  updated_by: 'آخر من عدّل',
  closed_by: 'أغلقها المستخدم',
  cancelled_by: 'ألغاها المستخدم',
  voided_by: 'ألغاه المستخدم',
  created_at: 'تاريخ الإنشاء',
  updated_at: 'تاريخ آخر تعديل',
  opened_at: 'وقت فتح الجلسة',
  closed_at: 'وقت إغلاق الجلسة',
  cancelled_at: 'وقت الإلغاء',
  voided_at: 'وقت إلغاء الدين',
  added_at: 'وقت الإضافة',
  date: 'التاريخ',
  type: 'النوع',
  status: 'الحالة',
  kind: 'نوع الدين',
  method: 'طريقة الدفع',
  payment_method: 'طريقة الدفع',
  coverage: 'طريقة التغطية',
  plan_type: 'نوع الاشتراك',
  scheduled_weekdays: 'أيام حضور الاشتراك',
  item_type: 'نوع البند',
  price: 'السعر',
  base_price: 'السعر الأساسي',
  extra_unit_price: 'سعر الوحدة الإضافية',
  amount: 'المبلغ',
  base_amount: 'المبلغ الأساسي',
  base_total: 'الإجمالي الأساسي',
  services_total: 'إجمالي الخدمات',
  discount: 'الخصم',
  net_total: 'الإجمالي الصافي',
  paid_amount: 'المبلغ المدفوع',
  paid_total: 'إجمالي المدفوع',
  wallet_amount: 'المبلغ من الرصيد',
  debt_amount: 'مبلغ الدين',
  refund_amount: 'مبلغ الاسترجاع',
  balance: 'الرصيد',
  balance_after: 'الرصيد بعد العملية',
  wallet_balance: 'رصيد المريض',
  debt_total: 'إجمالي الدين',
  debt_paid: 'الدين المسدد',
  remaining: 'المبلغ المتبقي',
  remaining_amount: 'المبلغ المتبقي',
  all_time_remaining: 'إجمالي المستحق المتبقي',
  doctor_dues_remaining: 'مستحقات الطبيب المتبقية',
  total_due: 'إجمالي المستحق',
  total_paid: 'إجمالي المدفوع',
  commission_rate: 'نسبة الطبيب',
  commission_base_amount: 'أساس احتساب العمولة',
  commission_amount: 'قيمة العمولة',
  rate: 'النسبة',
  note: 'الملاحظة',
  reason: 'السبب',
  cancel_reason: 'سبب الإلغاء',
  void_reason: 'سبب إلغاء الدين',
  clinic_name: 'اسم المركز',
  clinic_subtitle: 'وصف المركز',
  receipt_footer: 'تذييل الإيصال',
  currency: 'رمز العملة',
  print_size: 'حجم الطباعة',
  show_logo: 'إظهار الشعار',
  body_font_size: 'حجم خط النص',
  body_font_weight: 'سماكة خط النص',
  table_font_size: 'حجم خط الجداول',
  table_font_weight: 'سماكة خط الجداول',
  heading_font_size: 'حجم خط العناوين',
  heading_font_weight: 'سماكة خط العناوين',
  section_font_size: 'حجم خط الأقسام',
  section_font_weight: 'سماكة خط الأقسام',
  sessions_count: 'عدد الجلسات',
  sessions_used: 'الجلسات المستخدمة',
  sessions_remaining: 'الجلسات المتبقية',
  start_date: 'تاريخ البداية',
  expected_end_date: 'تاريخ الانتهاء المتوقع',
  end_date: 'تاريخ الانتهاء',
  included_units: 'الوحدات المشمولة',
  extra_units: 'الوحدات الإضافية',
  total_units: 'إجمالي الوحدات',
  unit_name: 'اسم الوحدة',
  is_addition: 'إضافة لاحقة',
  services: 'الخدمات',
  allocations: 'تخصيصات الدفعة',
  preview: 'ملخص الإلغاء',
  refund: 'الاسترجاع',
  replacement_debt: 'الدين البديل',
  debts_to_void: 'الديون المطلوب إلغاؤها',
  effects: 'الآثار المالية',
  replacement: 'العملية البديلة',
  subscription_usage: 'استخدام الاشتراك',
  wallet_transaction: 'حركة الرصيد',
  payment: 'الدفعة',
  debt: 'الدين',
  wallet: 'حركات الرصيد',
  refunds: 'الاسترجاعات',
  voided_debts: 'الديون الملغاة',
  voided_debt_ids: 'الديون الملغاة',
  commission_removed: 'العمولة الملغاة',
  commission_adjusted: 'تعديل العمولة',
  net_delta: 'فرق الصافي',
  commission_delta: 'فرق العمولة',
  period_days: 'أيام مدة الاشتراك',
  elapsed_days: 'الأيام المستخدمة',
  remaining_days: 'الأيام المتبقية',
  per_session_value: 'قيمة الجلسة',
  consumed_value: 'القيمة المستهلكة',
  debt_voided_amount: 'قيمة الديون الملغاة',
  replacement_debt_amount: 'قيمة الدين البديل',
  source_payment_ids: 'الدفعات الأصلية',
  tokens_revoked: 'إلغاء جلسات الدخول',
  historical_financial_rows_changed: 'تغيير السجلات المالية السابقة',
  replayed: 'عملية مكررة',
})

const auditValueLabels = Object.freeze({
  admin: 'مدير النظام',
  receptionist: 'موظف استقبال',
  male: 'ذكر',
  female: 'أنثى',
  open: 'مفتوحة',
  closed: 'مغلقة',
  cancelled: 'ملغاة',
  active: 'نشط',
  completed: 'مكتمل',
  stopped: 'موقوف',
  partial: 'مسدد جزئيًا',
  paid: 'مسدد',
  void: 'ملغى',
  individual: 'فردية',
  subscription: 'اشتراك',
  monthly: 'اشتراك شهري',
  legacy_package: 'باقة جلسات',
  session: 'جلسة',
  opening: 'افتتاحي',
  session_payment: 'دفعة جلسة',
  subscription_payment: 'دفعة اشتراك',
  wallet_topup: 'شحن رصيد',
  debt_payment: 'دفعة دين',
  refund: 'استرجاع',
  adjustment: 'تسوية',
  cash: 'نقدًا',
  transfer: 'تحويل مصرفي',
  wallet: 'رصيد',
  debt: 'دين',
  mixed: 'مختلط',
  topup: 'شحن',
  debit: 'خصم',
  reversal: 'عكس حركة',
  consume: 'استهلاك',
  restore: 'استعادة',
  service: 'خدمة',
  extra_units: 'وحدات إضافية',
})

const statusValueLabels = Object.freeze({
  session: { open: 'مفتوحة', closed: 'مغلقة', cancelled: 'ملغاة' },
  subscription: { active: 'نشط', completed: 'مكتمل', stopped: 'موقوف', cancelled: 'ملغي' },
  debt: { open: 'مفتوح', partial: 'مسدد جزئيًا', paid: 'مسدد', void: 'ملغى' },
})

const actionSuffixes = [
  ['_created', 'إنشاء'],
  ['_updated', 'تعديل'],
  ['_changed', 'تغيير'],
  ['_closed', 'إغلاق'],
  ['_cancelled', 'إلغاء'],
  ['_deleted', 'حذف'],
  ['_restored', 'استعادة'],
  ['_refunded', 'استرجاع'],
  ['_recorded', 'تسجيل'],
  ['_corrected', 'تصحيح'],
]

const auditEntityFilterKeys = [
  'session',
  'patient',
  'subscription',
  'service',
  'doctor',
  'doctor_payout',
  'sponsor',
  'user',
  'payment',
  'wallet_transaction',
  'debt',
  'settings',
]

const booleanFields = new Set([
  'is_active',
  'show_logo',
  'is_addition',
  'commission_adjusted',
  'tokens_revoked',
  'historical_financial_rows_changed',
  'replayed',
])

const moneyFields = new Set([
  'amount',
  'base_amount',
  'base_price',
  'base_total',
  'price',
  'extra_unit_price',
  'services_total',
  'discount',
  'net_total',
  'paid_amount',
  'paid_total',
  'wallet_amount',
  'debt_amount',
  'refund_amount',
  'balance',
  'balance_after',
  'wallet_balance',
  'debt_total',
  'debt_paid',
  'remaining',
  'remaining_amount',
  'all_time_remaining',
  'doctor_dues_remaining',
  'total_due',
  'total_paid',
  'commission_base_amount',
  'commission_amount',
  'per_session_value',
  'consumed_value',
  'debt_voided_amount',
  'replacement_debt_amount',
  'net_delta',
  'commission_delta',
])

const percentageFields = new Set(['commission_rate', 'rate'])
const enumFields = new Set(['role', 'gender', 'status', 'type', 'kind', 'method', 'payment_method', 'coverage', 'plan_type', 'item_type'])
const hiddenFields = new Set(['password', 'password_confirmation', 'remember_token', 'token'])
const relationFields = new Set(['patient', 'doctor', 'sponsor', 'creator', 'services'])

const dateFormatter = new Intl.DateTimeFormat('ar-LY-u-nu-latn', {
  dateStyle: 'medium',
  timeZone: CLINIC_TIME_ZONE,
})

const dateTimeFormatter = new Intl.DateTimeFormat('ar-LY-u-nu-latn', {
  dateStyle: 'medium',
  timeStyle: 'short',
  timeZone: CLINIC_TIME_ZONE,
})

export const auditActionItems = Object.entries(auditActionLabels).map(([value, title]) => ({ title, value }))
export const auditEntityItems = auditEntityFilterKeys.map((value) => ({ title: auditEntityLabels[value], value }))

function hasValue(value) {
  return value !== null && value !== undefined && String(value).trim() !== ''
}

function isNumeric(value) {
  if (typeof value === 'number') return Number.isFinite(value)
  if (typeof value !== 'string' || !value.trim()) return false
  return /^-?\d+(?:\.\d+)?$/.test(normalizeDecimalInput(value))
}

function fieldName(path) {
  return String(path || '').split('.').filter(Boolean).at(-1) || ''
}

function booleanValue(value) {
  if (value === true || value === 1 || value === '1' || value === 'true') return true
  if (value === false || value === 0 || value === '0' || value === 'false') return false
  return null
}

function parseDate(value) {
  const text = String(value).trim()
  const dateOnly = /^\d{4}-\d{2}-\d{2}$/.test(text)
  const normalized = dateOnly ? `${text}T00:00:00` : text.replace(' ', 'T')
  const parsed = new Date(normalized)
  return Number.isNaN(parsed.getTime()) ? null : { parsed, dateOnly }
}

function formatDateValue(value) {
  const result = parseDate(value)
  if (!result) return '—'
  return (result.dateOnly ? dateFormatter : dateTimeFormatter).format(result.parsed)
}

function isDateField(field) {
  return field === 'date' || field.endsWith('_date') || field.endsWith('_at')
}

function flattenAuditValues(value, path = [], result = new Map()) {
  const key = path.join('.')

  if (Array.isArray(value)) {
    if (!value.length) result.set(key, [])
    else if (value.some((item) => item && typeof item === 'object')) {
      value.forEach((item, index) => flattenAuditValues(item, [...path, String(index)], result))
    } else result.set(key, value)
    return result
  }

  if (value && typeof value === 'object') {
    const entries = Object.entries(value).filter(([field]) => !hiddenFields.has(field))
    if (!entries.length && key) result.set(key, null)
    entries.forEach(([field, item]) => flattenAuditValues(item, [...path, field], result))
    return result
  }

  result.set(key, value)
  return result
}

function rootField(path) {
  return String(path).split('.')[0]
}

function hasOwn(value, field) {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value) && Object.prototype.hasOwnProperty.call(value, field))
}

function isOneSidedRelation(path, oldValues, newValues) {
  const field = rootField(path)
  return relationFields.has(field) && hasOwn(oldValues, field) !== hasOwn(newValues, field)
}

function valuesMatch(oldValue, newValue, path, entityType) {
  return formatAuditValue(oldValue, path, entityType) === formatAuditValue(newValue, path, entityType)
}

export function auditActionLabel(value) {
  const key = String(value ?? '').trim()
  if (!key) return 'عملية غير محددة'
  if (auditActionLabels[key]) return auditActionLabels[key]

  const suffix = actionSuffixes.find(([ending]) => key.endsWith(ending))
  if (!suffix) return 'عملية أخرى'
  const entityKey = key.slice(0, -suffix[0].length)
  return `${suffix[1]} ${auditEntityLabels[entityKey] || 'سجل'}`
}

export function auditEntityLabel(value) {
  return auditEntityLabels[String(value ?? '').trim()] || 'سجل'
}

export function auditRecordLabel(entityType, entityId) {
  const label = auditEntityLabel(entityType)
  if (['setting', 'settings'].includes(String(entityType))) return label
  return hasValue(entityId) ? `${label} #${entityId}` : label
}

export function auditFieldLabel(path) {
  const parts = String(path || '').split('.').filter(Boolean)
  if (!parts.length) return 'البيانات'

  return parts.map((part) => {
    if (/^\d+$/.test(part)) return `البند ${Number(part) + 1}`
    return auditFieldLabels[part] || 'حقل إضافي'
  }).join(' — ')
}

export function formatAuditReason(value) {
  return hasValue(value) ? String(value).trim() : 'غير مذكور'
}

export function formatAuditTimestamp(value) {
  if (!hasValue(value)) return '—'
  const result = parseDate(value)
  return result ? dateTimeFormatter.format(result.parsed) : '—'
}

export function formatAuditValue(value, path = '', entityType = '') {
  const field = fieldName(path)
  if (!hasValue(value)) return '—'

  if (field === 'scheduled_weekdays') {
    const weekdayLabels = ['الأحد', 'الإثنين', 'الثلاثاء', 'الأربعاء', 'الخميس']
    const values = Array.isArray(value) ? value : [value]
    const labels = values.map((item) => weekdayLabels[Number(item)]).filter(Boolean)
    return labels.length ? labels.join('، ') : '—'
  }

  if (Array.isArray(value)) {
    if (!value.length) return '—'
    return value.map((item) => formatAuditValue(item, field, entityType)).join('، ')
  }

  if (booleanFields.has(field) || typeof value === 'boolean') {
    const normalized = booleanValue(value)
    return normalized === null ? '—' : normalized ? 'نعم' : 'لا'
  }

  if (isDateField(field)) return formatDateValue(value)
  if (moneyFields.has(field) && isNumeric(value)) return formatMoney(value)
  if (percentageFields.has(field) && isNumeric(value)) return `${formatNumber(value)}%`
  if (field === 'status' && statusValueLabels[entityType]?.[String(value)]) {
    return statusValueLabels[entityType][String(value)]
  }
  if (enumFields.has(field)) return auditValueLabels[String(value)] || 'قيمة أخرى'
  if (field.endsWith('_id') && isNumeric(value)) return `#${formatNumber(value)}`
  if (isNumeric(value) && typeof value === 'number') return formatNumber(value)
  if (value && typeof value === 'object') return 'بيانات إضافية'
  return String(value)
}

export function buildAuditChanges(entry = {}) {
  const oldValues = entry.old_values ?? null
  const newValues = entry.new_values ?? null
  const action = String(entry.action || '')
  const showOld = oldValues !== null && !action.endsWith('_created')
  const showNew = newValues !== null && !action.endsWith('_deleted') && !action.endsWith('_removed')
  const oldFields = flattenAuditValues(oldValues || {})
  const newFields = flattenAuditValues(newValues || {})
  const visiblePaths = showOld && showNew
    ? [...oldFields.keys(), ...newFields.keys()]
    : showNew
      ? [...newFields.keys()]
      : [...oldFields.keys()]
  const paths = [...new Set(visiblePaths)].filter(Boolean)

  const rows = paths.flatMap((path) => {
    if (showOld && showNew && isOneSidedRelation(path, oldValues, newValues)) return []
    const hasOld = oldFields.has(path)
    const hasNew = newFields.has(path)
    const oldValue = hasOld ? oldFields.get(path) : null
    const newValue = hasNew ? newFields.get(path) : null
    if (showOld && showNew && hasOld && hasNew && valuesMatch(oldValue, newValue, path, entry.entity_type)) return []

    return [{
      key: path,
      label: auditFieldLabel(path),
      oldValue: formatAuditValue(oldValue, path, entry.entity_type),
      newValue: formatAuditValue(newValue, path, entry.entity_type),
    }]
  })

  return { rows, showOld, showNew }
}
