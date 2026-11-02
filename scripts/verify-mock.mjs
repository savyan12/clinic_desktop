import { createServer } from 'vite'

const storage = new Map()
globalThis.localStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, value),
  removeItem: (key) => storage.delete(key),
}

const server = await createServer({ server: { middlewareMode: true }, appType: 'custom', logLevel: 'silent' })
const { mockRequest } = await server.ssrLoadModule('/src/mocks/handlers.js')

let token = null
const call = (method, url, data = undefined, params = undefined) => mockRequest({
  method,
  url,
  data,
  params,
  headers: token ? { Authorization: `Bearer ${token}` } : {},
})

const assert = (condition, message) => {
  if (!condition) throw new Error(message)
}

const expectStatus = async (status, action, label) => {
  try {
    await action()
    throw new Error(`${label}: expected HTTP ${status}`)
  } catch (error) {
    if (error.status !== status) throw error
  }
}

try {
  const receptionLogin = await call('POST', '/auth/login', { username: 'reception', password: 'demo' })
  token = receptionLogin.data.token
  localStorage.setItem('clinic-auth', JSON.stringify(receptionLogin.data))
  assert(receptionLogin.data.user.role === 'receptionist', 'Receptionist login failed')

  await expectStatus(422, () => call('POST', '/patients', {
    name: 'سجل مكرر للاختبار', phone: '0912345678', age: 30, gender: 'male', area: 'طرابلس', confirm_duplicate: false,
  }), 'Duplicate phone warning')

  const patientResponse = await call('POST', '/patients', {
    name: 'مريض اختبار الواجهة', phone: '0910000029', age: 32, gender: 'male', area: 'طرابلس', confirm_duplicate: false,
  })
  const patient = patientResponse.data

  const subscriptionResponse = await call('POST', '/subscriptions', {
    patient_id: patient.id,
    service_ids: [1, 2],
    sessions_count: 3,
    discount: '15.000',
    paid_amount: '100.000',
    method: 'cash',
    start_date: new Date().toISOString().slice(0, 10),
    expected_end_date: null,
    sponsor_id: 1,
  })
  const subscription = subscriptionResponse.data
  assert(subscription.remaining === 3, 'Subscription creation did not preserve remaining sessions')

  const subscriptionSession = (await call('POST', '/sessions', {
    patient_id: patient.id, doctor_id: 1, type: 'subscription', subscription_id: subscription.id,
  })).data
  const subscriptionClose = await call('POST', `/sessions/${subscriptionSession.id}/close`, {})
  assert(subscriptionClose.data.effects.payment === null, 'Subscription close created a cash payment')
  const repeatedClose = await call('POST', `/sessions/${subscriptionSession.id}/close`, {})
  assert(repeatedClose.data.replayed === true, 'Repeated close was not idempotent')

  const cashSession = (await call('POST', '/sessions', {
    patient_id: patient.id, doctor_id: 1, type: 'individual', service_ids: [1], discount: '0.000',
    settlement: { coverage: 'cash', wallet_amount: '0.000', paid_amount: '80.000', debt_amount: '0.000', method: 'cash', sponsor_id: null },
  })).data
  const cashClose = await call('POST', `/sessions/${cashSession.id}/close`, {})
  assert(cashClose.data.effects.payment?.amount === '80.000', 'Cash close did not create the expected payment')

  await expectStatus(422, () => call('POST', '/sessions', {
    patient_id: patient.id, doctor_id: 2, type: 'individual', service_ids: [3], discount: '0.000',
    settlement: { coverage: 'wallet', wallet_amount: '65.000', paid_amount: '0.000', debt_amount: '0.000', method: 'cash', sponsor_id: null },
  }), 'Insufficient wallet')

  await call('POST', `/patients/${patient.id}/wallet/topups`, { amount: '100.000', method: 'cash' })
  const walletSession = (await call('POST', '/sessions', {
    patient_id: patient.id, doctor_id: 2, type: 'individual', service_ids: [3], discount: '0.000',
    settlement: { coverage: 'wallet', wallet_amount: '65.000', paid_amount: '0.000', debt_amount: '0.000', method: 'cash', sponsor_id: null },
  })).data
  const walletClose = await call('POST', `/sessions/${walletSession.id}/close`, {})
  assert(walletClose.data.effects.wallet_transaction?.amount === '-65.000', 'Wallet close did not create the debit')

  const debtSession = (await call('POST', '/sessions', {
    patient_id: patient.id, doctor_id: 3, type: 'individual', service_ids: [2], discount: '0.000',
    settlement: { coverage: 'debt', wallet_amount: '0.000', paid_amount: '0.000', debt_amount: '45.000', method: 'cash', sponsor_id: 1 },
  })).data
  const debtClose = await call('POST', `/sessions/${debtSession.id}/close`, {})
  assert(debtClose.data.effects.debt?.amount === '45.000', 'Debt close did not create the debt')

  const mixedSession = (await call('POST', '/sessions', {
    patient_id: patient.id, doctor_id: 3, type: 'individual', service_ids: [3], discount: '0.000',
    settlement: { coverage: 'mixed', wallet_amount: '20.000', paid_amount: '20.000', debt_amount: '25.000', method: 'transfer', sponsor_id: 1 },
  })).data
  const mixedClose = await call('POST', `/sessions/${mixedSession.id}/close`, {})
  assert(mixedClose.data.effects.payment && mixedClose.data.effects.wallet_transaction && mixedClose.data.effects.debt, 'Mixed close did not create all three effects')

  await expectStatus(403, () => call('PATCH', `/sessions/${cashSession.id}`, { doctor_id: 2 }), 'Receptionist closed-session edit denial')

  const patientDetails = (await call('GET', `/patients/${patient.id}`)).data
  const openDebts = patientDetails.debts.filter((debt) => ['open', 'partial'].includes(debt.status))
  assert(openDebts.length > 0, 'Expected debts were not visible in patient account')
  const allocationAmount = Math.min(30, Number(openDebts[0].remaining)).toFixed(3)
  await call('POST', '/debts/settlements', {
    amount: allocationAmount,
    method: 'cash',
    allocations: [{ debt_id: openDebts[0].id, amount: allocationAmount }],
  })
  await expectStatus(422, () => call('POST', '/debts/settlements', {
    amount: '9999.000', method: 'cash', allocations: [{ debt_id: openDebts[0].id, amount: '9999.000' }],
  }), 'Debt over-allocation')

  const adminLogin = await call('POST', '/auth/login', { username: 'admin', password: 'demo' })
  token = adminLogin.data.token
  localStorage.setItem('clinic-auth', JSON.stringify(adminLogin.data))
  assert(adminLogin.data.user.role === 'admin', 'Admin login failed')

  const cancellationPreview = await call('GET', `/subscriptions/${subscription.id}/cancel-preview`)
  assert(cancellationPreview.data.sessions_used === 1, 'Subscription cancellation preview did not include usage')
  const cancellation = await call('POST', `/subscriptions/${subscription.id}/cancel`, { reason: 'إلغاء تحقق الواجهة' })
  assert(cancellation.data.subscription.status === 'cancelled', 'Subscription cancellation did not set the terminal status')
  assert(cancellation.data.refund === null && Number(cancellation.data.replacement_debt?.amount) > 0, 'Subscription cancellation did not reconcile the consumed balance')

  const doctor = (await call('GET', '/doctors/1')).data
  const payoutAmount = Math.min(50, Number(doctor.remaining)).toFixed(3)
  await call('POST', '/doctors/1/payouts', { amount: payoutAmount, date: new Date().toISOString().slice(0, 10), method: 'cash', note: 'Phase 1 verification' })

  for (const report of ['sessions', 'collections', 'debts', 'doctor-dues', 'subscriptions']) {
    const response = await call('GET', `/reports/${report}`, undefined, {})
    assert(Array.isArray(response.data.rows), `Report ${report} did not return rows`)
  }

  const audit = await call('GET', '/audit-log', undefined, { per_page: 100 })
  assert(audit.data.some((entry) => entry.action === 'doctor_paid'), 'Payout was not written to audit log')
  assert(audit.data.some((entry) => entry.action === 'subscription_cancelled'), 'Subscription cancellation was not written to audit log')

  process.stdout.write('Mock walkthrough passed: login, duplicate warning, patient, subscription creation/cancellation, subscription/cash/wallet/debt/mixed session close, idempotency, role denial, wallet top-up, debt settlement limits, payout, reports, and audit.\n')
} finally {
  await server.close()
}
