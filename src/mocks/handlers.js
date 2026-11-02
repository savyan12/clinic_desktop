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

function isNearEnd(subscription) {
  if (subscription.status !== 'active') return false
  const nearSessions = subscription.remaining <= 2
  const end = subscription.expected_end_date ? new Date(`${subscription.expected_end_date}T00:00:00`) : null
  const days = end ? Math.ceil((end - new Date()) / 86400000) : Number.POSITIVE_INFINITY
  return nearSessions || (days >= 0 && days <= 7)
}

function sessionValue(subscription) {
  return money(Number(subscription.net_total) / Number(subscription.sessions_count || 1))
}

function getDashboard(config) {
  const user = requireAuth(config)
  const today = isoDate()
  const todaySessions = db.sessions.filter((item) => item.opened_at.startsWith(today))
  const openSessions = db.sessions.filter((item) => item.status === 'open').map(hydrateSession)
  const nearSubscriptions = db.subscriptions.filter(isNearEnd).map(hydrateSubscription)
  const patientsToday = new Set(todaySessions.map((item) => item.patient_id)).size
  const common = {
    open_sessions: openSessions,
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
        { key: 'service_value', label: 'قيمة جلسات اليوم', value: money(sum(closedToday.map((item) => item.net_total))), money: true, caption: 'قيمة خدمات مغلقة', to: `/reports/sessions?from=${today}&to=${today}` },
        { key: 'collections_today', label: 'المقبوضات اليوم', value: money(collectedToday), money: true, caption: 'بعد الاسترجاعات', to: `/reports/collections?from=${today}&to=${today}` },
        { key: 'debts', label: 'إجمالي الديون', value: money(sum(db.debts.filter((item) => item.status !== 'void').map((item) => item.remaining))), money: true, caption: 'مبالغ لم تُحصّل', to: '/reports/debts?status=open' },
        { key: 'near_subscriptions', label: 'اشتراكات قريبة', value: nearSubscriptions.length, caption: 'تحتاج متابعة', to: '/reports/subscriptions?near_end=1' },
        { key: 'doctor_dues', label: 'مستحقات الأطباء', value: money(sum(db.doctors.map((item) => item.remaining))), money: true, caption: 'غير مصروفة', to: '/reports/doctor-dues' },
      ],
      financial_summary: {
        collections_today: money(collectedToday),
        debts_remaining: money(sum(db.debts.map((item) => item.remaining))),
        doctor_dues_remaining: money(sum(db.doctors.map((item) => item.remaining))),
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
      sessions: db.sessions.filter((item) => item.patient_id === patient.id).map(hydrateSession),
      subscriptions: db.subscriptions.filter((item) => item.patient_id === patient.id).map(hydrateSubscription),
      wallet_movements: db.walletTransactions.filter((item) => item.patient_id === patient.id),
      debts: db.debts.filter((item) => item.patient_id === patient.id),
      payments: db.payments.filter((item) => item.patient_id === patient.id),
    },
  }
}

function createPatient(config) {
  const user = requireAuth(config)
  const payload = config.data || {}
  const errors = {}
  if (!payload.name?.trim()) errors.name = ['الاسم الكامل مطلوب.']
  if (!payload.phone?.trim()) errors.phone = ['رقم الهاتف مطلوب.']
  if (!payload.age || Number(payload.age) < 0) errors.age = ['يرجى إدخال عمر صحيح.']
  if (!payload.gender) errors.gender = ['الجنس مطلوب.']
  if (Object.keys(errors).length) fail(422, 'يرجى مراجعة الحقول المطلوبة.', errors)
  const duplicate = db.patients.find((item) => item.phone === payload.phone)
  if (duplicate && !payload.confirm_duplicate) {
    fail(422, 'يوجد مريض مسجل برقم الهاتف نفسه. أكد الحفظ إذا كان هذا سجلًا مختلفًا.', {
      phone: ['رقم الهاتف مستخدم في سجل آخر.'],
      confirm_duplicate: ['يلزم تأكيد إنشاء سجل مكرر.'],
    })
  }
  const patient = {
    id: nextId(db.patients),
    name: payload.name.trim(),
    phone: payload.phone.trim(),
    age: Number(payload.age),
    gender: payload.gender,
    area: payload.area || '',
    emergency_phone: payload.emergency_phone || null,
    sponsor_id: payload.sponsor_id ? Number(payload.sponsor_id) : null,
    wallet_balance: '0.000',
    debt_total: '0.000',
    subscription_status: null,
    is_active: true,
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
  Object.assign(patient, config.data, { id: patient.id })
  addAudit(user, 'patient_updated', 'patient', patient.id, old, patient)
  return { data: clone(hydratePatient(patient)) }
}

function listSessions(config) {
  requireAuth(config)
  let items = db.sessions.map(hydrateSession).sort((a, b) => b.opened_at.localeCompare(a.opened_at))
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
    const subscription = resourceById(db.subscriptions, payload.subscription_id, 'الاشتراك')
    if (subscription.patient_id !== patient.id || subscription.status !== 'active' || subscription.remaining <= 0) {
      fail(422, 'الاشتراك غير صالح أو لا يحتوي على جلسات متبقية.', { subscription_id: ['اختر اشتراكًا نشطًا به جلسات متبقية.'] })
    }
    services = clone(subscription.services)
    servicesTotal = Number(sessionValue(subscription))
    netTotal = servicesTotal
    subscriptionId = subscription.id
  } else {
    const serviceIds = (payload.service_ids || []).map(Number)
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
    opened_at: isoDateTime(0, new Date().getHours(), new Date().getMinutes()),
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
  return { data: clone(hydrateSession(session)) }
}

function updateSession(config, id) {
  const user = requireAuth(config)
  const session = resourceById(db.sessions, id, 'الجلسة')
  if (session.status !== 'open') {
    if (user.role !== 'admin') fail(403, 'لا يمكن لموظف الاستقبال تعديل جلسة مغلقة.')
    fail(409, 'استخدم إجراء تصحيح الجلسة المغلقة مع إدخال السبب.')
  }
  const old = clone(session)
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

function closeSession(config, id) {
  const user = requireAuth(config)
  const session = resourceById(db.sessions, id, 'الجلسة')
  if (session.status === 'closed') return { data: { session: clone(hydrateSession(session)), replayed: true } }
  if (session.status !== 'open') fail(409, 'لا يمكن إغلاق جلسة ملغاة.')
  const patient = resourceById(db.patients, session.patient_id, 'المريض')
  const effects = { subscription_usage: null, wallet_transaction: null, payment: null, debt: null }
  if (session.type === 'subscription') {
    const subscription = resourceById(db.subscriptions, session.subscription_id, 'الاشتراك')
    if (subscription.status !== 'active' || subscription.remaining <= 0) fail(422, 'الاشتراك لم يعد صالحًا أو لا توجد جلسات متبقية.')
    subscription.sessions_used += 1
    subscription.remaining -= 1
    subscription.status = subscription.remaining === 0 ? 'completed' : 'active'
    subscription.usages = subscription.usages || []
    const usage = { id: nextId(subscription.usages), session_id: session.id, type: 'consume', created_at: isoDateTime(0, new Date().getHours(), new Date().getMinutes()) }
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
  const doctor = resourceById(db.doctors, session.doctor_id, 'الطبيب')
  const commission = Number(session.net_total) * Number(doctor.commission_rate) / 100
  session.commission_rate = doctor.commission_rate
  session.commission_base_amount = session.net_total
  session.commission_amount = money(commission)
  doctor.earned = money(Number(doctor.earned) + commission)
  doctor.remaining = money(Number(doctor.remaining) + commission)
  doctor.sessions_count += 1
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
      subscription.sessions_used = Math.max(0, subscription.sessions_used - 1)
      subscription.remaining += 1
      if (subscription.status === 'completed') subscription.status = 'active'
      subscription.usages = subscription.usages || []
      subscription.usages.unshift({ id: nextId(subscription.usages), session_id: session.id, type: 'restore', created_at: isoDateTime() })
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
    const doctor = resourceById(db.doctors, session.doctor_id)
    doctor.earned = money(Number(doctor.earned) - Number(session.commission_amount || 0))
    doctor.remaining = money(Number(doctor.remaining) - Number(session.commission_amount || 0))
    session.commission_amount = '0.000'
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
  const oldNet = Number(session.net_total)
  if (config.data.discount !== undefined) {
    const discount = Number(config.data.discount)
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
  items = filterBySearch(items, config.params?.search, [(item) => item.patient.name, (item) => item.patient.phone])
  if (config.params?.status) items = items.filter((item) => item.status === config.params.status)
  if (String(config.params?.near_end) === '1') items = items.filter(isNearEnd)
  return paginate(items, config.params)
}

function subscriptionDetails(config, id) {
  requireAuth(config)
  const subscription = hydrateSubscription(resourceById(db.subscriptions, id, 'الاشتراك'))
  const sessions = db.sessions.filter((item) => item.subscription_id === subscription.id).map(hydrateSession)
  const payments = db.payments.filter((item) => item.subscription_id === subscription.id)
  const debts = db.debts.filter((item) => item.subscription_id === subscription.id)
  return { data: clone({ ...subscription, usages: subscription.usages || sessions.filter((item) => item.status === 'closed').map((item, index) => ({ id: index + 1, session_id: item.id, type: 'consume', created_at: item.closed_at })), sessions, payments, debts }) }
}

function subscriptionCancellationState(config, id) {
  requireAdmin(config)
  const subscription = resourceById(db.subscriptions, id, 'الاشتراك')
  if (subscription.status === 'cancelled') fail(409, 'تم إلغاء الاشتراك بالفعل.')
  if (subscription.status === 'completed') fail(409, 'لا يمكن إلغاء اشتراك مكتمل.')
  if (!['active', 'stopped'].includes(subscription.status)) fail(409, 'لا يمكن إلغاء الاشتراك في حالته الحالية.')
  const remaining = subscription.sessions_count - subscription.sessions_used
  if (remaining <= 0) fail(422, 'لا توجد جلسات متبقية لإلغاء الاشتراك.')
  if (db.sessions.some((item) => item.subscription_id === subscription.id && item.status === 'open')) fail(409, 'توجد جلسة مفتوحة مرتبطة بالاشتراك. أغلقها أو ألغها أولًا.')
  const debts = db.debts.filter((item) => item.subscription_id === subscription.id)
  const sources = new Map()
  db.payments.filter((item) => item.subscription_id === subscription.id && item.type === 'subscription_payment' && Number(item.amount) > 0).forEach((payment) => sources.set(payment.id, Number(payment.amount)))
  db.paymentAllocations.filter((allocation) => debts.some((debt) => debt.id === allocation.debt_id)).forEach((allocation) => sources.set(allocation.payment_id, (sources.get(allocation.payment_id) || 0) + Number(allocation.amount)))
  for (const [paymentId, contribution] of sources.entries()) {
    const refunded = db.payments.filter((item) => item.type === 'refund' && item.reverses_payment_id === paymentId).reduce((total, item) => total + Math.abs(Number(item.amount)), 0)
    sources.set(paymentId, Math.max(0, contribution - refunded))
  }
  const paidTotal = sum([...sources.values()])
  const consumedValue = Number((Number(subscription.net_total) * subscription.sessions_used / subscription.sessions_count).toFixed(3))
  const debtsToVoid = debts.filter((item) => ['open', 'partial'].includes(item.status))
  return {
    subscription,
    debtsToVoid,
    sourcePaymentIds: [...sources.entries()].filter(([, amount]) => amount > 0).map(([paymentId]) => paymentId),
    preview: {
      sessions_count: subscription.sessions_count,
      sessions_used: subscription.sessions_used,
      sessions_remaining: remaining,
      per_session_value: money(Number(subscription.net_total) / subscription.sessions_count),
      consumed_value: money(consumedValue),
      paid_total: money(paidTotal),
      refund_amount: money(Math.max(0, paidTotal - consumedValue)),
      debt_voided_amount: money(sum(debtsToVoid.map((debt) => Number(debt.amount) - Number(debt.paid_amount)))),
      replacement_debt_amount: money(Math.max(0, consumedValue - paidTotal)),
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
  const { subscription, debtsToVoid, sourcePaymentIds, preview } = subscriptionCancellationState(config, id)
  const old = clone(subscription)
  const patient = resourceById(db.patients, subscription.patient_id, 'المريض')
  let refund = null
  if (Number(preview.refund_amount) > 0) {
    refund = { id: nextId(db.payments), patient_id: subscription.patient_id, subscription_id: subscription.id, session_id: null, type: 'refund', method: 'cash', amount: money(-Number(preview.refund_amount)), reverses_payment_id: sourcePaymentIds.length === 1 ? sourcePaymentIds[0] : null, note: reason, created_at: isoDateTime(), created_by: user.id }
    db.payments.unshift(refund)
  }
  debtsToVoid.forEach((debt) => { patient.debt_total = money(Math.max(0, Number(patient.debt_total) - Number(debt.remaining))); debt.status = 'void'; debt.remaining = '0.000'; debt.void_reason = reason; debt.voided_at = isoDateTime(); debt.voided_by = user.id })
  let replacementDebt = null
  if (Number(preview.replacement_debt_amount) > 0) {
    replacementDebt = { id: nextId(db.debts), patient_id: subscription.patient_id, sponsor_id: subscription.sponsor_id || null, session_id: null, subscription_id: subscription.id, kind: 'subscription', amount: preview.replacement_debt_amount, paid_amount: '0.000', remaining: preview.replacement_debt_amount, status: 'open', note: `دين معدل بعد إلغاء الاشتراك #${subscription.id}`, created_at: isoDateTime(), created_by: user.id }
    db.debts.unshift(replacementDebt)
    patient.debt_total = money(Number(patient.debt_total) + Number(preview.replacement_debt_amount))
  }
  Object.assign(subscription, { status: 'cancelled', cancelled_at: isoDateTime(), cancelled_by: user.id, cancel_reason: reason, refund_amount: preview.refund_amount })
  patient.subscription_status = db.subscriptions.some((item) => item.patient_id === patient.id && item.status === 'active') ? 'active' : null
  addAudit(user, 'subscription_cancelled', 'subscription', subscription.id, old, { ...subscription, preview }, reason)
  return { data: clone({ subscription: hydrateSubscription(subscription), preview, refund, voided_debt_ids: debtsToVoid.map((debt) => debt.id), replacement_debt: replacementDebt }) }
}

function createSubscription(config) {
  const user = requireAuth(config)
  const payload = config.data || {}
  const patient = resourceById(db.patients, payload.patient_id, 'المريض')
  const services = (payload.service_ids || []).map((id) => resourceById(db.services, id, 'الخدمة')).map((service) => ({ service_id: service.id, name: service.name, price: service.price }))
  const count = Number(payload.sessions_count)
  if (!services.length) fail(422, 'اختر خدمة واحدة على الأقل.', { service_ids: ['الخدمات مطلوبة.'] })
  if (!Number.isInteger(count) || count <= 0) fail(422, 'عدد الجلسات يجب أن يكون رقمًا صحيحًا أكبر من صفر.', { sessions_count: ['أدخل عدد جلسات صحيحًا.'] })
  const base = sum(services.map((item) => item.price)) * count
  const discount = Number(payload.discount || 0)
  if (discount < 0 || discount > base) fail(422, 'قيمة الخصم غير صحيحة.', { discount: ['الخصم يتجاوز السعر الأساسي.'] })
  const net = base - discount
  const paid = Number(payload.paid_amount || 0)
  if (paid < 0 || paid > net) fail(422, 'المبلغ المدفوع غير صحيح.', { paid_amount: ['لا يجوز أن يتجاوز المبلغ القيمة النهائية.'] })
  const remaining = net - paid
  const subscription = {
    id: nextId(db.subscriptions), patient_id: patient.id, service_ids: services.map((item) => item.service_id), services,
    sessions_count: count, sessions_used: 0, remaining: count, base_total: money(base), discount: money(discount), net_total: money(net),
    paid_amount: money(paid), remaining_amount: money(remaining), start_date: payload.start_date || isoDate(), expected_end_date: payload.expected_end_date || null,
    status: 'active', sponsor_id: payload.sponsor_id ? Number(payload.sponsor_id) : null, usages: [],
  }
  db.subscriptions.unshift(subscription)
  patient.subscription_status = 'active'
  if (paid > 0) db.payments.unshift({ id: nextId(db.payments), patient_id: patient.id, subscription_id: subscription.id, type: 'subscription_payment', method: payload.method || 'cash', amount: money(paid), created_at: isoDateTime(), created_by: user.id })
  if (remaining > 0) {
    const debt = { id: nextId(db.debts), patient_id: patient.id, sponsor_id: subscription.sponsor_id, session_id: null, subscription_id: subscription.id, kind: 'subscription', amount: money(remaining), paid_amount: '0.000', remaining: money(remaining), status: 'open', created_at: isoDateTime() }
    db.debts.unshift(debt)
    patient.debt_total = money(Number(patient.debt_total) + remaining)
    if (debt.sponsor_id) {
      const sponsor = resourceById(db.sponsors, debt.sponsor_id)
      sponsor.total_due = money(Number(sponsor.total_due) + remaining)
      sponsor.remaining = money(Number(sponsor.remaining) + remaining)
    }
  }
  addAudit(user, 'subscription_created', 'subscription', subscription.id, null, subscription)
  return { data: clone(hydrateSubscription(subscription)) }
}

function subscriptionAction(config, id, action) {
  const user = requireAdmin(config)
  const subscription = resourceById(db.subscriptions, id, 'الاشتراك')
  const old = clone(subscription)
  const reason = config.data?.reason?.trim()
  if (['stop', 'correct'].includes(action) && !reason) fail(422, 'سبب الإجراء مطلوب.', { reason: ['أدخل السبب.'] })
  if (action === 'stop') subscription.status = 'stopped'
  if (action === 'reactivate') {
    if (subscription.remaining <= 0) fail(422, 'لا يمكن إعادة تفعيل اشتراك مكتمل دون جلسات متبقية.')
    subscription.status = 'active'
  }
  if (action === 'correct') {
    Object.assign(subscription, config.data.changes || {})
    subscription.remaining = subscription.sessions_count - subscription.sessions_used
  }
  addAudit(user, `subscription_${action}`, 'subscription', subscription.id, old, subscription, reason || null)
  return { data: clone(hydrateSubscription(subscription)) }
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
  return paginate(items, config.params)
}

function doctorDetails(config, id) {
  requireAuth(config)
  const doctor = resourceById(db.doctors, id, 'الطبيب')
  return { data: clone({ ...doctor, sessions: db.sessions.filter((item) => item.doctor_id === doctor.id && item.status === 'closed').map(hydrateSession), payouts: db.payouts.filter((item) => item.doctor_id === doctor.id) }) }
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
  const amount = Number(config.data?.amount || 0)
  if (amount <= 0 || amount > Number(doctor.remaining)) fail(422, 'مبلغ الصرف يجب أن يكون أكبر من صفر وألا يتجاوز المستحق المتبقي.', { amount: ['راجع المبلغ المتاح.'] })
  const payout = { id: nextId(db.payouts), doctor_id: doctor.id, amount: money(amount), date: config.data.date || isoDate(), method: config.data.method || 'cash', note: config.data.note || '', created_by: user.id }
  db.payouts.unshift(payout)
  doctor.paid = money(Number(doctor.paid) + amount)
  doctor.remaining = money(Number(doctor.remaining) - amount)
  addAudit(user, 'doctor_paid', 'doctor', doctor.id, null, payout, payout.note)
  return { data: clone({ payout, doctor }) }
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
  return { data: clone({ ...sponsor, patients, sessions: db.sessions.filter((item) => patientIds.includes(item.patient_id)).map(hydrateSession), debts: db.debts.filter((item) => item.sponsor_id === sponsor.id), payments: db.payments.filter((item) => patientIds.includes(item.patient_id)) }) }
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

  const subscription = payment.subscription_id ? db.subscriptions.find((item) => item.id === payment.subscription_id) : null
  if (subscription?.sponsor_id) sponsorIds.add(subscription.sponsor_id)

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
    let rows = db.sessions.map(hydrateSession)
    if (params.doctor_id) rows = rows.filter((item) => item.doctor_id === Number(params.doctor_id))
    if (params.patient_id) rows = rows.filter((item) => item.patient_id === Number(params.patient_id))
    if (params.status) rows = rows.filter((item) => item.status === params.status)
    if (params.from || params.to) rows = rows.filter((item) => inDateRange(item.opened_at, params))
    return { data: { rows: clone(rows), totals: { count: rows.length, service_value: money(sum(rows.filter((item) => item.status === 'closed').map((item) => item.net_total))) } } }
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
    let rows = db.doctors.map((item) => ({ ...item }))
    if (params.doctor_id) rows = rows.filter((item) => item.id === Number(params.doctor_id))
    return { data: { rows: clone(rows), totals: { earned: money(sum(rows.map((item) => item.earned))), paid: money(sum(rows.map((item) => item.paid))), remaining: money(sum(rows.map((item) => item.remaining))) } } }
  }
  let rows = db.subscriptions.map(hydrateSubscription)
  if (params.from || params.to) rows = rows.filter((item) => inDateRange(item.start_date, params))
  if (params.patient_id) rows = rows.filter((item) => item.patient_id === Number(params.patient_id))
  if (params.status) rows = rows.filter((item) => item.status === params.status)
  if (String(params.near_end) === '1') rows = rows.filter(isNearEnd)
  return { data: { rows: clone(rows), totals: { count: rows.length, active: rows.filter((item) => item.status === 'active').length, remaining_sessions: sum(rows.map((item) => item.remaining)), net_total: money(sum(rows.map((item) => item.net_total))) } } }
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
  if (segments[0] === 'sessions' && segments.length === 2 && method === 'GET') return { data: clone(hydrateSession(resourceById(db.sessions, segments[1], 'الجلسة'))) }
  if (segments[0] === 'sessions' && segments.length === 2 && method === 'PATCH') return updateSession(config, segments[1])
  if (segments[0] === 'sessions' && segments[2] === 'close' && method === 'POST') return closeSession(config, segments[1])
  if (segments[0] === 'sessions' && segments[2] === 'cancel' && method === 'POST') return cancelSession(config, segments[1])
  if (segments[0] === 'sessions' && segments[2] === 'correct' && method === 'POST') return correctSession(config, segments[1])

  if (path === '/subscriptions' && method === 'GET') return listSubscriptions(config)
  if (path === '/subscriptions' && method === 'POST') return createSubscription(config)
  if (segments[0] === 'subscriptions' && segments.length === 2 && method === 'GET') return subscriptionDetails(config, segments[1])
  if (segments[0] === 'subscriptions' && segments[2] === 'cancel-preview' && method === 'GET') return subscriptionCancellationPreview(config, segments[1])
  if (segments[0] === 'subscriptions' && segments[2] === 'cancel' && method === 'POST') return cancelSubscription(config, segments[1])
  if (segments[0] === 'subscriptions' && ['stop', 'reactivate', 'correct'].includes(segments[2]) && method === 'POST') return subscriptionAction(config, segments[1], segments[2])

  if (path === '/debts/settlements' && method === 'POST') return settleDebts(config)
  if (path === '/refunds' && method === 'POST') return createRefund(config)

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
