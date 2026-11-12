import {
  clone,
  db,
  hydratePatient,
  hydrateSession,
  hydrateSubscription,
  isoDate,
  isoDateTime,
  nextId,
} from './data'

export class MockApiError extends Error {
  constructor(status, message, errors = {}) {
    super(message)
    this.name = 'MockApiError'
    this.status = status
    this.errors = errors
    this.response = { status, data: { message, errors } }
  }
}

const fail = (status, message, errors = {}) => {
  throw new MockApiError(status, message, errors)
}

const money = (value) => Number(value || 0).toFixed(3)
const sum = (values) => values.reduce((total, value) => total + Number(value || 0), 0)

function latency() {
  return new Promise((resolve) => setTimeout(resolve, 200 + Math.round(Math.random() * 200)))
}

function authUser(config) {
  const token = config.headers?.Authorization?.replace('Bearer ', '')
  if (token === 'mock-admin-token') return db.users.find((item) => item.username === 'admin')
  if (token === 'mock-reception-token') return db.users.find((item) => item.username === 'reception')
  try {
    const stored = JSON.parse(localStorage.getItem('clinic-auth'))
    return db.users.find((item) => item.id === stored?.user?.id) || null
  } catch {
    return null
  }
}

function requireAuth(config) {
  const user = authUser(config)
  if (!user || !user.is_active) fail(401, 'انتهت جلسة الدخول، يرجى تسجيل الدخول مجددًا.')
  return user
}

function requireAdmin(config) {
  const user = requireAuth(config)
  if (user.role !== 'admin') fail(403, 'ليس لديك صلاحية لتنفيذ هذا الإجراء.')
  return user
}

function paginate(items, params = {}) {
  const perPage = Math.max(1, Number(params.per_page || 15))
  const currentPage = Math.max(1, Number(params.page || 1))
  const start = (currentPage - 1) * perPage
  return {
    data: clone(items.slice(start, start + perPage)),
    meta: {
      current_page: currentPage,
      last_page: Math.max(1, Math.ceil(items.length / perPage)),
      per_page: perPage,
      total: items.length,
    },
  }
}

function addAudit(user, action, entityType, entityId, oldValues, newValues, reason = null) {
  db.auditLogs.unshift({
    id: nextId(db.auditLogs),
    user_id: user.id,
    action,
    entity_type: entityType,
    entity_id: entityId,
    old_values: oldValues ? clone(oldValues) : null,
    new_values: newValues ? clone(newValues) : null,
    reason,
    created_at: isoDateTime(0, new Date().getHours(), new Date().getMinutes()),
  })
}

function resourceById(collection, id, label = 'السجل') {
  const item = collection.find((entry) => entry.id === Number(id))
  if (!item) fail(404, `تعذر العثور على ${label} المطلوب.`)
  return item
}

function filterBySearch(items, search, fields) {
  if (!search) return items
  const query = String(search).trim().toLowerCase()
  return items.filter((item) => fields.some((field) => String(field(item) ?? '').toLowerCase().includes(query)))
}

function isNearCompletion(subscription) {
  return subscription.status === 'active'
    && Number(subscription.sessions_count) - Number(subscription.sessions_used) <= 2
}

function subscriptionDebts(subscriptionId) {
  return db.debts.filter((debt) => debt.subscription_id === Number(subscriptionId))
}

function subscriptionReceived(subscriptionId) {
  const debts = subscriptionDebts(subscriptionId)
  const direct = db.payments
    .filter((payment) => payment.subscription_id === Number(subscriptionId) && payment.type === 'subscription_payment' && Number(payment.amount) > 0)
  const allocated = db.paymentAllocations
    .filter((allocation) => debts.some((debt) => debt.id === allocation.debt_id))
  return sum(direct.map((payment) => payment.amount)) + sum(allocated.map((allocation) => allocation.amount))
}

function subscriptionRefunded(subscriptionId) {
  return sum(db.payments
    .filter((payment) => payment.subscription_id === Number(subscriptionId) && payment.type === 'refund' && Number(payment.amount) < 0)
    .map((payment) => Math.abs(Number(payment.amount))))
}

function refreshPatientDebtTotal(patientId) {
  const patient = resourceById(db.patients, patientId, 'المريض')
  patient.debt_total = money(sum(db.debts
    .filter((debt) => debt.patient_id === patient.id && debt.status !== 'void')
    .map((debt) => Math.max(0, Number(debt.amount) - Number(debt.paid_amount)))))
}

function doctorSummary(doctor) {
  const sessions = db.sessions
    .filter((session) => session.doctor_id === doctor.id && session.status === 'closed' && session.type === 'individual')
  const hydrated = sessions.map(hydrateSession)
  const deferred = sum(sessions.map((session, index) => Math.max(0, Number(session.commission_amount || 0) - Number(hydrated[index].commission_amount || 0))))
  const fullyDeferred = hydrated.filter((session, index) => Number(sessions[index].commission_amount || 0) > 0 && Number(session.commission_amount || 0) === 0).length
  const earned = Number(doctor.earned) - deferred
  const paid = Number(doctor.paid)
  return {
    ...doctor,
    sessions_count: Math.max(0, Number(doctor.sessions_count) - fullyDeferred),
    earned: money(earned),
    paid: money(paid),
    remaining: money(earned - paid),
  }
}

function dateTimeOn(date) {
  const now = new Date()
  const pad = (value) => String(value).padStart(2, '0')
  return `${date || isoDate()}T${pad(now.getHours())}:${pad(now.getMinutes())}:00+02:00`
}

function getDashboard(config) {
  const user = requireAuth(config)
  const today = isoDate()
  const todaySessions = db.sessions.filter((item) => item.status !== 'cancelled' && item.opened_at.startsWith(today))
  const openSessions = db.sessions.filter((item) => item.status === 'open').map(hydrateSession)
  const nearSubscriptions = db.subscriptions.filter(isNearCompletion).map(hydrateSubscription)
  const patientsToday = new Set(todaySessions.map((item) => item.patient_id)).size
  const common = {
    open_sessions: openSessions,
    today_sessions: todaySessions.map(hydrateSession),
    near_subscriptions: nearSubscriptions,
    shortcuts: [
      { label: 'المرضى', to: '/patients', icon: 'mdi-account-group-outline' },
      { label: 'الاشتراكات', to: '/subscriptions', icon: 'mdi-wallet-membership' },
    ],
  }
  if (user.role === 'receptionist') {
    return {
      data: {
        ...common,
        stats: [
          { key: 'sessions_today', label: 'جلسات اليوم', value: todaySessions.length, caption: `${openSessions.length} جلسات مفتوحة` },
          { key: 'patients_today', label: 'المرضى الحاضرون اليوم', value: patientsToday, caption: 'مرضى فريدون اليوم' },
          { key: 'near_subscriptions', label: 'اشتراكات تحتاج متابعة', value: nearSubscriptions.length, caption: 'متبقي جلستان أو أقل' },
        ],
      },
    }
  }
  const closedToday = todaySessions.filter((item) => item.status === 'closed')
  const collectedToday = db.payments.filter((item) => item.created_at.startsWith(today)).reduce((total, item) => total + Number(item.amount), 0)
  return {
    data: {
      ...common,
      shortcuts: [...common.shortcuts, { label: 'التقارير', to: '/reports/sessions', icon: 'mdi-chart-box-outline' }],
      stats: [
        { key: 'sessions_today', label: 'جلسات اليوم', value: todaySessions.length, caption: `${openSessions.length} مفتوحة`, to: `/reports/sessions?from=${today}&to=${today}` },
        { key: 'patients_today', label: 'المرضى الحاضرون', value: patientsToday, caption: 'مرضى فريدون', to: `/reports/sessions?from=${today}&to=${today}` },
        { key: 'service_value_today', label: 'محصّل جلسات اليوم', value: money(sum(closedToday.filter((item) => item.type === 'individual').map((item) => Number(item.settlement.wallet_amount) + Number(item.settlement.paid_amount)))), money: true, caption: 'المدفوع والرصيد المستخدم فقط', to: `/reports/sessions?from=${today}&to=${today}` },
        { key: 'collections_today', label: 'المقبوضات اليوم', value: money(collectedToday), money: true, caption: 'بعد الاسترجاعات', to: `/reports/collections?from=${today}&to=${today}` },
        { key: 'debts', label: 'إجمالي الديون', value: money(sum(db.debts.filter((item) => item.status !== 'void').map((item) => item.remaining))), money: true, caption: 'مبالغ لم تُحصّل', to: '/reports/debts?status=open' },
        { key: 'near_subscriptions', label: 'اشتراكات قريبة من الاكتمال', value: nearSubscriptions.length, caption: 'متبقي جلستان أو أقل', to: '/subscriptions' },
        { key: 'doctor_dues', label: 'مستحقات الأطباء', value: money(sum(db.doctors.map((item) => doctorSummary(item).remaining))), money: true, caption: 'غير مصروفة', to: '/reports/doctor-dues' },
      ],
      financial_summary: {
        collections_today: money(collectedToday),
        debts_remaining: money(sum(db.debts.map((item) => item.remaining))),
        doctor_dues_remaining: money(sum(db.doctors.map((item) => doctorSummary(item).remaining))),
      },
      recent_movements: db.payments.slice().reverse().slice(0, 5).map((payment) => ({
        ...payment,
        patient: payment.patient_id ? db.patients.find((item) => item.id === payment.patient_id) : null,
      })),
    },
  }
}

function listPatients(config) {
  requireAuth(config)
  let items = db.patients.map(hydratePatient)
  items = filterBySearch(items, config.params?.search, [(item) => item.name, (item) => item.phone])
  if (config.params?.subscription_status === 'active') items = items.filter((item) => item.subscription_status === 'active')
  if (config.params?.subscription_status === 'none') items = items.filter((item) => !item.subscription_status)
  return paginate(items, config.params)
}

function patientDetails(config, id) {
  requireAuth(config)
  const patient = hydratePatient(resourceById(db.patients, id, 'المريض'))
  return {
    data: {
      ...clone(patient),
      sessions: db.sessions.filter((item) => item.patient_id === patient.id && item.status !== 'cancelled').map(hydrateSession),
      subscriptions: db.subscriptions.filter((item) => item.patient_id === patient.id).map(hydrateSubscription),
      wallet_movements: db.walletTransactions.filter((item) => item.patient_id === patient.id),
      debts: db.debts.filter((item) => item.patient_id === patient.id),
      payments: db.payments.filter((item) => item.patient_id === patient.id),
    },
  }
}

function patientInput(payload, current = null) {
  const errors = {}
  const name = String(payload.name || '').trim()
  const phone = String(payload.phone || '').trim()
  const hasAge = payload.age !== null && payload.age !== undefined && payload.age !== ''
  const age = hasAge ? Number(payload.age) : null
  const gender = payload.gender || null
  const sponsorId = payload.sponsor_id ? Number(payload.sponsor_id) : null
  if (!name) errors.name = ['الاسم الكامل مطلوب.']
  if (!phone) errors.phone = ['رقم الهاتف مطلوب.']
  if (age !== null && (!Number.isInteger(age) || age < 0 || age > 130)) errors.age = ['العمر يجب أن يكون بين 0 و130.']
  if (gender && !['male', 'female'].includes(gender)) errors.gender = ['قيمة الجنس غير صحيحة.']
  if (sponsorId && !db.sponsors.some((item) => item.id === sponsorId && item.is_active)) errors.sponsor_id = ['الجهة المحددة غير موجودة أو غير نشطة.']
  if (Object.keys(errors).length) fail(422, 'يرجى مراجعة الحقول المطلوبة.', errors)
  const duplicate = db.patients.find((item) => item.phone === phone && item.id !== current?.id)
  if (duplicate && !payload.confirm_duplicate) {
    fail(422, 'يوجد مريض مسجل برقم الهاتف نفسه. أكد الحفظ إذا كان هذا سجلًا مختلفًا.', {
      phone: ['رقم الهاتف مستخدم في سجل آخر.'],
      confirm_duplicate: ['يلزم تأكيد حفظ سجل مكرر.'],
    })
  }
  return {
    name,
    phone,
    age,
    gender,
    area: String(payload.area || '').trim(),
    emergency_phone: String(payload.emergency_phone || '').trim() || null,
    sponsor_id: sponsorId,
    is_active: payload.is_active ?? current?.is_active ?? true,
  }
}

function createPatient(config) {
  const user = requireAuth(config)
  const payload = config.data || {}
  const input = patientInput(payload)
  const patient = {
    id: nextId(db.patients),
    ...input,
    wallet_balance: '0.000',
    debt_total: '0.000',
    subscription_status: null,
    created_at: isoDateTime(0, new Date().getHours(), new Date().getMinutes()),
  }
  db.patients.unshift(patient)
  addAudit(user, 'patient_created', 'patient', patient.id, null, patient)
  return { data: clone(hydratePatient(patient)) }
}

function updatePatient(config, id) {
  const user = requireAuth(config)
  const patient = resourceById(db.patients, id, 'المريض')
  const old = clone(patient)
  Object.assign(patient, patientInput(config.data || {}, patient), { id: patient.id })
  addAudit(user, 'patient_updated', 'patient', patient.id, old, patient)
  return { data: clone(hydratePatient(patient)) }
}

function listSessions(config) {
  requireAuth(config)
  let items = db.sessions.filter((item) => item.status !== 'cancelled').map(hydrateSession).sort((a, b) => b.opened_at.localeCompare(a.opened_at))
  items = filterBySearch(items, config.params?.search, [(item) => item.patient.name, (item) => item.patient.phone, (item) => item.code])
  if (config.params?.doctor_id) items = items.filter((item) => item.doctor_id === Number(config.params.doctor_id))
  if (config.params?.status) items = items.filter((item) => item.status === config.params.status)
  if (config.params?.date) items = items.filter((item) => item.opened_at.startsWith(config.params.date))
  return paginate(items, config.params)
}

function createSession(config) {
  const user = requireAuth(config)
  const payload = config.data || {}
  const patient = resourceById(db.patients, payload.patient_id, 'المريض')
  resourceById(db.doctors, payload.doctor_id, 'الطبيب')
  if (!patient.is_active) fail(422, 'لا يمكن فتح جلسة لمريض معطل.', { patient_id: ['المريض غير نشط.'] })
  let services = []
  let servicesTotal = 0
  let netTotal = 0
  let subscriptionId = null
  if (payload.type === 'subscription') {
    if (payload.settlement || Number(payload.discount || 0) !== 0 || payload.sponsor_id) fail(422, 'جلسة الاشتراك لا تقبل تسوية مالية مستقلة.')
    const subscription = resourceById(db.subscriptions, payload.subscription_id, 'الاشتراك')
    const remaining = Number(subscription.sessions_count) - Number(subscription.sessions_used)
    if (subscription.patient_id !== patient.id || subscription.status !== 'active' || remaining <= 0) {
      fail(422, 'الاشتراك غير صالح أو لا يحتوي على جلسات متبقية.', { subscription_id: ['اختر اشتراكًا نشطًا به جلسات متبقية.'] })
    }
    const requestedIds = (payload.services || payload.service_ids || []).map((item) => Number(item?.service_id ?? item))
    const coveredIds = new Set(subscription.services.map((service) => Number(service.service_id)))
    if (!requestedIds.length) fail(422, 'اختر خدمة واحدة على الأقل.', { services: ['الخدمات مطلوبة.'] })
    if (requestedIds.some((id) => !coveredIds.has(id))) fail(422, 'إحدى الخدمات غير مغطاة بهذا الاشتراك.', { services: ['اختر خدمات الاشتراك المحددة فقط.'] })
    services = requestedIds.map((id) => resourceById(db.services, id, 'الخدمة')).map((service) => ({ service_id: service.id, name: service.name, price: '0.000' }))
    servicesTotal = 0
    netTotal = 0
    subscriptionId = subscription.id
  } else {
    const serviceIds = (payload.services || payload.service_ids || []).map((item) => Number(item?.service_id ?? item))
    services = serviceIds.map((id) => resourceById(db.services, id, 'الخدمة')).map((service) => ({ service_id: service.id, name: service.name, price: service.price }))
    if (!services.length) fail(422, 'اختر خدمة واحدة على الأقل.', { service_ids: ['الخدمات مطلوبة.'] })
    if (services.some((service) => !resourceById(db.services, service.service_id).is_active)) fail(422, 'إحدى الخدمات المختارة غير نشطة.')
    servicesTotal = sum(services.map((service) => service.price))
    const discount = Number(payload.discount || 0)
    if (discount < 0 || discount > servicesTotal) fail(422, 'قيمة الخصم غير صحيحة.', { discount: ['الخصم يجب أن يكون بين صفر وإجمالي الخدمات.'] })
    netTotal = servicesTotal - discount
    const settlement = payload.settlement || {}
    const settlementTotal = sum([settlement.wallet_amount, settlement.paid_amount, settlement.debt_amount])
    if (Math.abs(settlementTotal - netTotal) > 0.001) {
      fail(422, 'مجموع أجزاء التسوية يجب أن يساوي صافي قيمة الجلسة.', { settlement: ['راجع مبالغ الرصيد والدفع والدين.'] })
    }
    if (Number(settlement.wallet_amount || 0) > Number(patient.wallet_balance)) {
      fail(422, 'الرصيد المتاح غير كافٍ. اشحن الرصيد أولًا ثم أعد المحاولة.', { 'settlement.wallet_amount': ['المبلغ يتجاوز الرصيد المتاح.'] })
    }
  }
  const session = {
    id: nextId(db.sessions),
    code: `S-${String(nextId(db.sessions) + 1000).padStart(4, '0')}`,
    patient_id: patient.id,
    doctor_id: Number(payload.doctor_id),
    services,
    service_ids: services.map((item) => item.service_id),
    type: payload.type || 'individual',
    subscription_id: subscriptionId,
    status: 'open',
    opened_at: dateTimeOn(payload.session_date),
    closed_at: null,
    services_total: money(servicesTotal),
    discount: payload.type === 'subscription' ? '0.000' : money(payload.discount),
    net_total: money(netTotal),
    settlement: payload.type === 'subscription' ? {
      coverage: 'subscription', wallet_amount: '0.000', paid_amount: '0.000', debt_amount: '0.000', method: null, sponsor_id: null,
    } : {
      coverage: payload.settlement?.coverage || 'cash',
      wallet_amount: money(payload.settlement?.wallet_amount),
      paid_amount: money(payload.settlement?.paid_amount),
      debt_amount: money(payload.settlement?.debt_amount),
      method: payload.settlement?.method || 'cash',
      sponsor_id: payload.settlement?.sponsor_id ? Number(payload.settlement.sponsor_id) : null,
    },
    created_by: user.id,
    closed_by: null,
    cancel_reason: null,
    commission_rate: null,
    commission_amount: null,
  }
  db.sessions.unshift(session)
  addAudit(user, 'session_created', 'session', session.id, null, session)
  return closeSession(config, session.id)
}

function updateSession(config, id) {
  const user = requireAuth(config)
  const session = resourceById(db.sessions, id, 'الجلسة')
  if (session.status !== 'open') {
    if (user.role !== 'admin') fail(403, 'لا يمكن لموظف الاستقبال تعديل جلسة مغلقة.')
    fail(409, 'استخدم إجراء تصحيح الجلسة المغلقة مع إدخال السبب.')
  }
  const old = clone(session)
  if (session.type === 'subscription' && (config.data.services || config.data.service_ids || config.data.subscription_id)) {
    fail(422, 'لا يمكن تغيير خدمات جلسة الاشتراك.', { services: ['ألغِ الجلسة وأنشئ جلسة صحيحة بدل إضافة خدمات أو تغيير الاشتراك المرتبط.'] })
  }
  if (config.data.doctor_id) session.doctor_id = Number(config.data.doctor_id)
  if (session.type === 'individual' && config.data.service_ids) {
    const services = config.data.service_ids.map((serviceId) => resourceById(db.services, serviceId, 'الخدمة'))
    if (!services.length) fail(422, 'اختر خدمة واحدة على الأقل.', { service_ids: ['الخدمات مطلوبة.'] })
    session.services = services.map((service) => ({ service_id: service.id, name: service.name, price: service.price }))
    session.service_ids = services.map((service) => service.id)
    session.services_total = money(sum(services.map((service) => service.price)))
    const discount = Number(config.data.discount || 0)
    if (discount < 0 || discount > Number(session.services_total)) fail(422, 'قيمة الخصم غير صحيحة.', { discount: ['راجع قيمة الخصم.'] })
    session.discount = money(discount)
    session.net_total = money(Number(session.services_total) - discount)
  }
  if (config.data.settlement) {
    const patient = resourceById(db.patients, session.patient_id)
    const updated = { ...session.settlement, ...config.data.settlement }
    const settlementTotal = sum([updated.wallet_amount, updated.paid_amount, updated.debt_amount])
    if (Math.abs(settlementTotal - Number(session.net_total)) > 0.001) fail(422, 'مجموع أجزاء التسوية يجب أن يساوي صافي الجلسة.')
    if (Number(updated.wallet_amount) > Number(patient.wallet_balance)) fail(422, 'الرصيد المتاح غير كافٍ. اشحن الرصيد أولًا ثم أعد المحاولة.')
    const parts = [Number(updated.wallet_amount) > 0, Number(updated.paid_amount) > 0, Number(updated.debt_amount) > 0].filter(Boolean).length
    updated.coverage = parts > 1 ? 'mixed' : Number(updated.wallet_amount) > 0 ? 'wallet' : Number(updated.debt_amount) > 0 ? 'debt' : updated.method === 'transfer' ? 'transfer' : 'cash'
    session.settlement = updated
  }
  addAudit(user, 'session_updated', 'session', session.id, old, session)
  return { data: clone(hydrateSession(session)) }
}

function addSessionServices(config, id) {
  const user = requireAuth(config)
  const session = resourceById(db.sessions, id, 'الجلسة')
  if (session.type === 'subscription') fail(422, 'لا يمكن إضافة خدمات إلى جلسة اشتراك.', { services: ['خدمات جلسة الاشتراك ثابتة ولا تقبل إضافات بعد تسجيل الجلسة.'] })
  if (session.status !== 'closed') fail(409, 'يمكن إضافة خدمات إلى جلسة مغلقة فقط.')

  const payload = config.data || {}
  const rawItems = payload.services || (payload.service_ids || []).map((serviceId) => ({ service_id: serviceId, item_type: 'service', extra_units: 0 }))
  if (!rawItems.length) fail(422, 'اختر خدمة واحدة على الأقل.', { services: ['حقل الخدمات مطلوب.'] })
  const seen = new Set()
  const services = rawItems.map((item) => {
    const service = resourceById(db.services, Number(item.service_id), 'الخدمة')
    if (!service.is_active) fail(422, 'إحدى الخدمات غير متاحة للجلسات الجديدة.', { services: ['اختر خدمات نشطة فقط.'] })
    const itemType = item.item_type || 'service'
    const key = `${service.id}:${itemType}`
    if (seen.has(key)) fail(422, 'لا يمكن تكرار بند الخدمة نفسه في العملية الواحدة.', { services: ['ادمج الكمية الإضافية في بند واحد.'] })
    seen.add(key)
    const extraUnits = Math.max(0, Number(item.extra_units || 0))
    if (itemType === 'extra_units' && (!service.extra_unit_price || extraUnits < 1)) fail(422, 'بيانات ملحقات الخدمة غير صحيحة.', { services: ['اختر خدمة تدعم وحدات إضافية وأدخل الكمية.'] })
    const total = (itemType === 'service' ? Number(service.price) : 0) + Number(service.extra_unit_price || 0) * extraUnits
    return { service_id: service.id, name: service.name, price: money(total), item_type: itemType, extra_units: extraUnits, is_addition: true }
  })
  const additionTotal = sum(services.map((service) => service.price))
  const settlement = payload.settlement || {}
  if (Math.abs(sum([settlement.wallet_amount, settlement.paid_amount, settlement.debt_amount]) - additionTotal) > 0.001) {
    fail(422, 'مجموع تسوية الخدمات الإضافية يجب أن يساوي قيمتها.', { settlement: ['راجع مبالغ الرصيد والمدفوع والدين.'] })
  }
  const patient = resourceById(db.patients, session.patient_id, 'المريض')
  const walletAmount = Number(settlement.wallet_amount || 0)
  const paidAmount = Number(settlement.paid_amount || 0)
  const debtAmount = Number(settlement.debt_amount || 0)
  if (walletAmount > Number(patient.wallet_balance)) fail(422, 'الرصيد المتاح غير كافٍ. اشحن الرصيد أولاً.')

  const addition = { id: nextId(session.additions || []), session_id: session.id, services_total: money(additionTotal), net_total: money(additionTotal), created_at: isoDateTime(), created_by: user.id }
  const effects = { wallet_transaction: null, payment: null, debt: null }
  if (walletAmount > 0) {
    patient.wallet_balance = money(Number(patient.wallet_balance) - walletAmount)
    effects.wallet_transaction = { id: nextId(db.walletTransactions), patient_id: patient.id, session_id: session.id, session_addition_id: addition.id, type: 'debit', amount: money(-walletAmount), balance_after: patient.wallet_balance, created_at: isoDateTime(), created_by: user.id }
    db.walletTransactions.unshift(effects.wallet_transaction)
  }
  if (paidAmount > 0) {
    effects.payment = { id: nextId(db.payments), patient_id: patient.id, session_id: session.id, session_addition_id: addition.id, type: 'adjustment', method: settlement.method || 'cash', amount: money(paidAmount), created_at: isoDateTime(), created_by: user.id }
    db.payments.unshift(effects.payment)
  }
  if (debtAmount > 0) {
    effects.debt = { id: nextId(db.debts), patient_id: patient.id, sponsor_id: settlement.sponsor_id ? Number(settlement.sponsor_id) : null, session_id: session.id, session_addition_id: addition.id, subscription_id: null, kind: 'session', amount: money(debtAmount), paid_amount: '0.000', remaining: money(debtAmount), status: 'open', created_at: isoDateTime(), created_by: user.id }
    db.debts.unshift(effects.debt)
    patient.debt_total = money(Number(patient.debt_total) + debtAmount)
    if (effects.debt.sponsor_id) {
      const sponsor = resourceById(db.sponsors, effects.debt.sponsor_id, 'الجهة الراعية')
      sponsor.total_due = money(Number(sponsor.total_due) + debtAmount)
      sponsor.remaining = money(Number(sponsor.remaining) + debtAmount)
    }
  }

  session.services.push(...services)
  session.service_ids.push(...services.map((service) => service.service_id))
  session.additions = session.additions || []
  session.additions.push(addition)
  session.services_total = money(Number(session.services_total) + additionTotal)
  session.net_total = money(Number(session.net_total) + additionTotal)
  session.settlement.wallet_amount = money(Number(session.settlement.wallet_amount) + walletAmount)
  session.settlement.paid_amount = money(Number(session.settlement.paid_amount) + paidAmount)
  session.settlement.debt_amount = money(Number(session.settlement.debt_amount) + debtAmount)
  if (paidAmount > 0) session.settlement.method = Number(session.settlement.paid_amount) === paidAmount || session.settlement.method === settlement.method ? (settlement.method || 'cash') : null
  const coverageParts = [Number(session.settlement.wallet_amount) > 0, Number(session.settlement.paid_amount) > 0, Number(session.settlement.debt_amount) > 0].filter(Boolean).length
  session.settlement.coverage = coverageParts > 1 ? 'mixed' : Number(session.settlement.wallet_amount) > 0 ? 'wallet' : Number(session.settlement.debt_amount) > 0 ? 'debt' : (settlement.method || session.settlement.method) === 'transfer' ? 'transfer' : 'cash'
  const doctor = resourceById(db.doctors, session.doctor_id, 'الطبيب')
  const commissionDelta = additionTotal * Number(session.commission_rate || doctor.commission_rate) / 100
  session.commission_base_amount = money(Number(session.commission_base_amount || 0) + additionTotal)
  session.commission_amount = money(Number(session.commission_amount || 0) + commissionDelta)
  doctor.earned = money(Number(doctor.earned) + commissionDelta)
  doctor.remaining = money(Number(doctor.remaining) + commissionDelta)
  addAudit(user, 'session_services_added', 'session', session.id, null, { addition, effects })
  return { data: clone({ session: hydrateSession(session), addition, effects }) }
}

function closeSession(config, id) {
  const user = requireAuth(config)
  const session = resourceById(db.sessions, id, 'الجلسة')
  if (session.status === 'closed') return { data: { session: clone(hydrateSession(session)), replayed: true } }
  if (session.status !== 'open') fail(409, 'لا يمكن إغلاق جلسة ملغاة.')
  const patient = resourceById(db.patients, session.patient_id, 'المريض')
  const effects = { subscription_usage: null, wallet_transaction: null, payment: null, debt: null }
  if (session.type === 'subscription') {
    const subscription = resourceById(db.subscriptions, session.subscription_id, 'الاشتراك')
    const remaining = Number(subscription.sessions_count) - Number(subscription.sessions_used)
    if (subscription.status !== 'active' || remaining <= 0) fail(422, 'الاشتراك لم يعد صالحًا أو لا توجد جلسات متبقية.')
    subscription.sessions_used += 1
    subscription.remaining = remaining - 1
    subscription.status = subscription.remaining === 0 ? 'completed' : 'active'
    subscription.usages = subscription.usages || []
    const usage = { id: nextId(subscription.usages), subscription_id: subscription.id, session_id: session.id, type: 'consume', reverses_usage_id: null, created_by: user.id, created_at: isoDateTime(0, new Date().getHours(), new Date().getMinutes()) }
    subscription.usages.unshift(usage)
    effects.subscription_usage = usage
  } else {
    const walletAmount = Number(session.settlement.wallet_amount || 0)
    if (walletAmount > Number(patient.wallet_balance)) fail(422, 'الرصيد المتاح غير كافٍ. اشحن الرصيد أولًا ثم أعد المحاولة.')
    if (walletAmount > 0) {
      patient.wallet_balance = money(Number(patient.wallet_balance) - walletAmount)
      const movement = { id: nextId(db.walletTransactions), patient_id: patient.id, session_id: session.id, type: 'debit', amount: money(-walletAmount), balance_after: patient.wallet_balance, created_at: isoDateTime(0, new Date().getHours(), new Date().getMinutes()), created_by: user.id }
      db.walletTransactions.unshift(movement)
      effects.wallet_transaction = movement
    }
    const paidAmount = Number(session.settlement.paid_amount || 0)
    if (paidAmount > 0) {
      const payment = { id: nextId(db.payments), patient_id: patient.id, session_id: session.id, type: 'session_payment', method: session.settlement.method, amount: money(paidAmount), created_at: isoDateTime(0, new Date().getHours(), new Date().getMinutes()), created_by: user.id }
      db.payments.unshift(payment)
      effects.payment = payment
    }
    const debtAmount = Number(session.settlement.debt_amount || 0)
    if (debtAmount > 0) {
      const debt = { id: nextId(db.debts), patient_id: patient.id, sponsor_id: session.settlement.sponsor_id, session_id: session.id, subscription_id: null, kind: 'session', amount: money(debtAmount), paid_amount: '0.000', remaining: money(debtAmount), status: 'open', created_at: isoDateTime(0, new Date().getHours(), new Date().getMinutes()) }
      db.debts.unshift(debt)
      patient.debt_total = money(Number(patient.debt_total) + debtAmount)
      if (debt.sponsor_id) {
        const sponsor = resourceById(db.sponsors, debt.sponsor_id)
        sponsor.total_due = money(Number(sponsor.total_due) + debtAmount)
        sponsor.remaining = money(Number(sponsor.remaining) + debtAmount)
      }
      effects.debt = debt
    }
  }
  if (session.type === 'individual') {
    const doctor = resourceById(db.doctors, session.doctor_id, 'الطبيب')
    const commission = Number(session.net_total) * Number(doctor.commission_rate) / 100
    session.commission_rate = doctor.commission_rate
    session.commission_base_amount = session.net_total
    session.commission_amount = money(commission)
    doctor.earned = money(Number(doctor.earned) + commission)
    doctor.remaining = money(Number(doctor.remaining) + commission)
    doctor.sessions_count += 1
  } else {
    session.commission_rate = null
    session.commission_base_amount = null
    session.commission_amount = null
  }
  session.status = 'closed'
  session.closed_at = isoDateTime(0, new Date().getHours(), new Date().getMinutes())
  session.closed_by = user.id
  addAudit(user, 'session_closed', 'session', session.id, { status: 'open' }, { status: 'closed', effects })
  return { data: { session: clone(hydrateSession(session)), effects: clone(effects), replayed: false } }
}

function cancelSession(config, id) {
  const user = requireAuth(config)
  const session = resourceById(db.sessions, id, 'الجلسة')
  const reason = config.data?.reason?.trim()
  if (!reason) fail(422, 'سبب الإلغاء مطلوب.', { reason: ['أدخل سبب الإلغاء.'] })
  if (session.status === 'cancelled') return { data: clone(hydrateSession(session)) }
  const oldStatus = session.status
  if (session.status === 'closed' && user.role !== 'admin') fail(403, 'إلغاء جلسة مغلقة متاح لمدير النظام فقط.')
  if (session.status === 'closed') {
    const patient = resourceById(db.patients, session.patient_id)
    if (session.type === 'subscription') {
      const subscription = resourceById(db.subscriptions, session.subscription_id)
      const consume = (subscription.usages || []).find((usage) => usage.session_id === session.id && usage.type === 'consume')
      subscription.sessions_used = Math.max(0, subscription.sessions_used - 1)
      subscription.remaining = Math.max(0, subscription.sessions_count - subscription.sessions_used)
      if (subscription.status === 'completed' && subscription.remaining > 0) subscription.status = 'active'
      subscription.usages = subscription.usages || []
      subscription.usages.unshift({ id: nextId(subscription.usages), subscription_id: subscription.id, session_id: session.id, type: 'restore', reverses_usage_id: consume?.id || null, created_by: user.id, created_at: isoDateTime() })
    }
    if (Number(session.settlement.wallet_amount) > 0) {
      const amount = Number(session.settlement.wallet_amount)
      patient.wallet_balance = money(Number(patient.wallet_balance) + amount)
      db.walletTransactions.unshift({ id: nextId(db.walletTransactions), patient_id: patient.id, session_id: session.id, type: 'reversal', amount: money(amount), balance_after: patient.wallet_balance, created_at: isoDateTime(), created_by: user.id })
    }
    if (Number(session.settlement.paid_amount) > 0) {
      db.payments.unshift({ id: nextId(db.payments), patient_id: patient.id, session_id: session.id, type: 'refund', method: session.settlement.method, amount: money(-Number(session.settlement.paid_amount)), created_at: isoDateTime(), created_by: user.id })
    }
    db.debts.filter((item) => item.session_id === session.id).forEach((debt) => { debt.status = 'void' })
    if (session.type === 'individual') {
      const doctor = resourceById(db.doctors, session.doctor_id)
      doctor.earned = money(Number(doctor.earned) - Number(session.commission_amount || 0))
      doctor.remaining = money(Number(doctor.remaining) - Number(session.commission_amount || 0))
      session.commission_amount = '0.000'
    }
  }
  session.status = 'cancelled'
  session.cancel_reason = reason
  session.cancelled_at = isoDateTime(0, new Date().getHours(), new Date().getMinutes())
  session.cancelled_by = user.id
  addAudit(user, 'session_cancelled', 'session', session.id, { status: oldStatus }, { status: 'cancelled' }, reason)
  return { data: clone(hydrateSession(session)) }
}

function correctSession(config, id) {
  const user = requireAdmin(config)
  const session = resourceById(db.sessions, id, 'الجلسة')
  if (session.status !== 'closed') fail(409, 'التصحيح الإداري مخصص للجلسات المغلقة.')
  const reason = config.data?.reason?.trim()
  if (!reason) fail(422, 'سبب التصحيح مطلوب.', { reason: ['أدخل سبب التصحيح.'] })
  const old = clone(session)
  const changes = config.data?.changes || config.data || {}
  if (session.type === 'subscription') {
    const changeKeys = Object.keys(changes).filter((key) => key !== 'reason')
    if (changeKeys.some((key) => key !== 'doctor_id')) fail(422, 'تصحيح جلسة الاشتراك يقتصر على الطبيب المشرف.')
    if (changes.doctor_id) resourceById(db.doctors, Number(changes.doctor_id), 'الطبيب')
    if (changes.doctor_id) session.doctor_id = Number(changes.doctor_id)
    const impact = { net_delta: '0.000', commission_delta: '0.000', supervising_doctor_updated: Boolean(changes.doctor_id) }
    addAudit(user, 'session_corrected', 'session', session.id, old, session, reason)
    return { data: { session: clone(hydrateSession(session)), financial_impact: impact } }
  }
  const oldNet = Number(session.net_total)
  if (changes.discount !== undefined) {
    const discount = Number(changes.discount)
    if (discount < 0 || discount > Number(session.services_total)) fail(422, 'قيمة الخصم غير صحيحة.')
    session.discount = money(discount)
    session.net_total = money(Number(session.services_total) - discount)
  }
  const doctor = resourceById(db.doctors, session.doctor_id)
  const oldCommission = Number(session.commission_amount)
  const newCommission = Number(session.net_total) * Number(session.commission_rate) / 100
  session.commission_amount = money(newCommission)
  doctor.earned = money(Number(doctor.earned) + newCommission - oldCommission)
  doctor.remaining = money(Number(doctor.remaining) + newCommission - oldCommission)
  const impact = { net_delta: money(Number(session.net_total) - oldNet), commission_delta: money(newCommission - oldCommission) }
  addAudit(user, 'session_corrected', 'session', session.id, old, session, reason)
  return { data: { session: clone(hydrateSession(session)), financial_impact: impact } }
}

function listSubscriptions(config) {
  requireAuth(config)
  let items = db.subscriptions.map(hydrateSubscription)
  items = filterBySearch(items, config.params?.search, [(item) => item.name, (item) => item.patient.name, (item) => item.patient.phone])
  if (config.params?.status) items = items.filter((item) => item.status === config.params.status)
  return paginate(items, config.params)
}

function subscriptionDetails(config, id) {
  requireAuth(config)
  const stored = resourceById(db.subscriptions, id, 'الاشتراك')
  const subscription = hydrateSubscription(stored)
  const sessions = db.sessions.filter((item) => item.subscription_id === subscription.id && item.status !== 'cancelled').map(hydrateSession).sort((a, b) => b.opened_at.localeCompare(a.opened_at))
  const debts = subscriptionDebts(subscription.id)
  const payments = db.payments.filter((item) => item.subscription_id === subscription.id)
  const allocationsByPayment = new Map()
  db.paymentAllocations.filter((allocation) => debts.some((debt) => debt.id === allocation.debt_id)).forEach((allocation) => {
    allocationsByPayment.set(allocation.payment_id, (allocationsByPayment.get(allocation.payment_id) || 0) + Number(allocation.amount))
  })
  for (const [paymentId, amount] of allocationsByPayment) {
    const payment = db.payments.find((item) => item.id === paymentId && item.type === 'debt_payment')
    if (payment) payments.push({ ...payment, amount: money(amount), source: 'debt_allocation', subscription_id: subscription.id })
  }
  payments.sort((a, b) => b.id - a.id)
  const usages = stored.usages || sessions.flatMap((session, index) => {
    if (session.status === 'closed') return [{ id: index + 1, subscription_id: subscription.id, session_id: session.id, type: 'consume', reverses_usage_id: null, created_at: session.closed_at }]
    return []
  })
  return { data: clone({ ...subscription, usages, sessions, payments, debts }) }
}

function listSubscriptionUsage(config, id) {
  requireAuth(config)
  const subscription = resourceById(db.subscriptions, id, 'الاشتراك')
  return paginate(subscription.usages || [], config.params)
}

function subscriptionCancellationState(config, id) {
  requireAdmin(config)
  const subscription = resourceById(db.subscriptions, id, 'الاشتراك')
  if (subscription.status === 'cancelled') fail(422, 'الاشتراك ملغي بالفعل.')
  if (db.sessions.some((item) => item.subscription_id === subscription.id && item.status === 'open')) fail(422, 'أغلق أو ألغِ الجلسة المفتوحة قبل إلغاء الاشتراك.')
  const debts = subscriptionDebts(subscription.id)
  const sources = new Map()
  db.payments.filter((item) => item.subscription_id === subscription.id && item.type === 'subscription_payment' && Number(item.amount) > 0).forEach((payment) => sources.set(payment.id, Number(payment.amount)))
  db.paymentAllocations.filter((allocation) => debts.some((debt) => debt.id === allocation.debt_id)).forEach((allocation) => sources.set(allocation.payment_id, (sources.get(allocation.payment_id) || 0) + Number(allocation.amount)))
  for (const [paymentId, contribution] of sources) {
    const refunded = sum(db.paymentRefundAllocations.filter((allocation) => allocation.original_payment_id === paymentId).map((allocation) => allocation.amount))
    sources.set(paymentId, Math.max(0, contribution - refunded))
  }
  const received = subscriptionReceived(subscription.id)
  const refunded = subscriptionRefunded(subscription.id)
  return {
    subscription,
    debtsToVoid: debts.filter((item) => ['open', 'partial'].includes(item.status)),
    sources,
    preview: {
      subscription_id: subscription.id,
      subscription_name: subscription.name,
      sessions_count: subscription.sessions_count,
      sessions_used: subscription.sessions_used,
      sessions_remaining: Math.max(0, subscription.sessions_count - subscription.sessions_used),
      total_price: money(subscription.total_price),
      received_amount: money(received),
      refunded_amount: money(refunded),
      maximum_refund: money(Math.max(0, received - refunded)),
      outstanding_amount: money(Math.max(0, Number(subscription.total_price) - received)),
    },
  }
}

function subscriptionCancellationPreview(config, id) {
  return { data: clone(subscriptionCancellationState(config, id).preview) }
}

function cancelSubscription(config, id) {
  const user = requireAdmin(config)
  const reason = config.data?.reason?.trim()
  if (!reason) fail(422, 'سبب إلغاء الاشتراك مطلوب.', { reason: ['اكتب سبب الإلغاء.'] })
  const { subscription, debtsToVoid, sources, preview } = subscriptionCancellationState(config, id)
  const refundAmount = Number(config.data?.refund_amount || 0)
  if (refundAmount < 0 || refundAmount > Number(preview.maximum_refund) + 0.001) fail(422, 'مبلغ الاسترجاع غير صالح.', { refund_amount: [`الحد الأقصى المتاح للاسترجاع هو ${preview.maximum_refund}.`] })
  if (refundAmount > 0 && !['cash', 'transfer'].includes(config.data?.refund_method)) fail(422, 'طريقة الاسترجاع مطلوبة.', { refund_method: ['اختر طريقة الاسترجاع.'] })
  const old = clone(subscription)
  let refund = null
  if (refundAmount > 0) {
    refund = { id: nextId(db.payments), patient_id: subscription.patient_id, subscription_id: subscription.id, session_id: null, type: 'refund', method: config.data.refund_method, amount: money(-refundAmount), reverses_payment_id: null, note: reason, created_at: isoDateTime(), created_by: user.id }
    db.payments.unshift(refund)
    let unallocated = refundAmount
    for (const [paymentId, available] of sources) {
      const amount = Math.min(unallocated, available)
      if (amount <= 0) continue
      db.paymentRefundAllocations.unshift({ id: nextId(db.paymentRefundAllocations), refund_payment_id: refund.id, original_payment_id: paymentId, amount: money(amount), created_by: user.id, created_at: isoDateTime() })
      unallocated -= amount
      if (unallocated <= 0.001) break
    }
  }
  debtsToVoid.forEach((debt) => { debt.status = 'void'; debt.remaining = '0.000'; debt.void_reason = reason; debt.voided_at = isoDateTime(); debt.voided_by = user.id })
  refreshPatientDebtTotal(subscription.patient_id)
  Object.assign(subscription, { status: 'cancelled', cancelled_at: isoDateTime(), cancelled_by: user.id, cancel_reason: reason })
  const patient = resourceById(db.patients, subscription.patient_id, 'المريض')
  patient.subscription_status = db.subscriptions.some((item) => item.patient_id === patient.id && item.status === 'active') ? 'active' : null
  addAudit(user, 'subscription_cancelled', 'subscription', subscription.id, old, { subscription: clone(subscription), refund_amount: money(refundAmount), refund_payment_id: refund?.id || null, voided_debt_ids: debtsToVoid.map((debt) => debt.id) }, reason)
  return { data: clone({ subscription: hydrateSubscription(subscription), refund, voided_debt_ids: debtsToVoid.map((debt) => debt.id) }) }
}

function createSubscription(config) {
  const user = requireAuth(config)
  const payload = config.data || {}
  const patient = resourceById(db.patients, payload.patient_id, 'المريض')
  if (!patient.is_active) fail(422, 'اختر مريضًا نشطًا.', { patient_id: ['المريض المحدد غير موجود أو غير نشط.'] })
  if (!payload.name?.trim()) fail(422, 'اسم الاشتراك مطلوب.', { name: ['اسم الاشتراك مطلوب.'] })
  const services = (payload.service_ids || []).map((id) => resourceById(db.services, id, 'الخدمة')).map((service) => ({ service_id: service.id, name: service.name, price: service.price }))
  if (!services.length) fail(422, 'اختر خدمة واحدة على الأقل.', { service_ids: ['الخدمات مطلوبة.'] })
  if (services.some((service) => !resourceById(db.services, service.service_id).is_active)) fail(422, 'إحدى الخدمات المختارة غير نشطة.', { service_ids: ['اختر خدمات نشطة فقط.'] })
  const count = Number(payload.sessions_count)
  if (!Number.isInteger(count) || count <= 0) fail(422, 'عدد الجلسات يجب أن يكون رقمًا صحيحًا أكبر من صفر.', { sessions_count: ['أدخل عدد جلسات صحيحًا.'] })
  const totalPrice = Number(payload.total_price)
  if (!Number.isFinite(totalPrice) || totalPrice <= 0) fail(422, 'قيمة الاشتراك غير صحيحة.', { total_price: ['أدخل سعر اشتراك أكبر من صفر.'] })
  const paid = Number(payload.paid_amount || 0)
  if (paid < 0 || paid > totalPrice) fail(422, 'المبلغ المدفوع غير صحيح.', { paid_amount: ['لا يجوز أن يتجاوز المبلغ قيمة الاشتراك.'] })
  if (paid > 0 && !['cash', 'transfer'].includes(payload.method)) fail(422, 'طريقة الدفع مطلوبة.', { method: ['اختر طريقة الدفع.'] })
  const remaining = totalPrice - paid
  const subscription = {
    id: nextId(db.subscriptions), patient_id: patient.id, name: payload.name.trim(), service_ids: services.map((item) => item.service_id), services,
    sessions_count: count, sessions_used: 0, remaining: count, total_price: money(totalPrice), start_date: isoDate(), status: 'active',
    cancelled_at: null, cancelled_by: null, cancel_reason: null, created_by: user.id, usages: [],
  }
  db.subscriptions.unshift(subscription)
  patient.subscription_status = 'active'
  let payment = null
  if (paid > 0) {
    payment = { id: nextId(db.payments), patient_id: patient.id, subscription_id: subscription.id, session_id: null, type: 'subscription_payment', method: payload.method, amount: money(paid), note: 'دفعة إنشاء الاشتراك', created_at: isoDateTime(), created_by: user.id }
    db.payments.unshift(payment)
  }
  let debt = null
  if (remaining > 0) {
    debt = { id: nextId(db.debts), patient_id: patient.id, sponsor_id: null, session_id: null, subscription_id: subscription.id, kind: 'subscription', amount: money(remaining), paid_amount: '0.000', remaining: money(remaining), status: 'open', note: 'المتبقي من قيمة الاشتراك', created_at: isoDateTime(), created_by: user.id }
    db.debts.unshift(debt)
    refreshPatientDebtTotal(patient.id)
  }
  addAudit(user, 'subscription_created', 'subscription', subscription.id, null, subscription)
  return { data: clone({ subscription: hydrateSubscription(subscription), payment, debt }) }
}

function correctSubscription(config, id) {
  const user = requireAdmin(config)
  const subscription = resourceById(db.subscriptions, id, 'الاشتراك')
  if (subscription.status === 'cancelled') fail(422, 'لا يمكن تعديل اشتراك ملغي.')
  const old = clone(subscription)
  const reason = config.data?.reason?.trim()
  if (!reason) fail(422, 'سبب التصحيح مطلوب.', { reason: ['أدخل السبب.'] })
  const changes = { ...(config.data?.changes || {}) }
  if (!Object.keys(changes).length) fail(422, 'أرسل تغييرًا واحدًا على الأقل.', { changes: ['أرسل تغييرًا واحدًا على الأقل.'] })
  if (Object.keys(changes).some((key) => !['sessions_count', 'total_price'].includes(key))) fail(422, 'البيانات المرسلة غير صحيحة.', { changes: ['تحتوي التغييرات على حقل غير مسموح.'] })
  const sessionsCount = Object.hasOwn(changes, 'sessions_count') ? Number(changes.sessions_count) : subscription.sessions_count
  if (!Number.isInteger(sessionsCount) || sessionsCount <= 0 || sessionsCount < subscription.sessions_used) fail(422, 'عدد الجلسات الجديد أقل من الجلسات المستخدمة.', { 'changes.sessions_count': [`الحد الأدنى هو ${subscription.sessions_used} جلسة.`] })
  const totalPrice = Object.hasOwn(changes, 'total_price') ? Number(changes.total_price) : Number(subscription.total_price)
  const retained = subscriptionReceived(subscription.id) - subscriptionRefunded(subscription.id)
  if (!Number.isFinite(totalPrice) || totalPrice <= 0 || totalPrice < retained) fail(422, 'قيمة الاشتراك الجديدة أقل من صافي المبلغ المحصل.', { 'changes.total_price': [`لا يمكن أن تقل القيمة عن ${money(retained)}.`] })
  subscription.sessions_count = sessionsCount
  subscription.total_price = money(totalPrice)
  subscription.remaining = Math.max(0, sessionsCount - subscription.sessions_used)
  subscription.status = subscription.remaining === 0 ? 'completed' : 'active'
  const outstanding = Math.max(0, totalPrice - subscriptionReceived(subscription.id))
  const debts = subscriptionDebts(subscription.id).filter((debt) => debt.status !== 'void')
  if (debts.length > 1) fail(500, 'Subscription has more than one active debt record.')
  if (debts[0]) {
    debts[0].amount = money(Number(debts[0].paid_amount) + outstanding)
    debts[0].remaining = money(outstanding)
    debts[0].status = outstanding === 0 ? 'paid' : Number(debts[0].paid_amount) > 0 ? 'partial' : 'open'
  } else if (outstanding > 0) {
    db.debts.unshift({ id: nextId(db.debts), patient_id: subscription.patient_id, sponsor_id: null, session_id: null, subscription_id: subscription.id, kind: 'subscription', amount: money(outstanding), paid_amount: '0.000', remaining: money(outstanding), status: 'open', note: 'المتبقي من قيمة الاشتراك', created_at: isoDateTime(), created_by: user.id })
  }
  refreshPatientDebtTotal(subscription.patient_id)
  addAudit(user, 'subscription_corrected', 'subscription', subscription.id, old, subscription, reason)
  return { data: clone({ subscription: hydrateSubscription(subscription) }) }
}

function topUpWallet(config, patientId) {
  const user = requireAuth(config)
  const patient = resourceById(db.patients, patientId, 'المريض')
  const amount = Number(config.data?.amount || 0)
  if (amount <= 0) fail(422, 'مبلغ الشحن يجب أن يكون أكبر من صفر.', { amount: ['أدخل مبلغًا صحيحًا.'] })
  const oldBalance = patient.wallet_balance
  patient.wallet_balance = money(Number(patient.wallet_balance) + amount)
  const movement = { id: nextId(db.walletTransactions), patient_id: patient.id, type: 'topup', amount: money(amount), balance_after: patient.wallet_balance, created_at: isoDateTime(), created_by: user.id }
  db.walletTransactions.unshift(movement)
  const payment = { id: nextId(db.payments), patient_id: patient.id, type: 'wallet_topup', method: config.data.method || 'cash', amount: money(amount), created_at: isoDateTime(), created_by: user.id }
  db.payments.unshift(payment)
  addAudit(user, 'wallet_topped_up', 'patient', patient.id, { wallet_balance: oldBalance }, { wallet_balance: patient.wallet_balance })
  return { data: { balance: patient.wallet_balance, movement: clone(movement), payment: clone(payment) } }
}

function settleDebts(config) {
  const user = requireAuth(config)
  const amount = Number(config.data?.amount || 0)
  const allocations = config.data?.allocations || []
  if (amount <= 0) fail(422, 'مبلغ التسديد يجب أن يكون أكبر من صفر.', { amount: ['أدخل مبلغًا صحيحًا.'] })
  const allocated = sum(allocations.map((item) => item.amount))
  if (allocated > amount + 0.001) fail(422, 'مجموع التخصيصات يتجاوز مبلغ الدفعة.', { allocations: ['خفّض مبالغ التخصيص.'] })
  for (const allocation of allocations) {
    const debt = resourceById(db.debts, allocation.debt_id, 'الدين')
    if (Number(allocation.amount) > Number(debt.remaining) + 0.001) fail(422, 'لا يمكن تخصيص مبلغ يتجاوز المتبقي على الدين.', { allocations: [`الدين رقم ${debt.id} لا يقبل هذا المبلغ.`] })
  }
  const firstDebt = resourceById(db.debts, allocations[0]?.debt_id, 'الدين')
  const payment = { id: nextId(db.payments), patient_id: firstDebt.patient_id, type: 'debt_payment', method: config.data.method || 'cash', amount: money(amount), created_at: isoDateTime(), created_by: user.id }
  db.payments.unshift(payment)
  allocations.forEach((allocation) => {
    const debt = resourceById(db.debts, allocation.debt_id)
    debt.paid_amount = money(Number(debt.paid_amount) + Number(allocation.amount))
    debt.remaining = money(Number(debt.amount) - Number(debt.paid_amount))
    debt.status = Number(debt.remaining) === 0 ? 'paid' : 'partial'
    db.paymentAllocations.unshift({ id: nextId(db.paymentAllocations), payment_id: payment.id, debt_id: debt.id, amount: money(allocation.amount) })
    const patient = resourceById(db.patients, debt.patient_id)
    patient.debt_total = money(Math.max(0, Number(patient.debt_total) - Number(allocation.amount)))
    if (debt.sponsor_id) {
      const sponsor = resourceById(db.sponsors, debt.sponsor_id)
      sponsor.total_paid = money(Number(sponsor.total_paid) + Number(allocation.amount))
      sponsor.remaining = money(Math.max(0, Number(sponsor.remaining) - Number(allocation.amount)))
    }
  })
  addAudit(user, 'debt_settled', 'payment', payment.id, null, { payment, allocations })
  return { data: { payment: clone(payment), allocations: clone(db.paymentAllocations.filter((item) => item.payment_id === payment.id)) } }
}

function createRefund(config) {
  const user = requireAdmin(config)
  const amount = Number(config.data?.amount || 0)
  if (amount <= 0) fail(422, 'مبلغ الاسترجاع يجب أن يكون أكبر من صفر.')
  const original = resourceById(db.payments, config.data.payment_id, 'الدفعة')
  const attributedToSubscription = Boolean(original.subscription_id)
    || db.paymentAllocations.some((allocation) => allocation.payment_id === original.id && db.debts.some((debt) => debt.id === allocation.debt_id && debt.subscription_id))
  if (attributedToSubscription) fail(422, 'استرجاع دفعات الاشتراك يتم من خلال إلغاء الاشتراك فقط.', { payment_id: ['استخدم إجراء إلغاء الاشتراك لتسجيل الاسترجاع وتخصيصه ماليًا.'] })
  const alreadyRefunded = db.payments.filter((item) => item.type === 'refund' && item.reverses_payment_id === original.id).reduce((total, item) => total + Math.abs(Number(item.amount)), 0)
  if (amount > Number(original.amount) - alreadyRefunded) fail(422, 'مبلغ الاسترجاع يتجاوز المبلغ المتاح.', { amount: ['راجع المبلغ المتاح للاسترجاع.'] })
  const patient = resourceById(db.patients, original.patient_id, 'المريض')
  if (original.type === 'wallet_topup' && amount > Number(patient.wallet_balance)) fail(422, 'رصيد المريض الحالي لا يكفي لاسترجاع هذا المبلغ.', { amount: ['رصيد المريض الحالي لا يكفي لاسترجاع هذا المبلغ.'] })
  const payment = { id: nextId(db.payments), patient_id: original.patient_id, session_id: original.session_id || null, subscription_id: original.subscription_id || null, type: 'refund', method: config.data.method || 'cash', amount: money(-amount), reverses_payment_id: original.id, note: config.data.reason, created_at: isoDateTime(), created_by: user.id }
  db.payments.unshift(payment)
  if (original.type === 'wallet_topup') {
    patient.wallet_balance = money(Number(patient.wallet_balance) - amount)
    db.walletTransactions.unshift({ id: nextId(db.walletTransactions), patient_id: patient.id, payment_id: payment.id, type: 'debit', amount: money(-amount), balance_after: patient.wallet_balance, note: config.data.reason, created_at: isoDateTime(), created_by: user.id })
  }
  addAudit(user, 'refund_created', 'payment', payment.id, null, payment, config.data.reason)
  return { data: clone(payment) }
}

function createOpeningDebt(config, patientId) {
  const user = requireAdmin(config)
  const patient = resourceById(db.patients, patientId, 'المريض')
  const amount = Number(config.data?.amount || 0)
  if (amount <= 0) fail(422, 'قيمة الدين الافتتاحي يجب أن تكون أكبر من صفر.', { amount: ['أدخل مبلغًا صحيحًا.'] })
  const debt = {
    id: nextId(db.debts), patient_id: patient.id, sponsor_id: config.data?.sponsor_id ? Number(config.data.sponsor_id) : null,
    session_id: null, subscription_id: null, kind: 'opening', amount: money(amount), paid_amount: '0.000', remaining: money(amount),
    status: 'open', created_at: isoDateTime(), note: config.data?.note || '',
  }
  db.debts.unshift(debt)
  patient.debt_total = money(Number(patient.debt_total) + amount)
  addAudit(user, 'opening_debt_created', 'debt', debt.id, null, debt, debt.note || null)
  return { data: clone(debt) }
}

function listDoctors(config) {
  requireAuth(config)
  let items = filterBySearch(db.doctors, config.params?.search, [(item) => item.name, (item) => item.phone])
    .map((doctor) => doctorSummary(doctor, config.params))
  return paginate(items, config.params)
}

function doctorDetails(config, id) {
  requireAuth(config)
  const doctor = resourceById(db.doctors, id, 'الطبيب')
  return { data: clone({ ...doctorSummary(doctor, config.params), sessions: db.sessions.filter((item) => item.doctor_id === doctor.id && item.status === 'closed' && item.commission_amount !== null).map(hydrateSession).filter((item) => Number(item.commission_amount) > 0), payouts: db.payouts.filter((item) => item.doctor_id === doctor.id) }) }
}

function saveDoctor(config, id = null) {
  const user = requireAdmin(config)
  const payload = config.data || {}
  if (!payload.name?.trim()) fail(422, 'اسم الطبيب مطلوب.', { name: ['الاسم مطلوب.'] })
  if (id) {
    const doctor = resourceById(db.doctors, id, 'الطبيب')
    const old = clone(doctor)
    Object.assign(doctor, payload, { commission_rate: money(payload.commission_rate), id: doctor.id })
    addAudit(user, 'doctor_updated', 'doctor', doctor.id, old, doctor, payload.reason || null)
    return { data: clone(doctor) }
  }
  const doctor = { id: nextId(db.doctors), name: payload.name, phone: payload.phone || '', commission_rate: money(payload.commission_rate), is_active: payload.is_active !== false, sessions_count: 0, earned: '0.000', paid: '0.000', remaining: '0.000' }
  db.doctors.unshift(doctor)
  addAudit(user, 'doctor_created', 'doctor', doctor.id, null, doctor)
  return { data: clone(doctor) }
}

function createPayout(config, doctorId) {
  const user = requireAdmin(config)
  const doctor = resourceById(db.doctors, doctorId, 'الطبيب')
  const current = doctorSummary(doctor)
  const amount = Number(config.data?.amount || 0)
  if (amount <= 0 || amount > Number(current.remaining)) fail(422, 'مبلغ الصرف يجب أن يكون أكبر من صفر وألا يتجاوز المستحق المتبقي.', { amount: ['راجع المبلغ المتاح.'] })
  const payout = { id: nextId(db.payouts), doctor_id: doctor.id, amount: money(amount), date: config.data.date || isoDate(), method: config.data.method || 'cash', note: config.data.note || '', created_by: user.id }
  db.payouts.unshift(payout)
  doctor.paid = money(Number(doctor.paid) + amount)
  doctor.remaining = money(Number(doctor.remaining) - amount)
  addAudit(user, 'doctor_paid', 'doctor', doctor.id, null, payout, payout.note)
  return { data: clone({ payout, doctor: doctorSummary(doctor) }) }
}

function listSimple(config, collection, searchFields) {
  requireAuth(config)
  const items = filterBySearch(collection, config.params?.search, searchFields)
  return paginate(items, config.params)
}

function saveService(config, id = null) {
  const user = requireAdmin(config)
  const payload = config.data || {}
  if (!payload.name?.trim()) fail(422, 'اسم الخدمة مطلوب.', { name: ['الاسم مطلوب.'] })
  if (Number(payload.price) < 0) fail(422, 'سعر الخدمة غير صحيح.', { price: ['أدخل سعرًا صحيحًا.'] })
  if (id) {
    const service = resourceById(db.services, id, 'الخدمة')
    const old = clone(service)
    Object.assign(service, payload, { price: money(payload.price), updated_at: isoDateTime(), id: service.id })
    addAudit(user, 'service_updated', 'service', service.id, old, service, payload.reason || null)
    return { data: clone(service) }
  }
  const service = { id: nextId(db.services), name: payload.name, price: money(payload.price), is_active: payload.is_active !== false, updated_at: isoDateTime() }
  db.services.unshift(service)
  addAudit(user, 'service_created', 'service', service.id, null, service)
  return { data: clone(service) }
}

function sponsorDetails(config, id) {
  requireAuth(config)
  const sponsor = resourceById(db.sponsors, id, 'الجهة الراعية')
  const patients = db.patients.filter((item) => item.sponsor_id === sponsor.id).map(hydratePatient)
  const patientIds = patients.map((item) => item.id)
  return { data: clone({ ...sponsor, patients, sessions: db.sessions.filter((item) => patientIds.includes(item.patient_id) && item.status !== 'cancelled').map(hydrateSession), debts: db.debts.filter((item) => item.sponsor_id === sponsor.id), payments: db.payments.filter((item) => patientIds.includes(item.patient_id)) }) }
}

function saveSponsor(config, id = null) {
  const user = requireAdmin(config)
  const payload = config.data || {}
  if (!payload.name?.trim()) fail(422, 'اسم الجهة مطلوب.', { name: ['الاسم مطلوب.'] })
  if (id) {
    const sponsor = resourceById(db.sponsors, id, 'الجهة الراعية')
    const old = clone(sponsor)
    Object.assign(sponsor, payload, { id: sponsor.id })
    addAudit(user, 'sponsor_updated', 'sponsor', sponsor.id, old, sponsor)
    return { data: clone(sponsor) }
  }
  const sponsor = { id: nextId(db.sponsors), name: payload.name, phone: payload.phone || '', address: payload.address || '', is_active: payload.is_active !== false, patients_count: 0, total_due: '0.000', total_paid: '0.000', remaining: '0.000' }
  db.sponsors.unshift(sponsor)
  addAudit(user, 'sponsor_created', 'sponsor', sponsor.id, null, sponsor)
  return { data: clone(sponsor) }
}

function listUsers(config) {
  requireAdmin(config)
  return paginate(filterBySearch(db.users.filter((item) => item.role === 'receptionist'), config.params?.search, [(item) => item.name, (item) => item.username]), config.params)
}

function saveUser(config, id = null) {
  const admin = requireAdmin(config)
  const payload = config.data || {}
  if (!payload.name?.trim() || !payload.username?.trim()) fail(422, 'اسم الموظف واسم المستخدم مطلوبان.', { name: !payload.name ? ['الاسم مطلوب.'] : [], username: !payload.username ? ['اسم المستخدم مطلوب.'] : [] })
  if (!id && (!payload.password || payload.password !== payload.password_confirmation)) fail(422, 'كلمتا المرور غير متطابقتين.', { password_confirmation: ['تأكيد كلمة المرور غير مطابق.'] })
  if (id) {
    const user = resourceById(db.users, id, 'المستخدم')
    const old = clone(user)
    Object.assign(user, { name: payload.name, username: payload.username, is_active: payload.is_active !== false })
    addAudit(admin, 'user_updated', 'user', user.id, old, user)
    return { data: clone(user) }
  }
  const user = { id: nextId(db.users), name: payload.name, username: payload.username, role: 'receptionist', is_active: payload.is_active !== false, created_at: isoDateTime(), last_login_at: null }
  db.users.unshift(user)
  addAudit(admin, 'user_created', 'user', user.id, null, user)
  return { data: clone(user) }
}

function resetPassword(config, id) {
  const admin = requireAdmin(config)
  const user = resourceById(db.users, id, 'المستخدم')
  if (!config.data?.password || config.data.password !== config.data.password_confirmation) fail(422, 'كلمتا المرور غير متطابقتين.', { password_confirmation: ['تأكيد كلمة المرور غير مطابق.'] })
  addAudit(admin, 'user_password_reset', 'user', user.id, null, { reset: true })
  return { data: { message: 'تمت إعادة تعيين كلمة المرور بنجاح.' } }
}

function isValidExpenseDate(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value || '')) return false
  const date = new Date(`${value}T00:00:00Z`)
  return !Number.isNaN(date.valueOf()) && date.toISOString().slice(0, 10) === value
}

function generatedExpenseReference(id) {
  const base = `EXP-${String(id).padStart(8, '0')}`
  let suffix = 1
  let candidate = base
  while (db.expenses.some((expense) => expense.reference_number === candidate && expense.id !== id)) {
    suffix += 1
    candidate = `${base}-${suffix}`
  }
  return candidate
}

function validateExpensePayload(payload, correction = false) {
  const errors = {}
  if (!['purchase', 'rent'].includes(payload.type)) errors.type = ['نوع المصروف مطلوب.']
  if (!payload.description?.trim()) errors.description = ['وصف المصروف مطلوب.']
  if (payload.type === 'purchase' && !payload.recipient?.trim()) errors.recipient = ['اسم المورد مطلوب.']
  if (!isValidExpenseDate(payload.payment_date)) errors.payment_date = ['تاريخ الدفع مطلوب.']
  else if (payload.payment_date > isoDate()) errors.payment_date = ['تاريخ الدفع لا يمكن أن يكون في المستقبل.']
  if (!(Number(payload.amount) > 0)) errors.amount = ['أدخل مبلغًا مدفوعًا صحيحًا.']
  if (!['cash', 'transfer'].includes(payload.payment_method)) errors.payment_method = ['طريقة الدفع مطلوبة.']
  if (payload.type === 'rent') {
    if (!/^\d{4}-(0[1-9]|1[0-2])$/.test(payload.rent_month || '')) errors.rent_month = ['شهر الإيجار مطلوب.']
    if (!isValidExpenseDate(payload.rent_start_date)) errors.rent_start_date = ['تاريخ بداية الإيجار مطلوب.']
    if (!isValidExpenseDate(payload.rent_end_date)) errors.rent_end_date = ['تاريخ نهاية الإيجار مطلوب.']
    if (!errors.rent_start_date && !errors.rent_end_date && payload.rent_end_date < payload.rent_start_date) {
      errors.rent_end_date = ['تاريخ نهاية الإيجار يجب ألا يسبق تاريخ البداية.']
    }
  }
  if (correction && !payload.reason?.trim()) errors.reason = ['سبب التصحيح مطلوب.']
  if (Object.keys(errors).length) fail(422, 'يرجى مراجعة بيانات المصروف.', errors)
  const isRent = payload.type === 'rent'
  return {
    type: payload.type,
    description: payload.description.trim(),
    recipient: isRent ? 'صاحب العقار' : payload.recipient.trim(),
    payment_date: payload.payment_date,
    amount: money(payload.amount),
    payment_method: payload.payment_method,
    rent_period: isRent ? `${payload.rent_start_date} إلى ${payload.rent_end_date}` : null,
    rent_month: isRent ? payload.rent_month : null,
    rent_start_date: isRent ? payload.rent_start_date : null,
    rent_end_date: isRent ? payload.rent_end_date : null,
    notes: payload.notes?.trim() || null,
  }
}

function listExpenses(config) {
  requireAdmin(config)
  const params = config.params || {}
  let items = filterBySearch(db.expenses, params.search, [
    (item) => item.description,
    (item) => item.recipient,
    (item) => item.reference_number,
  ])
  if (params.from || params.to) items = items.filter((item) => inDateRange(item.payment_date, params))
  if (params.type) items = items.filter((item) => item.type === params.type)
  if (params.method) items = items.filter((item) => item.payment_method === params.method)
  if (params.status) items = items.filter((item) => item.status === params.status)
  items = items.slice().sort((left, right) => right.payment_date.localeCompare(left.payment_date) || right.id - left.id)
  const active = items.filter((item) => item.status === 'active')
  const totalPurchases = sum(active.filter((item) => item.type === 'purchase').map((item) => item.amount))
  const totalRent = sum(active.filter((item) => item.type === 'rent').map((item) => item.amount))
  return {
    ...paginate(items, params),
    summary: {
      total_purchases: money(totalPurchases),
      total_rent: money(totalRent),
      total_expenses: money(totalPurchases + totalRent),
    },
  }
}

function createExpense(config) {
  const user = requireAdmin(config)
  const values = validateExpensePayload(config.data || {})
  const timestamp = dateTimeOn(isoDate())
  const id = nextId(db.expenses)
  const expense = {
    id,
    ...values,
    reference_number: generatedExpenseReference(id),
    status: 'active',
    cancelled_at: null,
    cancelled_by: null,
    cancel_reason: null,
    created_by: user.id,
    created_at: timestamp,
    updated_at: timestamp,
  }
  db.expenses.unshift(expense)
  addAudit(user, 'expense_created', 'expense', expense.id, null, expense)
  return { data: clone(expense) }
}

function correctExpense(config, id) {
  const user = requireAdmin(config)
  const expense = resourceById(db.expenses, id, 'المصروف')
  if (expense.status === 'cancelled') fail(422, 'لا يمكن تصحيح مصروف ملغي.')
  const values = validateExpensePayload(config.data || {}, true)
  const old = clone(expense)
  Object.assign(expense, values, { updated_at: dateTimeOn(isoDate()) })
  addAudit(user, 'expense_corrected', 'expense', expense.id, old, expense, config.data.reason.trim())
  return { data: clone(expense) }
}

function cancelExpenseRecord(config, id) {
  const user = requireAdmin(config)
  const expense = resourceById(db.expenses, id, 'المصروف')
  const reason = config.data?.reason?.trim()
  if (!reason) fail(422, 'سبب الإلغاء مطلوب.', { reason: ['سبب الإلغاء مطلوب.'] })
  if (expense.status === 'cancelled') fail(422, 'هذا المصروف ملغي بالفعل.')
  const old = clone(expense)
  Object.assign(expense, {
    status: 'cancelled',
    cancelled_at: dateTimeOn(isoDate()),
    cancelled_by: user.id,
    cancel_reason: reason,
    updated_at: dateTimeOn(isoDate()),
  })
  addAudit(user, 'expense_cancelled', 'expense', expense.id, old, expense, reason)
  return { data: clone(expense) }
}

function inDateRange(value, params) {
  if (!value) return false
  const date = String(value).slice(0, 10)
  return (!params.from || date >= params.from) && (!params.to || date <= params.to)
}

function paymentSponsorIds(payment) {
  const sponsorIds = new Set()
  const patient = db.patients.find((item) => item.id === payment.patient_id)
  if (patient?.sponsor_id) sponsorIds.add(patient.sponsor_id)

  const session = payment.session_id ? db.sessions.find((item) => item.id === payment.session_id) : null
  if (session?.settlement?.sponsor_id) sponsorIds.add(session.settlement.sponsor_id)

  db.paymentAllocations
    .filter((allocation) => allocation.payment_id === payment.id)
    .forEach((allocation) => {
      const debt = db.debts.find((item) => item.id === allocation.debt_id)
      if (debt?.sponsor_id) sponsorIds.add(debt.sponsor_id)
    })

  return sponsorIds
}

function reportData(config, type) {
  requireAdmin(config)
  const params = config.params || {}
  if (type === 'sessions') {
    let rows = db.sessions.filter((item) => item.status !== 'cancelled').map(hydrateSession)
    if (params.doctor_id) rows = rows.filter((item) => item.doctor_id === Number(params.doctor_id))
    if (params.patient_id) rows = rows.filter((item) => item.patient_id === Number(params.patient_id))
    if (params.status) rows = rows.filter((item) => item.status === params.status)
    if (params.from || params.to) rows = rows.filter((item) => inDateRange(item.opened_at, params))
    rows = rows.map((item) => ({ ...item, collected_amount: money(item.status === 'closed' && item.type === 'individual' ? Number(item.settlement.wallet_amount) + Number(item.settlement.paid_amount) : 0) }))
    return { data: { rows: clone(rows), totals: { count: rows.length, service_value: money(sum(rows.map((item) => item.collected_amount))) } } }
  }
  if (type === 'collections') {
    let rows = db.payments.map((item) => ({
      ...item,
      patient: item.patient_id ? db.patients.find((patient) => patient.id === item.patient_id) : null,
      sponsor_ids: [...paymentSponsorIds(item)],
    }))
    if (params.from || params.to) rows = rows.filter((item) => inDateRange(item.created_at, params))
    if (params.type) rows = rows.filter((item) => item.type === params.type)
    if (params.method) rows = rows.filter((item) => item.method === params.method)
    if (params.patient_id) rows = rows.filter((item) => item.patient_id === Number(params.patient_id))
    if (params.sponsor_id) rows = rows.filter((item) => item.sponsor_ids.includes(Number(params.sponsor_id)))
    return { data: { rows: clone(rows), totals: { received: money(sum(rows.filter((item) => Number(item.amount) > 0).map((item) => item.amount))), refunds: money(Math.abs(sum(rows.filter((item) => Number(item.amount) < 0).map((item) => item.amount)))), net: money(sum(rows.map((item) => item.amount))) } } }
  }
  if (type === 'debts') {
    let rows = db.debts.map((item) => ({ ...item, patient: db.patients.find((patient) => patient.id === item.patient_id), sponsor: item.sponsor_id ? db.sponsors.find((sponsor) => sponsor.id === item.sponsor_id) : null }))
    if (params.from || params.to) rows = rows.filter((item) => inDateRange(item.created_at, params))
    if (params.patient_id) rows = rows.filter((item) => item.patient_id === Number(params.patient_id))
    if (params.sponsor_id) rows = rows.filter((item) => item.sponsor_id === Number(params.sponsor_id))
    if (params.status) rows = rows.filter((item) => item.status === params.status)
    if (params.kind) rows = rows.filter((item) => item.kind === params.kind)
    return { data: { rows: clone(rows), totals: { original: money(sum(rows.map((item) => item.amount))), paid: money(sum(rows.map((item) => item.paid_amount))), remaining: money(sum(rows.map((item) => item.remaining))) } } }
  }
  if (type === 'doctor-dues') {
    let rows = db.doctors.map((item) => doctorSummary(item, params))
    if (params.doctor_id) rows = rows.filter((item) => item.id === Number(params.doctor_id))
    return { data: { rows: clone(rows), totals: { earned: money(sum(rows.map((item) => item.earned))), paid: money(sum(rows.map((item) => item.paid))), remaining: money(sum(rows.map((item) => item.remaining))) } } }
  }
  if (type === 'expenses') {
    let rows = db.expenses.slice()
    if (params.from || params.to) rows = rows.filter((item) => inDateRange(item.payment_date, params))
    if (params.type) rows = rows.filter((item) => item.type === params.type)
    if (params.method) rows = rows.filter((item) => item.payment_method === params.method)
    if (params.status) rows = rows.filter((item) => item.status === params.status)
    rows.sort((left, right) => right.payment_date.localeCompare(left.payment_date) || right.id - left.id)
    const active = rows.filter((item) => item.status === 'active')
    const totalPurchases = sum(active.filter((item) => item.type === 'purchase').map((item) => item.amount))
    const totalRent = sum(active.filter((item) => item.type === 'rent').map((item) => item.amount))
    return { data: { rows: clone(rows), totals: {
      count: active.length,
      total_purchases: money(totalPurchases),
      total_rent: money(totalRent),
      total_expenses: money(totalPurchases + totalRent),
    } } }
  }
  let rows = db.subscriptions.map((subscription) => ({
    ...hydrateSubscription(subscription),
    supervised_sessions: db.sessions
      .filter((session) => session.subscription_id === subscription.id && session.type === 'subscription' && session.status === 'closed')
      .filter((session) => (!params.from && !params.to) || inDateRange(session.opened_at, params))
      .filter((session) => !params.doctor_id || session.doctor_id === Number(params.doctor_id))
      .map((session) => ({
        id: session.id,
        code: session.code,
        opened_at: session.opened_at,
        closed_at: session.closed_at,
        status: session.status,
        doctor: db.doctors.find((doctor) => doctor.id === session.doctor_id),
      })),
  }))
  if (params.from || params.to) rows = rows.filter((item) => inDateRange(item.start_date, params))
  if (params.patient_id) rows = rows.filter((item) => item.patient_id === Number(params.patient_id))
  if (params.doctor_id) rows = rows.filter((item) => item.supervised_sessions.some((session) => session.doctor?.id === Number(params.doctor_id)))
  if (params.status) rows = rows.filter((item) => item.status === params.status)
  return { data: { rows: clone(rows), totals: {
    count: rows.length,
    active: rows.filter((item) => item.status === 'active').length,
    completed: rows.filter((item) => item.status === 'completed').length,
    cancelled: rows.filter((item) => item.status === 'cancelled').length,
    total_sessions: sum(rows.map((item) => item.sessions_count)),
    used_sessions: sum(rows.map((item) => item.sessions_used)),
    remaining_sessions: sum(rows.map((item) => item.remaining)),
    received_amount: money(sum(rows.map((item) => item.received_amount))),
    refunded_amount: money(sum(rows.map((item) => item.refunded_amount))),
    outstanding_amount: money(sum(rows.map((item) => item.outstanding_amount))),
    retained_amount: money(sum(rows.map((item) => item.retained_amount))),
  } } }
}

function listAudit(config) {
  requireAdmin(config)
  let items = db.auditLogs.map((item) => ({ ...item, user: db.users.find((user) => user.id === item.user_id) }))
  if (config.params?.user_id) items = items.filter((item) => item.user_id === Number(config.params.user_id))
  if (config.params?.action) items = items.filter((item) => item.action === config.params.action)
  if (config.params?.entity_type) items = items.filter((item) => item.entity_type === config.params.entity_type)
  if (config.params?.date) items = items.filter((item) => item.created_at.startsWith(config.params.date))
  return paginate(items, config.params)
}

function updateSettings(config) {
  const user = requireAdmin(config)
  const old = clone(db.settings)
  Object.assign(db.settings, config.data)
  addAudit(user, 'settings_updated', 'settings', 1, old, db.settings)
  return { data: clone(db.settings) }
}

export async function mockRequest(config) {
  await latency()
  const method = String(config.method || 'GET').toUpperCase()
  const path = String(config.url || '').replace(/^\/api\/v1/, '').split('?')[0]
  const segments = path.split('/').filter(Boolean)

  if (path === '/auth/login' && method === 'POST') {
    const username = config.data?.username
    const user = db.users.find((item) => item.username === username)
    if (!user || !['admin', 'reception'].includes(username)) fail(422, 'اسم المستخدم أو كلمة المرور غير صحيحة.', { username: ['تعذر تسجيل الدخول بهذه البيانات.'] })
    if (!user.is_active) fail(403, 'هذا الحساب معطل. راجع مدير النظام.')
    user.last_login_at = isoDateTime(0, new Date().getHours(), new Date().getMinutes())
    return { data: { token: user.role === 'admin' ? 'mock-admin-token' : 'mock-reception-token', user: clone(user) } }
  }
  if (path === '/auth/logout' && method === 'POST') return { data: { message: 'تم تسجيل الخروج بنجاح.' } }
  if (path === '/auth/me' && method === 'GET') return { data: clone(requireAuth(config)) }
  if (path === '/dashboard' && method === 'GET') return getDashboard(config)

  if (path === '/patients' && method === 'GET') return listPatients(config)
  if (path === '/patients' && method === 'POST') return createPatient(config)
  if (segments[0] === 'patients' && segments.length === 2 && method === 'GET') return patientDetails(config, segments[1])
  if (segments[0] === 'patients' && segments.length === 2 && method === 'PATCH') return updatePatient(config, segments[1])
  if (segments[0] === 'patients' && segments[2] === 'wallet' && segments[3] === 'topups' && method === 'POST') return topUpWallet(config, segments[1])
  if (segments[0] === 'patients' && segments[2] === 'opening-debts' && method === 'POST') return createOpeningDebt(config, segments[1])

  if (path === '/sessions' && method === 'GET') return listSessions(config)
  if (path === '/sessions' && method === 'POST') return createSession(config)
  if (segments[0] === 'sessions' && segments.length === 2 && method === 'GET') {
    const session = resourceById(db.sessions, segments[1], 'الجلسة')
    if (session.status === 'cancelled') fail(404, 'تعذر العثور على السجل المطلوب.')
    return { data: clone(hydrateSession(session)) }
  }
  if (segments[0] === 'sessions' && segments.length === 2 && method === 'PATCH') return updateSession(config, segments[1])
  if (segments[0] === 'sessions' && segments[2] === 'services' && method === 'POST') return addSessionServices(config, segments[1])
  if (segments[0] === 'sessions' && segments[2] === 'close' && method === 'POST') return closeSession(config, segments[1])
  if (segments[0] === 'sessions' && segments[2] === 'cancel' && method === 'POST') return cancelSession(config, segments[1])
  if (segments[0] === 'sessions' && segments[2] === 'correct' && method === 'POST') return correctSession(config, segments[1])

  if (path === '/subscriptions' && method === 'GET') return listSubscriptions(config)
  if (path === '/subscriptions' && method === 'POST') return createSubscription(config)
  if (segments[0] === 'subscriptions' && segments.length === 2 && method === 'GET') return subscriptionDetails(config, segments[1])
  if (segments[0] === 'subscriptions' && segments[2] === 'usage' && method === 'GET') return listSubscriptionUsage(config, segments[1])
  if (segments[0] === 'subscriptions' && segments[2] === 'cancel-preview' && method === 'GET') return subscriptionCancellationPreview(config, segments[1])
  if (segments[0] === 'subscriptions' && segments[2] === 'cancel' && method === 'POST') return cancelSubscription(config, segments[1])
  if (segments[0] === 'subscriptions' && segments[2] === 'correct' && method === 'POST') return correctSubscription(config, segments[1])

  if (path === '/debts/settlements' && method === 'POST') return settleDebts(config)
  if (path === '/refunds' && method === 'POST') return createRefund(config)

  if (path === '/expenses' && method === 'GET') return listExpenses(config)
  if (path === '/expenses' && method === 'POST') return createExpense(config)
  if (segments[0] === 'expenses' && segments[2] === 'correct' && method === 'POST') return correctExpense(config, segments[1])
  if (segments[0] === 'expenses' && segments[2] === 'cancel' && method === 'POST') return cancelExpenseRecord(config, segments[1])

  if (path === '/doctors' && method === 'GET') return listDoctors(config)
  if (path === '/doctors' && method === 'POST') return saveDoctor(config)
  if (segments[0] === 'doctors' && segments.length === 2 && method === 'GET') return doctorDetails(config, segments[1])
  if (segments[0] === 'doctors' && segments.length === 2 && method === 'PATCH') return saveDoctor(config, segments[1])
  if (segments[0] === 'doctors' && segments[2] === 'payouts' && method === 'POST') return createPayout(config, segments[1])

  if (path === '/services' && method === 'GET') return listSimple(config, db.services, [(item) => item.name])
  if (path === '/services' && method === 'POST') return saveService(config)
  if (segments[0] === 'services' && segments.length === 2 && method === 'PATCH') return saveService(config, segments[1])

  if (path === '/sponsors' && method === 'GET') return listSimple(config, db.sponsors, [(item) => item.name, (item) => item.phone])
  if (path === '/sponsors' && method === 'POST') return saveSponsor(config)
  if (segments[0] === 'sponsors' && segments.length === 2 && method === 'GET') return sponsorDetails(config, segments[1])
  if (segments[0] === 'sponsors' && segments.length === 2 && method === 'PATCH') return saveSponsor(config, segments[1])

  if (path === '/users' && method === 'GET') return listUsers(config)
  if (path === '/users' && method === 'POST') return saveUser(config)
  if (segments[0] === 'users' && segments.length === 2 && method === 'PATCH') return saveUser(config, segments[1])
  if (segments[0] === 'users' && segments[2] === 'reset-password' && method === 'POST') return resetPassword(config, segments[1])

  if (segments[0] === 'reports' && segments[1] && method === 'GET') return reportData(config, segments[1])
  if (path === '/audit-log' && method === 'GET') return listAudit(config)
  if (path === '/settings' && method === 'GET') { requireAuth(config); return { data: clone(db.settings) } }
  if (path === '/settings' && method === 'PUT') return updateSettings(config)

  fail(404, 'المسار المطلوب غير متاح في طبقة البيانات التجريبية.')
}
