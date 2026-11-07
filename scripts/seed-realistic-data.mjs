import { createHash, randomBytes, randomUUID } from 'node:crypto'
import { execFile } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)

class HttpError extends Error {
  constructor(method, route, status, body) {
    super(`${method} ${route} failed with HTTP ${status}: ${body?.message || JSON.stringify(body)}`)
    this.status = status
    this.body = body
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

function money(value) {
  return Number(value).toFixed(3)
}

function clinicDate() {
  const parts = new Intl.DateTimeFormat('en', {
    timeZone: 'Africa/Tripoli',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).formatToParts(new Date())
  const values = Object.fromEntries(parts.map((part) => [part.type, part.value]))
  return `${values.year}-${values.month}-${values.day}`
}

function formatDate(date) {
  return date.toISOString().slice(0, 10)
}

function dateFromClinic(value = clinicDate()) {
  const [year, month, day] = value.split('-').map(Number)
  return new Date(Date.UTC(year, month - 1, day, 12))
}

function daysAgo(days) {
  const date = dateFromClinic()
  date.setUTCDate(date.getUTCDate() - days)
  return formatDate(date)
}

function monthInfo(offset) {
  const today = dateFromClinic()
  const first = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth() + offset, 1, 12))
  const last = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth() + offset + 1, 0, 12))
  return {
    month: formatDate(first).slice(0, 7),
    start: formatDate(first),
    end: formatDate(last),
    paymentDate: offset === 0 ? clinicDate() : `${formatDate(first).slice(0, 8)}05`,
    arabicName: new Intl.DateTimeFormat('ar-LY', { month: 'long', timeZone: 'UTC' }).format(first),
  }
}

function sqlString(value) {
  return `'${String(value).replaceAll('\\', '\\\\').replaceAll("'", "''")}'`
}

async function mysqlQuery(localRuntime, sql) {
  const args = [
    `--defaults-extra-file=${localRuntime.rootClientFile}`,
    '--default-character-set=utf8mb4',
    '--batch',
    '--skip-column-names',
    `--database=${localRuntime.database}`,
    '-e',
    sql,
  ]
  const { stdout } = await execFileAsync(localRuntime.mysqlBinary, args, { maxBuffer: 1024 * 1024 })
  return stdout.trim()
}

async function localRuntimeInfo() {
  const runtimeDir = process.env.CLINIC_RUNTIME_DIR
    || path.join(process.env.XDG_DATA_HOME || path.join(os.homedir(), '.local', 'share'), 'com.ettizan.clinic', 'runtime')
  const config = JSON.parse(await readFile(path.join(runtimeDir, 'install.json'), 'utf8'))
  assert(config.activated, 'نسخة التطبيق المحلية غير مفعلة.')
  assert(/^[A-Za-z0-9_]+$/.test(config.database), 'اسم قاعدة البيانات المحلية غير صالح.')
  return {
    baseUrl: `http://127.0.0.1:${config.api_port}/api/v1`,
    database: config.database,
    rootClientFile: path.join(runtimeDir, 'mysql-root.cnf'),
    mysqlBinary: process.env.MYSQL_BIN || '/usr/bin/mysql',
  }
}

async function resetOperationalData(localRuntime) {
  const tables = [
    'payment_refund_allocations',
    'payment_allocations',
    'wallet_transactions',
    'payments',
    'debts',
    'subscription_usages',
    'session_services',
    'session_additions',
    'sessions',
    'subscription_services',
    'subscriptions',
    'doctor_payouts',
    'administrative_expenses',
    'patients',
    'services',
    'doctors',
    'sponsors',
    'audit_logs',
    'idempotency_keys',
  ]
  const statements = tables.map((table) => `TRUNCATE TABLE \`${table}\`;`).join('\n')
  await mysqlQuery(localRuntime, `
    SET FOREIGN_KEY_CHECKS = 0;
    ${statements}
    SET FOREIGN_KEY_CHECKS = 1;
  `)
}

async function issueLocalToken(localRuntime, role, batchId) {
  const userId = await mysqlQuery(localRuntime, `
    SELECT id FROM users
    WHERE role = ${sqlString(role)} AND is_active = 1
    ORDER BY id LIMIT 1;
  `)
  if (!userId) return null

  const plain = randomBytes(32).toString('base64url')
  const digest = createHash('sha256').update(plain).digest('hex')
  const tokenName = `realistic-seed-${batchId}`
  const tokenId = await mysqlQuery(localRuntime, `
    INSERT INTO personal_access_tokens
      (tokenable_type, tokenable_id, name, token, abilities, expires_at, created_at, updated_at)
    VALUES
      ('App\\\\Models\\\\User', ${Number(userId)}, ${sqlString(tokenName)}, ${sqlString(digest)}, '["*"]', DATE_ADD(NOW(), INTERVAL 2 HOUR), NOW(), NOW());
    SELECT LAST_INSERT_ID();
  `)
  assert(tokenId, `تعذر إنشاء رمز ${role} المؤقت.`)
  return { bearer: `${tokenId}|${plain}`, id: Number(tokenId) }
}

async function revokeLocalTokens(localRuntime, tokenIds, batchId) {
  if (!tokenIds.length) return
  const ids = tokenIds.filter(Number.isInteger).join(',')
  if (!ids) return
  await mysqlQuery(localRuntime, `
    DELETE FROM personal_access_tokens
    WHERE id IN (${ids}) AND name = ${sqlString(`realistic-seed-${batchId}`)};
  `)
}

async function request(baseUrl, method, route, { token, data, params, idempotent = false, binary = false } = {}) {
  const url = new URL(`${baseUrl}${route}`)
  for (const [key, value] of Object.entries(params || {})) {
    if (value !== null && value !== undefined && value !== '') url.searchParams.set(key, String(value))
  }
  const headers = { Accept: binary ? '*/*' : 'application/json' }
  if (token) headers.Authorization = `Bearer ${token}`
  if (data !== undefined) headers['Content-Type'] = 'application/json'
  if (idempotent) headers['Idempotency-Key'] = randomUUID()

  const response = await fetch(url, {
    method,
    headers,
    body: data === undefined ? undefined : JSON.stringify(data),
  })
  if (binary && response.ok) {
    return {
      status: response.status,
      contentType: response.headers.get('content-type') || '',
      buffer: Buffer.from(await response.arrayBuffer()),
    }
  }
  const text = await response.text()
  let body = null
  try { body = text ? JSON.parse(text) : null } catch { body = { message: text } }
  if (!response.ok) throw new HttpError(method, route, response.status, body)
  return body
}

function sessionPayload({ patient, doctor, services, date, discount = 0, settlement }) {
  return {
    patient_id: patient.id,
    doctor_id: doctor.id,
    type: 'individual',
    session_date: date,
    services,
    discount: money(discount),
    settlement: {
      wallet_amount: money(settlement.wallet || 0),
      paid_amount: money(settlement.paid || 0),
      debt_amount: money(settlement.debt || 0),
      method: settlement.method || null,
      sponsor_id: settlement.sponsorId || null,
    },
  }
}

async function seed({ baseUrl, adminToken, receptionistToken, batchId }) {
  const admin = (method, route, options = {}) => request(baseUrl, method, route, { ...options, token: adminToken })
  const reception = (method, route, options = {}) => request(baseUrl, method, route, { ...options, token: receptionistToken })
  const created = {
    sponsors: [], doctors: [], services: [], patients: [], sessions: [], subscriptions: [],
    expenses: [], debts: [], payouts: [],
  }

  const health = await request(baseUrl, 'GET', '/health')
  assert(health.status === 'ok', 'خدمة API المحلية غير جاهزة.')
  process.stdout.write(`بدء تعبئة البيانات الواقعية (${batchId})\n`)

  process.stdout.write('1/9 إنشاء الجهات والأطباء والخدمات...\n')
  const sponsorDefinitions = [
    { key: 'insurance', name: 'شركة المدار للتأمين الصحي', phone: '0213604100', address: 'طرابلس - حي الأندلس' },
    { key: 'social', name: 'صندوق الرعاية الاجتماعية', phone: '0214443270', address: 'طرابلس - الظهرة' },
    { key: 'company', name: 'شركة الواحة للخدمات', phone: '0912203040', address: 'طرابلس - قرقارش' },
  ]
  const sponsors = {}
  for (const definition of sponsorDefinitions) {
    const response = await admin('POST', '/sponsors', {
      data: { name: definition.name, phone: definition.phone, address: definition.address, is_active: true },
    })
    sponsors[definition.key] = response.data
    created.sponsors.push(response.data.id)
  }

  const doctorDefinitions = [
    { key: 'mohamed', name: 'د. محمد الورفلي', phone: '0913101101', rate: 30, active: true },
    { key: 'sara', name: 'د. سارة الفيتوري', phone: '0923101102', rate: 25, active: true },
    { key: 'ayman', name: 'د. أيمن المصراتي', phone: '0943101103', rate: 35, active: true },
    { key: 'reem', name: 'د. ريم الطرابلسي', phone: '0913101104', rate: 20, active: false },
  ]
  const doctors = {}
  for (const definition of doctorDefinitions) {
    const response = await admin('POST', '/doctors', {
      data: {
        name: definition.name, phone: definition.phone,
        commission_rate: money(definition.rate), is_active: definition.active,
      },
    })
    doctors[definition.key] = response.data
    created.doctors.push(response.data.id)
  }

  const serviceDefinitions = [
    { key: 'assessment', name: 'تقييم أولي للحالة', price: 40, active: true },
    { key: 'physio', name: 'جلسة علاج طبيعي', price: 65, active: true },
    { key: 'manual', name: 'علاج يدوي وتحريك مفاصل', price: 75, active: true },
    { key: 'rehab', name: 'تأهيل ما بعد الجراحة', price: 90, active: true },
    { key: 'balance', name: 'تمارين اتزان وتقوية', price: 50, active: true },
    { key: 'children', name: 'جلسة علاج طبيعي للأطفال', price: 55, active: true },
    { key: 'home', name: 'جلسة علاج منزلي', price: 110, active: true, includedUnits: 45, unitName: 'دقيقة', extraUnitPrice: 3 },
    { key: 'followup', name: 'جلسة متابعة قصيرة', price: 45, active: true },
    { key: 'archived', name: 'خدمة قديمة غير مستخدمة', price: 30, active: false },
  ]
  const services = {}
  for (const definition of serviceDefinitions) {
    const response = await admin('POST', '/services', {
      data: {
        name: definition.name, price: money(definition.price), is_active: definition.active,
        included_units: definition.includedUnits ?? null,
        unit_name: definition.unitName ?? null,
        extra_unit_price: definition.extraUnitPrice === undefined ? null : money(definition.extraUnitPrice),
      },
    })
    services[definition.key] = response.data
    created.services.push(response.data.id)
  }

  process.stdout.write('2/9 إنشاء ملفات مرضى متنوعة...\n')
  const phoneSeed = 1_000_000 + (Date.now() % 8_900_000)
  const phone = (index) => `${['091', '092', '094'][index % 3]}${String((phoneSeed + index) % 10_000_000).padStart(7, '0')}`
  const patientDefinitions = [
    { key: 'ahmed', name: 'أحمد سالم الجروشي', age: 42, gender: 'male', area: 'حي الأندلس', sponsor: sponsors.insurance },
    { key: 'fatima', name: 'فاطمة علي بن موسى', age: 58, gender: 'female', area: 'قرقارش' },
    { key: 'mahmoud', name: 'محمود الترهوني', age: 35, gender: 'male', area: 'عين زارة' },
    { key: 'maryam', name: 'مريم البوسيفي', age: 29, gender: 'female', area: 'الفرناج' },
    { key: 'yousef', name: 'يوسف السنوسي', age: 12, gender: 'male', area: 'تاجوراء', sponsor: sponsors.social },
    { key: 'khadija', name: 'خديجة الزروق', age: 67, gender: 'female', area: 'سوق الجمعة' },
    { key: 'salem', name: 'سالم الورفلي', age: 50, gender: 'male', area: 'جنزور' },
    { key: 'layla', name: 'ليلى المدني', age: 31, gender: 'female', area: 'زاوية الدهماني' },
    { key: 'ibrahim', name: 'إبراهيم الفلاح', age: null, gender: null, area: null, sponsor: sponsors.company },
    { key: 'najwa', name: 'نجوى عبدالله العباني', age: 44, gender: 'female', area: 'أبو سليم', inactive: true },
  ]
  const patients = {}
  for (const [index, definition] of patientDefinitions.entries()) {
    const response = await reception('POST', '/patients', {
      data: {
        name: definition.name, phone: phone(index), age: definition.age, gender: definition.gender,
        area: definition.area, emergency_phone: phone(index + 20),
        sponsor_id: definition.sponsor?.id || null, is_active: !definition.inactive,
        confirm_duplicate: false,
      },
    })
    patients[definition.key] = response.data
    created.patients.push(response.data.id)
  }

  let duplicateWarningVerified = false
  try {
    await reception('POST', '/patients', {
      data: {
        name: 'أحمد سالم الجروشي', phone: patients.ahmed.phone, age: 42, gender: 'male',
        area: 'حي الأندلس', emergency_phone: null, sponsor_id: null,
        is_active: true, confirm_duplicate: false,
      },
    })
  } catch (error) {
    duplicateWarningVerified = error instanceof HttpError && error.status === 422
  }
  assert(duplicateWarningVerified, 'لم يعمل تحذير تكرار رقم هاتف المريض.')

  process.stdout.write('3/9 تعبئة المحافظ والجلسات وطرق التسوية...\n')
  const ahmedTopup = await reception('POST', `/patients/${patients.ahmed.id}/wallet/topups`, {
    idempotent: true,
    data: { amount: money(100), method: 'cash', note: 'شحن رصيد تمهيدي للمريض' },
  })
  const fatimaTopup = await reception('POST', `/patients/${patients.fatima.id}/wallet/topups`, {
    idempotent: true,
    data: { amount: money(300), method: 'transfer', note: 'تحويل مصرفي إلى رصيد المريضة' },
  })
  await admin('POST', '/refunds', {
    idempotent: true,
    data: {
      patient_id: patients.fatima.id, payment_id: fatimaTopup.data.payment.id, amount: money(25),
      method: 'transfer', reason: 'إرجاع زيادة حُولت إلى المحفظة بالخطأ',
    },
  })
  assert(ahmedTopup.data.payment.id, 'لم تُسجل دفعة محفظة أحمد.')

  const createIndividual = async (payload) => {
    const response = await reception('POST', '/sessions', { idempotent: true, data: payload })
    created.sessions.push(response.data.session.id)
    return response.data
  }
  const serviceItem = (service, extraUnits = 0) => ({
    service_id: service.id, item_type: 'service', extra_units: extraUnits,
  })

  const cashAssessment = await createIndividual(sessionPayload({
    patient: patients.ahmed, doctor: doctors.mohamed,
    services: [serviceItem(services.assessment)], date: clinicDate(),
    settlement: { paid: 40, method: 'cash' },
  }))
  const mixedSession = await createIndividual(sessionPayload({
    patient: patients.ahmed, doctor: doctors.mohamed,
    services: [serviceItem(services.physio), serviceItem(services.balance)], date: daysAgo(1), discount: 15,
    settlement: { wallet: 20, paid: 30, debt: 50, method: 'transfer', sponsorId: sponsors.insurance.id },
  }))
  const walletSession = await createIndividual(sessionPayload({
    patient: patients.fatima, doctor: doctors.sara,
    services: [serviceItem(services.manual)], date: daysAgo(2),
    settlement: { wallet: 75 },
  }))
  const transferSession = await createIndividual(sessionPayload({
    patient: patients.salem, doctor: doctors.ayman,
    services: [serviceItem(services.rehab)], date: daysAgo(4),
    settlement: { paid: 90, method: 'transfer' },
  }))
  const openDebtSession = await createIndividual(sessionPayload({
    patient: patients.yousef, doctor: doctors.mohamed,
    services: [serviceItem(services.physio)], date: daysAgo(7),
    settlement: { debt: 65, sponsorId: sponsors.social.id },
  }))
  const cancelledSession = await createIndividual(sessionPayload({
    patient: patients.layla, doctor: doctors.sara,
    services: [serviceItem(services.physio)], date: clinicDate(),
    settlement: { paid: 65, method: 'cash' },
  }))
  await admin('POST', `/sessions/${cancelledSession.session.id}/cancel`, {
    idempotent: true, data: { reason: 'تم تسجيل الجلسة على ملف مريض غير صحيح' },
  })
  const correctedSession = await createIndividual(sessionPayload({
    patient: patients.maryam, doctor: doctors.sara,
    services: [serviceItem(services.followup)], date: daysAgo(10),
    settlement: { paid: 45, method: 'cash' },
  }))
  await admin('POST', `/sessions/${correctedSession.session.id}/correct`, {
    idempotent: true,
    data: {
      changes: {
        discount: money(5),
        settlement: { wallet_amount: money(0), paid_amount: money(40), debt_amount: money(0), method: 'cash', sponsor_id: null },
      },
      reason: 'إضافة خصم معتمد بعد مراجعة الفاتورة',
    },
  })
  const homeSession = await createIndividual(sessionPayload({
    patient: patients.ibrahim, doctor: doctors.ayman,
    services: [serviceItem(services.home, 5)], date: daysAgo(15),
    settlement: { paid: 125, method: 'cash' },
  }))
  const additionSession = await createIndividual(sessionPayload({
    patient: patients.salem, doctor: doctors.ayman,
    services: [serviceItem(services.assessment)], date: daysAgo(21),
    settlement: { paid: 40, method: 'cash' },
  }))
  await reception('POST', `/sessions/${additionSession.session.id}/services`, {
    idempotent: true,
    data: {
      services: [{ service_id: services.home.id, item_type: 'extra_units', extra_units: 4 }],
      settlement: { wallet_amount: money(0), paid_amount: money(12), debt_amount: money(0), method: 'transfer', sponsor_id: null },
    },
  })
  const khadijaDebtSession = await createIndividual(sessionPayload({
    patient: patients.khadija, doctor: doctors.mohamed,
    services: [serviceItem(services.balance)], date: daysAgo(35),
    settlement: { debt: 50 },
  }))
  const voidDebtSession = await createIndividual(sessionPayload({
    patient: patients.mahmoud, doctor: doctors.mohamed,
    services: [serviceItem(services.physio)], date: daysAgo(18),
    settlement: { debt: 65, sponsorId: sponsors.company.id },
  }))
  await admin('POST', `/sessions/${voidDebtSession.session.id}/cancel`, {
    idempotent: true, data: { reason: 'إلغاء مطالبة سُجلت على الجهة الراعية بالخطأ' },
  })
  const oldPriceSession = await createIndividual(sessionPayload({
    patient: patients.ahmed, doctor: doctors.mohamed,
    services: [serviceItem(services.physio)], date: daysAgo(50),
    settlement: { paid: 65, method: 'cash' },
  }))

  services.physio = (await admin('PATCH', `/services/${services.physio.id}`, {
    data: {
      name: services.physio.name, price: money(70), included_units: null, unit_name: null,
      extra_unit_price: null, is_active: true, reason: 'تحديث قائمة أسعار الجلسات',
    },
  })).data
  doctors.mohamed = (await admin('PATCH', `/doctors/${doctors.mohamed.id}`, {
    data: {
      name: doctors.mohamed.name, phone: doctors.mohamed.phone, commission_rate: money(32),
      is_active: true, reason: 'تعديل النسبة وفق الاتفاق الجديد مع الطبيب',
    },
  })).data
  const newPriceSession = await createIndividual(sessionPayload({
    patient: patients.ahmed, doctor: doctors.mohamed,
    services: [serviceItem(services.physio)], date: clinicDate(),
    settlement: { paid: 70, method: 'transfer' },
  }))

  const oldPriceDetails = await admin('GET', `/sessions/${oldPriceSession.session.id}`)
  const newPriceDetails = await admin('GET', `/sessions/${newPriceSession.session.id}`)
  assert(oldPriceDetails.data.services[0].price === money(65), 'لم تُحفظ لقطة السعر القديمة للجلسة.')
  assert(newPriceDetails.data.services[0].price === money(70), 'لم يُطبق السعر الجديد على الجلسة الجديدة.')
  assert(cashAssessment.session.settlement.coverage === 'cash', 'حالة الدفع النقدي غير صحيحة.')
  assert(mixedSession.session.settlement.coverage === 'mixed', 'حالة الدفع المختلط غير صحيحة.')
  assert(walletSession.session.settlement.coverage === 'wallet', 'حالة الدفع من المحفظة غير صحيحة.')
  assert(transferSession.session.settlement.coverage === 'transfer', 'حالة التحويل المصرفي غير صحيحة.')
  assert(openDebtSession.session.settlement.coverage === 'debt', 'حالة الدين غير صحيحة.')
  assert(homeSession.session.net_total === money(125), 'حساب الوحدات الإضافية غير صحيح.')

  process.stdout.write('4/9 إنشاء الديون المفتوحة والجزئية والمسددّة...\n')
  const openingPartial = await admin('POST', `/patients/${patients.khadija.id}/opening-debts`, {
    idempotent: true,
    data: { amount: money(500), sponsor_id: null, note: 'رصيد افتتاحي من ملف المريضة الورقي' },
  })
  const openingPaid = await admin('POST', `/patients/${patients.salem.id}/opening-debts`, {
    idempotent: true,
    data: { amount: money(150), sponsor_id: null, note: 'مطالبة علاج سابقة قبل تشغيل النظام' },
  })
  created.debts.push(openingPartial.data.id, openingPaid.data.id)
  await reception('POST', '/debts/settlements', {
    idempotent: true,
    data: { amount: money(200), method: 'cash', allocations: [{ debt_id: openingPartial.data.id, amount: money(200) }] },
  })
  await reception('POST', '/debts/settlements', {
    idempotent: true,
    data: { amount: money(150), method: 'transfer', allocations: [{ debt_id: openingPaid.data.id, amount: money(150) }] },
  })
  await reception('POST', '/debts/settlements', {
    idempotent: true,
    data: { amount: money(25), method: 'cash', allocations: [{ debt_id: mixedSession.effects.debt.id, amount: money(25) }] },
  })
  created.debts.push(
    mixedSession.effects.debt.id, openDebtSession.effects.debt.id,
    khadijaDebtSession.effects.debt.id, voidDebtSession.effects.debt.id,
  )

  process.stdout.write('5/9 إنشاء اشتراكات نشطة ومكتملة وملغاة...\n')
  const createSubscription = async (data) => {
    const response = await reception('POST', '/subscriptions', { idempotent: true, data })
    created.subscriptions.push(response.data.subscription.id)
    if (response.data.debt?.id) created.debts.push(response.data.debt.id)
    return response.data
  }
  const createSubscriptionSession = async ({ patient, doctor, subscription, service, date = clinicDate() }) => {
    const response = await reception('POST', '/sessions', {
      idempotent: true,
      data: {
        patient_id: patient.id, doctor_id: doctor.id, type: 'subscription',
        subscription_id: subscription.id, session_date: date,
        services: [serviceItem(service)],
      },
    })
    created.sessions.push(response.data.session.id)
    return response.data
  }

  let activeSubscription = await createSubscription({
    patient_id: patients.fatima.id, name: 'برنامج تأهيل آلام الظهر',
    service_ids: [services.physio.id, services.manual.id], sessions_count: 8,
    total_price: money(520), paid_amount: money(200), method: 'transfer',
  })
  await createSubscriptionSession({ patient: patients.fatima, doctor: doctors.sara, subscription: activeSubscription.subscription, service: services.physio })
  await createSubscriptionSession({ patient: patients.fatima, doctor: doctors.ayman, subscription: activeSubscription.subscription, service: services.manual })
  activeSubscription = (await admin('POST', `/subscriptions/${activeSubscription.subscription.id}/correct`, {
    idempotent: true,
    data: { changes: { sessions_count: 10, total_price: money(600) }, reason: 'تمديد البرنامج بعد تقييم الطبيب' },
  })).data
  const activeDetails = await admin('GET', `/subscriptions/${activeSubscription.subscription.id}`)
  const activeDebt = activeDetails.data.debts.find((debt) => ['open', 'partial'].includes(debt.status))
  assert(activeDebt, 'لم يتم العثور على دين الاشتراك النشط.')
  await reception('POST', '/debts/settlements', {
    idempotent: true,
    data: { amount: money(100), method: 'cash', allocations: [{ debt_id: activeDebt.id, amount: money(100) }] },
  })

  const completedSubscription = await createSubscription({
    patient_id: patients.mahmoud.id, name: 'باقة استعادة الحركة',
    service_ids: [services.physio.id, services.balance.id], sessions_count: 3,
    total_price: money(210), paid_amount: money(210), method: 'cash',
  })
  await createSubscriptionSession({ patient: patients.mahmoud, doctor: doctors.mohamed, subscription: completedSubscription.subscription, service: services.physio })
  await createSubscriptionSession({ patient: patients.mahmoud, doctor: doctors.mohamed, subscription: completedSubscription.subscription, service: services.balance })
  await createSubscriptionSession({ patient: patients.mahmoud, doctor: doctors.sara, subscription: completedSubscription.subscription, service: services.physio })

  const cancelledSubscription = await createSubscription({
    patient_id: patients.maryam.id, name: 'برنامج علاج الرقبة',
    service_ids: [services.followup.id, services.manual.id], sessions_count: 6,
    total_price: money(360), paid_amount: money(180), method: 'cash',
  })
  await createSubscriptionSession({ patient: patients.maryam, doctor: doctors.sara, subscription: cancelledSubscription.subscription, service: services.followup })
  await admin('POST', `/subscriptions/${cancelledSubscription.subscription.id}/cancel`, {
    idempotent: true,
    data: { reason: 'توقف المريضة عن البرنامج لأسباب سفر', refund_amount: money(100), refund_method: 'cash' },
  })

  const nearSubscription = await createSubscription({
    patient_id: patients.ahmed.id, name: 'باقة متابعة قصيرة',
    service_ids: [services.assessment.id, services.followup.id], sessions_count: 3,
    total_price: money(180), paid_amount: money(180), method: 'transfer',
  })
  await createSubscriptionSession({ patient: patients.ahmed, doctor: doctors.mohamed, subscription: nearSubscription.subscription, service: services.assessment })
  await createSubscriptionSession({ patient: patients.ahmed, doctor: doctors.mohamed, subscription: nearSubscription.subscription, service: services.followup })

  process.stdout.write('6/9 تسجيل مصروفات مشتريات وإيجارات...\n')
  const expenseDefinitions = [
    { description: 'مواد تعقيم وقفازات طبية', recipient: 'شركة الشفاء للمستلزمات الطبية', amount: 420, method: 'cash', date: daysAgo(2) },
    { description: 'أغطية أسرّة ومناديل ورقية', recipient: 'شركة النظافة الحديثة', amount: 185, method: 'transfer', date: daysAgo(8) },
    { description: 'كرات وأشرطة تمارين مقاومة', recipient: 'متجر الحركة الرياضية', amount: 310, method: 'cash', date: daysAgo(16) },
    { description: 'صيانة جهاز الموجات فوق الصوتية', recipient: 'المتحدة لصيانة الأجهزة', amount: 275, method: 'transfer', date: daysAgo(29) },
    { description: 'حبر وطباعة ملفات المرضى', recipient: 'مكتبة طرابلس الجديدة', amount: 96, method: 'cash', date: daysAgo(43) },
    { description: 'مياه وضيافة صالة الانتظار', recipient: 'أسواق المدينة', amount: 78, method: 'cash', date: daysAgo(55) },
  ]
  const purchaseExpenses = []
  for (const definition of expenseDefinitions) {
    const payload = {
      type: 'purchase', description: definition.description, recipient: definition.recipient,
      payment_date: definition.date, amount: money(definition.amount), payment_method: definition.method,
      notes: 'مصروف تشغيلي للعيادة',
    }
    const response = await admin('POST', '/expenses', { data: payload })
    purchaseExpenses.push(response.data)
    created.expenses.push(response.data.id)
  }
  const correctedExpense = purchaseExpenses[0]
  const correctedExpenseResponse = await admin('POST', `/expenses/${correctedExpense.id}/correct`, {
    data: {
      type: correctedExpense.type, description: correctedExpense.description, recipient: correctedExpense.recipient,
      payment_date: correctedExpense.payment_date, amount: money(Number(correctedExpense.amount) + 12),
      payment_method: correctedExpense.payment_method, notes: correctedExpense.notes,
      reason: 'إضافة قيمة التوصيل إلى الفاتورة',
    },
  })
  assert(correctedExpenseResponse.data.reference_number === correctedExpense.reference_number, 'تغير رقم مرجع المصروف بعد التصحيح.')

  const rents = []
  for (const offset of [-3, -2, -1, 0]) {
    const month = monthInfo(offset)
    const response = await admin('POST', '/expenses', {
      data: {
        type: 'rent', description: `إيجار شهر ${month.arabicName}`, payment_date: month.paymentDate,
        amount: money(1800), payment_method: offset % 2 === 0 ? 'transfer' : 'cash',
        rent_month: month.month, rent_start_date: month.start, rent_end_date: month.end,
        notes: 'إيجار مقر العيادة الرئيسي',
      },
    })
    rents.push(response.data)
    created.expenses.push(response.data.id)
  }
  const cancelledRentMonth = monthInfo(-1)
  const cancelledRent = await admin('POST', '/expenses', {
    data: {
      type: 'rent', description: `إيجار المخزن الملحق لشهر ${cancelledRentMonth.arabicName}`,
      payment_date: cancelledRentMonth.paymentDate, amount: money(600), payment_method: 'transfer',
      rent_month: cancelledRentMonth.month, rent_start_date: cancelledRentMonth.start,
      rent_end_date: cancelledRentMonth.end, notes: 'إيجار مساحة تخزين ملحقة بالعيادة',
    },
  })
  created.expenses.push(cancelledRent.data.id)
  await admin('POST', `/expenses/${cancelledRent.data.id}/cancel`, {
    data: { reason: 'أُلغي عقد المخزن قبل بدء فترة الإيجار' },
  })

  process.stdout.write('7/9 تسجيل دفعات مستحقات الأطباء وحالات التعطيل...\n')
  for (const [doctor, method] of [[doctors.mohamed, 'cash'], [doctors.sara, 'transfer']]) {
    const dues = await admin('GET', `/doctors/${doctor.id}/dues`)
    const remaining = Number(dues.data.all_time_remaining)
    const amount = Math.min(15, Math.floor((remaining / 2) * 1000) / 1000)
    assert(amount > 0, `لا توجد مستحقات قابلة للصرف للطبيب ${doctor.name}.`)
    const payout = await admin('POST', `/doctors/${doctor.id}/payouts`, {
      idempotent: true,
      data: { amount: money(amount), date: clinicDate(), method, note: 'دفعة جزئية من مستحقات الطبيب' },
    })
    created.payouts.push(payout.data.payout.id)
  }
  await admin('PATCH', `/sponsors/${sponsors.company.id}`, {
    data: { name: sponsors.company.name, phone: sponsors.company.phone, address: sponsors.company.address, is_active: false },
  })

  process.stdout.write('8/9 التحقق من الحالات والتقارير وسجل التدقيق...\n')
  const subscriptionList = await admin('GET', '/subscriptions', { params: { per_page: 100 } })
  const trackedSubscriptions = subscriptionList.data.filter((row) => created.subscriptions.includes(row.id))
  const subscriptionStatuses = new Set(trackedSubscriptions.map((row) => row.status))
  assert(subscriptionStatuses.has('active'), 'لا يوجد اشتراك نشط في بيانات الاختبار.')
  assert(subscriptionStatuses.has('completed'), 'لا يوجد اشتراك مكتمل في بيانات الاختبار.')
  assert(subscriptionStatuses.has('cancelled'), 'لا يوجد اشتراك ملغي في بيانات الاختبار.')

  const allExpenses = await admin('GET', '/expenses', { params: { per_page: 100 } })
  const expenseList = allExpenses.data.filter((row) => created.expenses.includes(row.id))
  assert(expenseList.length === expenseDefinitions.length + rents.length + 1, 'عدد المصروفات المزروعة غير متوقع.')
  const references = expenseList.map((row) => row.reference_number)
  assert(references.every((reference) => /^EXP-\d{8}(?:-\d+)?$/.test(reference)), 'يوجد رقم مرجع مصروف غير مولد آليًا.')
  assert(new Set(references).size === references.length, 'أرقام مراجع المصروفات غير فريدة.')
  assert(expenseList.filter((row) => row.type === 'rent').every((row) => row.recipient === 'صاحب العقار'), 'اسم مستلم الإيجار الداخلي غير صحيح.')
  assert(expenseList.some((row) => row.status === 'cancelled'), 'لا يوجد مصروف ملغي.')

  const debtRows = await admin('GET', '/debts', { params: { per_page: 100 } })
  const trackedDebts = debtRows.data.filter((row) => created.debts.includes(row.id))
  const debtStatuses = new Set(trackedDebts.map((row) => row.status))
  for (const status of ['open', 'partial', 'paid', 'void']) {
    assert(debtStatuses.has(status), `حالة الدين ${status} غير ممثلة في البيانات.`)
  }

  const cancelledDetails = await admin('GET', `/sessions/${cancelledSession.session.id}`)
  assert(cancelledDetails.data.status === 'cancelled', 'الجلسة الملغاة لم تُحفظ بحالتها الصحيحة.')
  const correctedDetails = await admin('GET', `/sessions/${correctedSession.session.id}`)
  assert(correctedDetails.data.net_total === money(40), 'تصحيح الجلسة لم يُطبق القيمة الجديدة.')

  const reportTypes = ['sessions', 'collections', 'debts', 'doctor-dues', 'subscriptions', 'expenses']
  for (const reportType of reportTypes) {
    const params = reportType === 'sessions'
      ? { from: daysAgo(60), to: clinicDate() }
      : {}
    const report = await admin('GET', `/reports/${reportType}`, { params })
    assert(Array.isArray(report.data.rows), `تقرير ${reportType} لا يحتوي صفوفًا.`)
    assert(report.data.totals, `تقرير ${reportType} لا يحتوي إجماليات.`)
  }
  const dashboard = await admin('GET', '/dashboard', { params: { date: clinicDate() } })
  assert(dashboard.data.stats.some((item) => item.key === 'sessions_today'), 'بيانات لوحة التحكم غير مكتملة.')

  const auditChecks = [
    ['session_corrected', 'session', correctedSession.session.id],
    ['session_cancelled', 'session', cancelledSession.session.id],
    ['subscription_cancelled', 'subscription', cancelledSubscription.subscription.id],
    ['expense_corrected', 'expense', correctedExpense.id],
    ['expense_cancelled', 'expense', cancelledRent.data.id],
  ]
  for (const [action, entityType, entityId] of auditChecks) {
    const audit = await admin('GET', '/audit-log', {
      params: { action, entity_type: entityType, entity_id: entityId, per_page: 10 },
    })
    assert(audit.data.length > 0, `سجل التدقيق لا يحتوي الإجراء ${action}.`)
  }

  process.stdout.write('9/9 التحقق من ملفات الطباعة PDF وExcel...\n')
  for (const [reportType, format, signature] of [
    ['expenses', 'pdf', '%PDF'],
    ['subscriptions', 'pdf', '%PDF'],
    ['expenses', 'xlsx', 'PK'],
  ]) {
    const exported = await admin('GET', `/reports/${reportType}/export`, {
      params: { format }, binary: true,
    })
    assert(exported.buffer.length > 500, `ملف ${reportType}.${format} فارغ.`)
    assert(exported.buffer.subarray(0, signature.length).toString() === signature, `توقيع ${reportType}.${format} غير صحيح.`)
  }

  const result = {
    run_id: batchId,
    counts: Object.fromEntries(Object.entries(created).map(([key, values]) => [key, values.length])),
    represented_states: {
      session_coverages: ['cash', 'transfer', 'wallet', 'debt', 'mixed'],
      session_statuses: ['closed', 'cancelled'],
      subscription_statuses: [...subscriptionStatuses].sort(),
      debt_statuses: [...debtStatuses].sort(),
      expense_types: ['purchase', 'rent'],
      expense_statuses: ['active', 'cancelled'],
    },
    key_records: {
      active_subscription_id: activeSubscription.subscription.id,
      completed_subscription_id: completedSubscription.subscription.id,
      cancelled_subscription_id: cancelledSubscription.subscription.id,
      corrected_session_id: correctedSession.session.id,
      cancelled_session_id: cancelledSession.session.id,
      cancelled_expense_id: cancelledRent.data.id,
    },
  }
  process.stdout.write('اكتملت تعبئة البيانات الواقعية والتحقق منها بنجاح.\n')
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
  return result
}

async function main() {
  if (process.argv.includes('--help')) {
    process.stdout.write('يشغّل بيانات اختبار واقعية على نسخة التطبيق المحلية الفعلية.\n')
    process.stdout.write('استخدم --reset لمسح كل البيانات التشغيلية قبل التعبئة مع إبقاء المستخدمين والإعدادات والتفعيل.\n')
    process.stdout.write('متغيرات اختيارية: API_URL, ADMIN_TOKEN, RECEPTION_TOKEN, SEED_BATCH, CLINIC_RUNTIME_DIR.\n')
    return
  }

  const shouldReset = process.argv.includes('--reset')
  const generatedBatch = `RUN-${clinicDate().replaceAll('-', '')}-${Date.now().toString().slice(-6)}`
  const batchId = (process.env.SEED_BATCH || generatedBatch)
    .replaceAll(/[^A-Za-z0-9_-]/g, '-')
    .slice(0, 40)
  let localRuntime = null
  let adminToken = process.env.ADMIN_TOKEN || null
  let receptionistToken = process.env.RECEPTION_TOKEN || null
  const localTokenIds = []

  try {
    if (shouldReset || !process.env.API_URL || !adminToken || !receptionistToken) {
      localRuntime = await localRuntimeInfo()
    }
    const baseUrl = process.env.API_URL || localRuntime.baseUrl
    if (shouldReset) {
      process.stdout.write('مسح جميع البيانات التشغيلية القديمة...\n')
      await resetOperationalData(localRuntime)
      process.stdout.write('تم المسح مع الإبقاء على المستخدمين وإعدادات العيادة.\n')
    }
    if (!adminToken) {
      const token = await issueLocalToken(localRuntime, 'admin', batchId)
      assert(token, 'لا يوجد حساب مدير نشط في النسخة المحلية.')
      adminToken = token.bearer
      localTokenIds.push(token.id)
    }
    if (!receptionistToken) {
      const token = await issueLocalToken(localRuntime, 'receptionist', batchId)
      if (token) {
        receptionistToken = token.bearer
        localTokenIds.push(token.id)
      } else {
        receptionistToken = adminToken
      }
    }
    await seed({ baseUrl, adminToken, receptionistToken, batchId })
  } finally {
    if (localRuntime && localTokenIds.length) {
      await revokeLocalTokens(localRuntime, localTokenIds, batchId)
    }
  }
}

main().catch((error) => {
  process.stderr.write(`فشلت تعبئة البيانات: ${error.stack || error.message}\n`)
  process.exitCode = 1
})
