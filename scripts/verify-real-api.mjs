import { randomUUID } from 'node:crypto'

const baseUrl = process.env.API_URL || 'http://127.0.0.1:8000/api/v1'
const adminPassword = process.env.ADMIN_PASSWORD || 'Admin123!'
const receptionistPassword = process.env.RECEPTION_PASSWORD || 'Reception123!'
const runId = Date.now().toString().slice(-9)

class HttpError extends Error {
  constructor(status, body) {
    super(body?.message || `HTTP ${status}`)
    this.status = status
    this.body = body
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

function decimal(value) {
  return Number(value).toFixed(3)
}

function assertMoney(actual, expected, message) {
  assert(Math.abs(Number(actual) - Number(expected)) < 0.0005, `${message}: expected ${decimal(expected)}, received ${actual}`)
}

function clinicDate() {
  const parts = new Intl.DateTimeFormat('en', {
    timeZone: 'Africa/Tripoli', year: 'numeric', month: '2-digit', day: '2-digit',
  }).formatToParts(new Date())
  const values = Object.fromEntries(parts.map((part) => [part.type, part.value]))
  return `${values.year}-${values.month}-${values.day}`
}

async function request(method, path, { token, data, params, key, binary = false } = {}) {
  const url = new URL(`${baseUrl}${path}`)
  for (const [name, value] of Object.entries(params || {})) {
    if (value !== null && value !== undefined && value !== '') url.searchParams.set(name, String(value))
  }

  const headers = { Accept: binary ? '*/*' : 'application/json' }
  if (token) headers.Authorization = `Bearer ${token}`
  if (data !== undefined) headers['Content-Type'] = 'application/json'
  if (key) headers['Idempotency-Key'] = key

  const response = await fetch(url, {
    method,
    headers,
    body: data === undefined ? undefined : JSON.stringify(data),
  })

  if (binary && response.ok) {
    return {
      status: response.status,
      buffer: Buffer.from(await response.arrayBuffer()),
      contentType: response.headers.get('content-type') || '',
    }
  }

  const text = await response.text()
  let body = null
  try { body = text ? JSON.parse(text) : null } catch { body = { message: text } }
  if (!response.ok) throw new HttpError(response.status, body)
  return body
}

async function expectStatus(statuses, action, label) {
  const expected = Array.isArray(statuses) ? statuses : [statuses]
  try {
    await action()
    throw new Error(`${label}: expected HTTP ${expected.join(' or ')}`)
  } catch (error) {
    if (!(error instanceof HttpError) || !expected.includes(error.status)) throw error
    return error
  }
}

async function login(username, password) {
  return request('POST', '/auth/login', { data: { username, password } })
}

async function createSession(token, patientId, doctorId, serviceIds, settlement, discount = '0.000') {
  const response = await request('POST', '/sessions', {
    token,
    key: randomUUID(),
    data: {
      patient_id: patientId,
      doctor_id: doctorId,
      type: 'individual',
      service_ids: serviceIds,
      discount,
      settlement,
    },
  })
  return Object.assign(response.data.session, { effects: response.data.effects, replayed: response.data.replayed })
}

async function closeSession(token, sessionId, key = randomUUID()) {
  return request('POST', `/sessions/${sessionId}/close`, { token, data: {}, key })
}

const adminLogin = await login('admin', adminPassword)
const receptionistLogin = await login('reception', receptionistPassword)
const adminToken = adminLogin.data.token
const receptionistToken = receptionistLogin.data.token

const sponsor = (await request('POST', '/sponsors', {
  token: adminToken,
  data: { name: `جهة اختبار Phase 3 ${runId}`, phone: `092${runId}`, address: 'طرابلس', is_active: true },
})).data
const doctor = (await request('POST', '/doctors', {
  token: adminToken,
  data: { name: `د. اختبار Phase 3 ${runId}`, phone: `093${runId}`, commission_rate: '30.000', is_active: true },
})).data

const servicePrices = ['40.000', '60.000', '80.000', '75.000']
const services = []
for (const [index, price] of servicePrices.entries()) {
  const response = await request('POST', '/services', {
    token: adminToken,
    data: { name: `خدمة Phase 3 ${runId}-${index + 1}`, price, is_active: true },
  })
  services.push(response.data)
}

const existingPatients = await request('GET', '/patients', { token: receptionistToken, params: { per_page: 1 } })
assert(existingPatients.data.length === 1, 'Seeded patient is required for duplicate-phone verification')
await expectStatus(422, () => request('POST', '/patients', {
  token: receptionistToken,
  data: {
    name: `مريض مكرر ${runId}`,
    phone: existingPatients.data[0].phone,
    age: 35,
    gender: 'male',
    area: 'طرابلس',
    confirm_duplicate: false,
  },
}), 'Duplicate patient phone warning')

const patient = (await request('POST', '/patients', {
  token: receptionistToken,
  data: {
    name: `مريض تكامل Phase 3 ${runId}`,
    phone: `091${runId}`,
    age: 33,
    gender: 'male',
    area: 'حي الأندلس',
    emergency_phone: `094${runId}`,
    sponsor_id: sponsor.id,
    confirm_duplicate: false,
  },
})).data
const patientSearch = await request('GET', '/patients', {
  token: receptionistToken,
  params: { search: patient.phone, per_page: 100 },
})
assert(patientSearch.data.some((item) => item.id === patient.id), 'Patient search did not find the created patient')
process.stdout.write('PASS 1 receptionist login, duplicate warning, patient creation, and search\n')

const subscriptionResponse = await request('POST', '/subscriptions', {
  token: receptionistToken,
  key: randomUUID(),
  data: {
    patient_id: patient.id,
    name: `برنامج تأهيل Phase 3 ${runId}`,
    service_ids: services.slice(0, 3).map((service) => service.id),
    sessions_count: 10,
    total_price: '1700.000',
    paid_amount: '500.000',
    method: 'cash',
  },
})
const subscription = subscriptionResponse.data.subscription
assertMoney(subscription.total_price, 1700, 'Subscription manual total price')
assertMoney(subscription.received_amount, 500, 'Subscription received amount')
assertMoney(subscription.outstanding_amount, 1200, 'Subscription outstanding amount')
assert(subscription.start_date === clinicDate(), 'Subscription start date was not generated by the backend')
assert(subscriptionResponse.data.debt?.sponsor_id === null, 'Subscription debt must belong directly to the patient')
await expectStatus(422, () => request('POST', '/refunds', {
  token: adminToken,
  key: randomUUID(),
  data: {
    patient_id: patient.id,
    payment_id: subscriptionResponse.data.payment.id,
    amount: '10.000',
    method: 'cash',
    reason: 'محاولة استرجاع عامة للاشتراك',
  },
}), 'Generic subscription refund rejection')
await request('POST', '/debts/settlements', {
  token: receptionistToken,
  key: randomUUID(),
  data: { amount: '100.000', method: 'cash', allocations: [{ debt_id: subscriptionResponse.data.debt.id, amount: '100.000' }] },
})
const subscriptionAfterDebtPayment = await request('GET', `/subscriptions/${subscription.id}`, { token: receptionistToken })
assertMoney(subscriptionAfterDebtPayment.data.received_amount, 600, 'Subscription debt allocation attribution')
assertMoney(subscriptionAfterDebtPayment.data.outstanding_amount, 1100, 'Subscription outstanding amount after debt payment')
process.stdout.write('PASS 2 manual subscription price, backend date, partial payment, and debt allocation attribution\n')

const subscriptionSession = (await request('POST', '/sessions', {
  token: receptionistToken,
  key: randomUUID(),
  data: {
    patient_id: patient.id,
    doctor_id: doctor.id,
    type: 'subscription',
    subscription_id: subscription.id,
    session_date: clinicDate(),
    services: services.slice(0, 2).map((service) => ({ service_id: service.id, item_type: 'service', extra_units: 0 })),
  },
})).data
await closeSession(receptionistToken, subscriptionSession.session.id, randomUUID())
const closedSubscription = await request('GET', `/subscriptions/${subscription.id}`, { token: receptionistToken })
assert(closedSubscription.data.sessions_used === 1 && closedSubscription.data.remaining === 9, 'Subscription close did not consume exactly one session')
assert(subscriptionSession.effects.payment === null, 'Subscription session created a payment row')
assertMoney(subscriptionSession.session.net_total, 0, 'Subscription session financial value')
assert(subscriptionSession.session.commission_amount === null, 'Subscription session created doctor commission')
const subscriptionUsage = await request('GET', `/subscriptions/${subscription.id}/usage`, { token: receptionistToken, params: { per_page: 100 } })
assert(subscriptionUsage.data.filter((usage) => usage.type === 'consume').length === 1, 'Double close consumed the subscription more than once')
const sessionsByNumber = await request('GET', '/sessions', {
  token: adminToken,
  params: { search: subscriptionSession.session.code.replace(/^S-/, ''), per_page: 100 },
})
assert(sessionsByNumber.data.length === 1 && sessionsByNumber.data[0].id === subscriptionSession.session.id, 'Session number search did not find the matching session')
const paymentsAfterSubscription = await request('GET', `/patients/${patient.id}/payments`, { token: receptionistToken, params: { per_page: 100 } })
assert(!paymentsAfterSubscription.data.some((payment) => payment.session_id === subscriptionSession.session.id), 'Subscription session appears in cash receipts')
process.stdout.write('PASS 3 subscription consumption and double-submit idempotency\n')

const cashSession = await createSession(receptionistToken, patient.id, doctor.id, [services[0].id], {
  coverage: 'cash', wallet_amount: '0.000', paid_amount: '40.000', debt_amount: '0.000', method: 'cash', sponsor_id: null,
})

await expectStatus(422, () => createSession(receptionistToken, patient.id, doctor.id, [services[1].id], {
  coverage: 'wallet', wallet_amount: '60.000', paid_amount: '0.000', debt_amount: '0.000', method: null, sponsor_id: null,
}), 'Insufficient wallet')

await request('POST', `/patients/${patient.id}/wallet/topups`, {
  token: receptionistToken,
  key: randomUUID(),
  data: { amount: '200.000', method: 'cash', note: 'Phase 3 verification' },
})
const walletSession = await createSession(receptionistToken, patient.id, doctor.id, [services[1].id], {
  coverage: 'wallet', wallet_amount: '60.000', paid_amount: '0.000', debt_amount: '0.000', method: null, sponsor_id: null,
})

const debtSession = await createSession(receptionistToken, patient.id, doctor.id, [services[2].id], {
  coverage: 'debt', wallet_amount: '0.000', paid_amount: '0.000', debt_amount: '80.000', method: null, sponsor_id: sponsor.id,
})

const mixedSession = await createSession(receptionistToken, patient.id, doctor.id, [services[0].id, services[1].id], {
  coverage: 'mixed', wallet_amount: '20.000', paid_amount: '30.000', debt_amount: '50.000', method: 'transfer', sponsor_id: sponsor.id,
})
process.stdout.write('PASS 4 cash, wallet, insufficient-wallet, sponsor debt, and mixed settlement\n')

const oldPriceSession = await createSession(receptionistToken, patient.id, doctor.id, [services[3].id], {
  coverage: 'cash', wallet_amount: '0.000', paid_amount: '75.000', debt_amount: '0.000', method: 'cash', sponsor_id: null,
})
await request('PATCH', `/services/${services[3].id}`, {
  token: adminToken,
  data: { name: services[3].name, price: '95.000', is_active: true, reason: 'Phase 3 snapshot verification' },
})
const oldPriceDetails = await request('GET', `/sessions/${oldPriceSession.id}`, { token: adminToken })
assertMoney(oldPriceDetails.data.services[0].price, 75, 'Historical service snapshot')
const newPriceSession = await createSession(receptionistToken, patient.id, doctor.id, [services[3].id], {
  coverage: 'cash', wallet_amount: '0.000', paid_amount: '95.000', debt_amount: '0.000', method: 'cash', sponsor_id: null,
})
assertMoney(newPriceSession.services[0].price, 95, 'New session service price')
await request('POST', `/sessions/${newPriceSession.id}/cancel`, {
  token: adminToken,
  key: randomUUID(),
  data: { reason: 'إنهاء اختبار لقطة السعر' },
})
process.stdout.write('PASS 5 historical price snapshots and new prices\n')

await expectStatus(403, () => request('PATCH', `/sessions/${cashSession.id}`, {
  token: receptionistToken,
  data: { doctor_id: doctor.id },
}), 'Receptionist closed-session edit denial')
await expectStatus(403, () => request('POST', `/sessions/${cashSession.id}/cancel`, {
  token: receptionistToken,
  key: randomUUID(),
  data: { reason: 'محاولة غير مسموحة' },
}), 'Receptionist closed-session cancellation denial')
const correction = await request('POST', `/sessions/${cashSession.id}/correct`, {
  token: adminToken,
  key: randomUUID(),
  data: {
    changes: {
      discount: '1.000',
      settlement: { wallet_amount: '0.000', paid_amount: '39.000', debt_amount: '0.000', method: 'cash', sponsor_id: null },
    },
    reason: 'Phase 3 approved correction',
  },
})
assertMoney(correction.data.financial_impact.net_delta, -1, 'Closed-session correction delta')
process.stdout.write('PASS 6 receptionist denial and audited admin correction\n')

await request('POST', `/sessions/${subscriptionSession.session.id}/cancel`, {
  token: adminToken,
  key: randomUUID(),
  data: { reason: 'Phase 3 subscription restore' },
})
const subscriptionAfterCancel = await request('GET', `/subscriptions/${subscription.id}`, { token: adminToken })
assert(subscriptionAfterCancel.data.sessions_used === 0 && subscriptionAfterCancel.data.remaining === 10, 'Cancelled subscription session was not restored')

const patientBeforeWalletCancel = (await request('GET', `/patients/${patient.id}`, { token: adminToken })).data
await request('POST', `/sessions/${walletSession.id}/cancel`, {
  token: adminToken,
  key: randomUUID(),
  data: { reason: 'Phase 3 wallet reversal' },
})
const patientAfterWalletCancel = (await request('GET', `/patients/${patient.id}`, { token: adminToken })).data
assertMoney(Number(patientAfterWalletCancel.wallet_balance) - Number(patientBeforeWalletCancel.wallet_balance), 60, 'Wallet reversal amount')

await request('POST', `/sessions/${cashSession.id}/cancel`, {
  token: adminToken,
  key: randomUUID(),
  data: { reason: 'Phase 3 cash refund' },
})
const cancelledCash = await request('GET', `/sessions/${cashSession.id}`, { token: adminToken })
assert(cancelledCash.data.commission_amount === null, 'Cancelled cash session retained doctor commission')
const paymentsAfterCashCancel = await request('GET', `/patients/${patient.id}/payments`, { token: adminToken, params: { per_page: 100 } })
assert(paymentsAfterCashCancel.data.some((payment) => payment.type === 'refund' && payment.session_id === cashSession.id), 'Cash cancellation did not create a refund')
process.stdout.write('PASS 7 closed-session subscription, wallet, cash, and commission reversals\n')

const patientDebts = await request('GET', `/patients/${patient.id}/debts`, { token: receptionistToken, params: { per_page: 100 } })
const targetDebts = patientDebts.data.filter((debt) => [debtSession.id, mixedSession.id].includes(debt.session_id) && ['open', 'partial'].includes(debt.status))
assert(targetDebts.length === 2, 'Expected two open session debts for split allocation')
const settlement = await request('POST', '/debts/settlements', {
  token: receptionistToken,
  key: randomUUID(),
  data: {
    amount: '2.000',
    method: 'cash',
    allocations: targetDebts.map((debt) => ({ debt_id: debt.id, amount: '1.000' })),
  },
})
assert(settlement.data.allocations.length === 2, 'Debt payment did not split across two debts')
const refreshedDebts = await request('GET', `/patients/${patient.id}/debts`, { token: receptionistToken, params: { per_page: 100 } })
const overTarget = refreshedDebts.data.find((debt) => debt.id === targetDebts[0].id)
const excessive = decimal(Number(overTarget.remaining) + 1)
await expectStatus(422, () => request('POST', '/debts/settlements', {
  token: receptionistToken,
  key: randomUUID(),
  data: { amount: excessive, method: 'cash', allocations: [{ debt_id: overTarget.id, amount: excessive }] },
}), 'Debt over-allocation')
const sponsorStatement = await request('GET', `/sponsors/${sponsor.id}/statement`, { token: adminToken })
assert(sponsorStatement.data.allocations.some((allocation) => allocation.payment_id === settlement.data.payment.id), 'Sponsor statement omitted debt allocation')
process.stdout.write('PASS 8 split debt allocation, over-allocation rejection, and sponsor statement\n')

const duesBeforePayout = await request('GET', `/doctors/${doctor.id}/dues`, { token: adminToken })
const earnedFromSessions = duesBeforePayout.data.sessions.reduce((total, session) => total + Number(session.commission_amount || 0), 0)
assertMoney(duesBeforePayout.data.doctor.earned, earnedFromSessions, 'Doctor earned dues')
const payoutAmount = Math.min(10, Number(duesBeforePayout.data.all_time_remaining))
assert(payoutAmount > 0, 'Doctor must have positive dues before payout')
await request('POST', `/doctors/${doctor.id}/payouts`, {
  token: adminToken,
  key: randomUUID(),
  data: { amount: decimal(payoutAmount), date: clinicDate(), method: 'cash', note: 'Phase 3 payout' },
})
const duesAfterPayout = await request('GET', `/doctors/${doctor.id}/dues`, { token: adminToken })
assertMoney(Number(duesBeforePayout.data.all_time_remaining) - Number(duesAfterPayout.data.all_time_remaining), payoutAmount, 'Doctor payout reduction')
const oldRateSession = await request('GET', `/sessions/${debtSession.id}`, { token: adminToken })
await request('PATCH', `/doctors/${doctor.id}`, {
  token: adminToken,
  data: { name: doctor.name, phone: doctor.phone, commission_rate: '35.000', is_active: true, reason: 'Phase 3 rate snapshot verification' },
})
const oldRateSessionAfterChange = await request('GET', `/sessions/${debtSession.id}`, { token: adminToken })
assertMoney(oldRateSessionAfterChange.data.commission_rate, oldRateSession.data.commission_rate, 'Historical doctor rate snapshot')
const newRateSession = await createSession(receptionistToken, patient.id, doctor.id, [services[0].id], {
  coverage: 'cash', wallet_amount: '0.000', paid_amount: '40.000', debt_amount: '0.000', method: 'cash', sponsor_id: null,
})
assertMoney(newRateSession.commission_rate, 35, 'Future doctor rate snapshot')
process.stdout.write('PASS 9 doctor dues, payout, and rate snapshots\n')

const correctedSubscription = await request('POST', `/subscriptions/${subscription.id}/correct`, {
  token: adminToken,
  key: randomUUID(),
  data: { changes: { sessions_count: 12, total_price: '1800.000' }, reason: 'Phase 3 subscription correction audit' },
})
assert(correctedSubscription.data.subscription.sessions_count === 12, 'Subscription session-count correction failed')
assertMoney(correctedSubscription.data.subscription.total_price, 1800, 'Subscription price correction')
const cancellationSubscription = (await request('POST', '/subscriptions', {
  token: receptionistToken,
  key: randomUUID(),
  data: {
    patient_id: patient.id,
    name: `اشتراك إلغاء Phase 3 ${runId}`,
    service_ids: services.slice(0, 2).map((service) => service.id),
    sessions_count: 5,
    total_price: '200.000',
    paid_amount: '100.000',
    method: 'cash',
  },
})).data.subscription
const cancellationPreview = await request('GET', `/subscriptions/${cancellationSubscription.id}/cancel-preview`, { token: adminToken })
assertMoney(cancellationPreview.data.received_amount, 100, 'Subscription cancellation received amount')
assertMoney(cancellationPreview.data.maximum_refund, 100, 'Subscription cancellation maximum refund')
const cancellationResult = await request('POST', `/subscriptions/${cancellationSubscription.id}/cancel`, {
  token: adminToken,
  key: randomUUID(),
  data: { reason: 'Phase 3 cancellation audit', refund_amount: '40.000', refund_method: 'cash' },
})
assert(cancellationResult.data.subscription.status === 'cancelled', 'Subscription cancellation did not set cancelled status')
assertMoney(cancellationResult.data.refund.amount, -40, 'Subscription cancellation explicit refund')

const reportTypes = ['sessions', 'collections', 'debts', 'doctor-dues', 'subscriptions']
for (const type of reportTypes) {
  const params = type === 'doctor-dues' ? { doctor_id: doctor.id } : { patient_id: patient.id }
  const report = await request('GET', `/reports/${type}`, { token: adminToken, params })
  assert(Array.isArray(report.data.rows) && report.data.totals, `Report ${type} did not return rows and totals`)
}
const today = clinicDate()
const dashboard = await request('GET', '/dashboard', { token: adminToken, params: { date: today } })
const sessionsToday = await request('GET', '/reports/sessions', { token: adminToken, params: { from: today, to: today } })
const collectionsToday = await request('GET', '/reports/collections', { token: adminToken, params: { from: today, to: today } })
const debtsReport = await request('GET', '/reports/debts', { token: adminToken })
const doctorDuesReport = await request('GET', '/reports/doctor-dues', { token: adminToken })
const stats = Object.fromEntries(dashboard.data.stats.map((stat) => [stat.key, stat.value]))
assertMoney(stats.service_value_today, sessionsToday.data.totals.service_value, 'Dashboard service-value drill-down')
assertMoney(stats.collections_today, collectionsToday.data.totals.net, 'Dashboard collections drill-down')
assertMoney(stats.debts_remaining, debtsReport.data.totals.remaining, 'Dashboard debt drill-down')
assertMoney(stats.doctor_dues_remaining, doctorDuesReport.data.totals.remaining, 'Dashboard doctor-dues drill-down')
for (const format of ['pdf', 'xlsx']) {
  const exported = await request('GET', '/reports/sessions/export', { token: adminToken, params: { patient_id: patient.id, format }, binary: true })
  assert(exported.buffer.length > 100, `${format.toUpperCase()} export is unexpectedly empty`)
  if (format === 'pdf') assert(exported.buffer.subarray(0, 4).toString() === '%PDF', 'PDF export signature is invalid')
  if (format === 'xlsx') assert(exported.buffer[0] === 0x50 && exported.buffer[1] === 0x4b, 'XLSX export signature is invalid')
}
process.stdout.write('PASS 10 report filters, exports, and dashboard drill-down totals\n')

const newUsername = `phase3_${runId}`
const newPassword = 'Reception123!'
const newUser = (await request('POST', '/users', {
  token: adminToken,
  data: { name: `موظف اختبار ${runId}`, username: newUsername, password: newPassword, password_confirmation: newPassword, is_active: true },
})).data
const newUserLogin = await login(newUsername, newPassword)
await request('PATCH', `/users/${newUser.id}`, {
  token: adminToken,
  data: { name: newUser.name, username: newUser.username, is_active: false },
})
await expectStatus(401, () => request('GET', '/auth/me', { token: newUserLogin.data.token }), 'Disabled-user token revocation')
await expectStatus(403, () => login(newUsername, newPassword), 'Disabled-user login denial')
await expectStatus([404, 405], () => request('DELETE', `/users/${newUser.id}`, { token: adminToken }), 'No user delete route')
process.stdout.write('PASS 11 user creation, disable, token revocation, login denial, and no delete route\n')

const expectedAuditActions = [
  'patient_created', 'subscription_created', 'subscription_corrected', 'subscription_cancelled',
  'session_created', 'session_closed', 'session_corrected', 'session_cancelled',
  'wallet_topped_up', 'payment_recorded', 'payment_refunded', 'debt_payment_recorded',
  'service_price_changed', 'doctor_rate_changed', 'doctor_payout_created', 'user_created', 'user_updated',
]
for (const action of expectedAuditActions) {
  const audit = await request('GET', '/audit-log', { token: adminToken, params: { action, date: today, per_page: 100 } })
  assert(audit.data.length > 0, `Audit log is missing action ${action}`)
}
const latestAudit = await request('GET', '/audit-log', { token: adminToken, params: { per_page: 1 } })
await expectStatus([404, 405], () => request('PATCH', `/audit-log/${latestAudit.data[0].id}`, { token: adminToken, data: { reason: 'tamper' } }), 'Audit log write denial')
process.stdout.write('PASS 12 required audit actions and append-only routes\n')

process.stdout.write(`Real API walkthrough passed all 12 fixed-session subscription scenarios. Test patient #${patient.id}, doctor #${doctor.id}, sponsor #${sponsor.id}.\n`)
