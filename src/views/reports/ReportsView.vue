<template>
  <div class="page-container page-stack">
    <PageHeader :title="currentReport.title" :subtitle="currentReport.description">
      <template #actions>
        <v-btn variant="outlined" prepend-icon="mdi-file-pdf-box" :loading="downloading === 'pdf'" :disabled="!report?.rows?.length || Boolean(downloading)" @click="downloadExport('pdf')">تنزيل PDF</v-btn>
        <v-btn variant="outlined" prepend-icon="mdi-microsoft-excel" :loading="downloading === 'xlsx'" :disabled="!report?.rows?.length || Boolean(downloading)" @click="downloadExport('xlsx')">تنزيل Excel</v-btn>
        <v-btn :to="{ path: `/print/report/${type}`, query: reportParams() }" color="primary" prepend-icon="mdi-printer-outline">طباعة</v-btn>
      </template>
    </PageHeader>

    <nav class="report-nav" aria-label="أنواع التقارير">
      <router-link v-for="item in reportTypes" :key="item.value" :to="`/reports/${item.value}`" :class="{ active: type === item.value }"><v-icon :icon="item.icon" />{{ item.title }}</router-link>
    </nav>

    <section class="surface-card report-filters">
      <v-text-field v-model="filters.from" label="من تاريخ" type="date" />
      <v-text-field v-model="filters.to" label="إلى تاريخ" type="date" />
      <v-select v-if="['sessions', 'doctor-dues'].includes(type)" v-model="filters.doctor_id" label="الطبيب" :items="doctors" item-title="name" item-value="id" clearable :loading="loadingFilters" />
      <v-autocomplete v-if="['sessions', 'collections', 'debts', 'subscriptions'].includes(type)" v-model="filters.patient_id" label="المريض" :items="patients" item-title="name" item-value="id" clearable :loading="loadingFilters" />
      <v-select v-if="['collections', 'debts'].includes(type)" v-model="filters.sponsor_id" label="الجهة الراعية" :items="sponsors" item-title="name" item-value="id" clearable :loading="loadingFilters" />
      <v-select v-if="type === 'sessions'" v-model="filters.status" label="حالة الجلسة" :items="sessionStatuses" clearable />
      <v-select v-if="type === 'collections'" v-model="filters.transaction_type" label="نوع المعاملة" :items="paymentTypes" clearable />
      <v-select v-if="type === 'collections'" v-model="filters.method" label="طريقة الدفع" :items="paymentMethods" clearable />
      <v-select v-if="type === 'debts'" v-model="filters.status" label="حالة الدين" :items="debtStatuses" clearable />
      <v-select v-if="type === 'debts'" v-model="filters.kind" label="نوع الدين" :items="debtKinds" clearable />
      <v-select v-if="type === 'subscriptions'" v-model="filters.status" label="الحالة" :items="subscriptionStatuses" clearable />
      <v-checkbox v-if="type === 'subscriptions'" v-model="filters.near_end" label="قريبة من الانتهاء" :true-value="1" :false-value="null" hide-details />
      <v-btn color="primary" prepend-icon="mdi-filter-outline" @click="load">تطبيق الفلاتر</v-btn>
      <v-btn variant="text" @click="clearFilters">مسح</v-btn>
    </section>
    <InlineError v-if="filterError" :message="filterError" retry @retry="loadFilters" />

    <v-skeleton-loader v-if="loading" type="image, table" />
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="report">
      <section class="report-totals">
        <div v-for="(value, key) in report.totals" :key="key"><span>{{ totalLabel(key) }}</span><MoneyText v-if="isMoneyTotal(key)" :value="value" /><strong v-else class="ltr-value">{{ value }}</strong></div>
      </section>
      <section class="report-table">
        <div v-if="report.rows.length" class="table-scroll"><v-table><thead><tr><th v-for="header in currentReport.headers" :key="header">{{ header }}</th></tr></thead><tbody>
          <template v-if="type === 'sessions'"><tr v-for="row in report.rows" :key="row.id"><td>{{ formatDate(row.opened_at) }}</td><td>{{ row.patient.name }}</td><td>{{ row.doctor.name }}</td><td>{{ row.services.map((item) => item.name).join('، ') }}</td><td><MoneyText :value="row.net_total" /></td><td><StatusChip type="session" :value="row.status" /></td></tr></template>
          <template v-else-if="type === 'collections'"><tr v-for="row in report.rows" :key="row.id"><td>{{ formatDate(row.created_at) }}</td><td>{{ paymentType(row.type) }}</td><td>{{ row.patient?.name || '—' }}</td><td>{{ labelFor('paymentMethod', row.method) }}</td><td><MoneyText :value="row.amount" /></td></tr></template>
          <template v-else-if="type === 'debts'"><tr v-for="row in report.rows" :key="row.id"><td>#{{ row.id }}</td><td>{{ row.patient.name }}</td><td>{{ row.sponsor?.name || 'المريض' }}</td><td><MoneyText :value="row.amount" /></td><td><MoneyText :value="row.paid_amount" /></td><td><MoneyText :value="row.remaining" /></td><td><StatusChip type="debt" :value="row.status" /></td></tr></template>
          <template v-else-if="type === 'doctor-dues'"><tr v-for="row in report.rows" :key="row.id"><td>{{ row.name }}</td><td>{{ row.sessions_count }}</td><td><MoneyText :value="row.earned" /></td><td><MoneyText :value="row.paid" /></td><td><MoneyText :value="row.remaining" /></td></tr></template>
          <template v-else><tr v-for="row in report.rows" :key="row.id"><td>{{ row.patient.name }}</td><td>{{ row.plan_type === 'monthly' ? 'شهري' : 'باقة قديمة' }}</td><td>{{ formatDate(row.start_date) }}</td><td>{{ formatDate(row.end_date || row.expected_end_date) }}</td><td>{{ row.plan_type === 'monthly' ? `${row.remaining_days} يوم` : `${row.remaining} جلسة` }}</td><td><MoneyText :value="row.net_total" /></td><td><StatusChip type="subscription" :value="row.status" /></td></tr></template>
        </tbody></v-table></div>
        <EmptyState v-else icon="mdi-chart-box-outline" title="لا توجد نتائج للتقرير" description="غيّر الفترة أو الفلاتر ثم أعد المحاولة." />
      </section>
      <p class="report-note">أُعد التقرير في {{ generatedAt }}. لا يوجد تقرير أرباح؛ الأرقام تفصل قيمة الخدمات والمقبوضات والديون ومستحقات الأطباء.</p>
    </template>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getDoctorOptions } from '@/api/doctors'
import { listPatients } from '@/api/patients'
import { exportReport, getReport } from '@/api/reports'
import { getSponsorOptions } from '@/api/sponsors'
import EmptyState from '@/components/common/EmptyState.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import { labelFor, paymentMethods } from '@/constants/labels'
import { useUiStore } from '@/stores/ui'

const route = useRoute(); const router = useRouter(); const ui = useUiStore()
const reportTypes = [
  { value: 'sessions', title: 'تقرير الجلسات', description: 'الجلسات والخدمات والأطباء وقيم الخدمات خلال الفترة.', icon: 'mdi-clipboard-pulse-outline', headers: ['التاريخ', 'المريض', 'الطبيب', 'الخدمات', 'القيمة', 'الحالة'] },
  { value: 'collections', title: 'تقرير المقبوضات', description: 'الدفعات والاسترجاعات وصافي حركة المقبوضات.', icon: 'mdi-cash-multiple', headers: ['التاريخ', 'النوع', 'المريض', 'الطريقة', 'المبلغ'] },
  { value: 'debts', title: 'تقرير الديون', description: 'المبالغ الأصلية والمسدد والمتبقي حسب المريض والجهة.', icon: 'mdi-file-clock-outline', headers: ['المرجع', 'المريض', 'المسؤول', 'الأصل', 'المسدد', 'المتبقي', 'الحالة'] },
  { value: 'doctor-dues', title: 'تقرير مستحقات الأطباء', description: 'المستحقات المكتسبة والمصروفة والمتبقية لكل طبيب.', icon: 'mdi-doctor', headers: ['الطبيب', 'الجلسات', 'المكتسب', 'المصروف', 'المتبقي'] },
  { value: 'subscriptions', title: 'تقرير الاشتراكات', description: 'الاشتراكات الشهرية ومددها وقيمها وحالاتها.', icon: 'mdi-wallet-membership', headers: ['المريض', 'النوع', 'البداية', 'النهاية', 'المتبقي', 'القيمة', 'الحالة'] },
]
const type = computed(() => reportTypes.some((item) => item.value === route.params.type) ? route.params.type : 'sessions')
const currentReport = computed(() => reportTypes.find((item) => item.value === type.value))
const report = ref(null); const loading = ref(true); const error = ref(''); const doctors = ref([]); const patients = ref([]); const sponsors = ref([]); const downloading = ref('')
const loadingFilters = ref(false)
const filterError = ref('')
let loadVersion = 0
onBeforeUnmount(() => { loadVersion += 1 })
const filters = reactive({
  from: route.query.from || '',
  to: route.query.to || '',
  doctor_id: route.query.doctor_id ? Number(route.query.doctor_id) : null,
  patient_id: route.query.patient_id ? Number(route.query.patient_id) : null,
  sponsor_id: route.query.sponsor_id ? Number(route.query.sponsor_id) : null,
  transaction_type: route.query.type || null,
  method: route.query.method || null,
  status: route.query.status || null,
  kind: route.query.kind || null,
  near_end: route.query.near_end === '1' ? 1 : null,
})
const sessionStatuses = [{ title: 'مفتوحة', value: 'open' }, { title: 'مغلقة', value: 'closed' }, { title: 'ملغاة', value: 'cancelled' }]
const subscriptionStatuses = [{ title: 'نشط', value: 'active' }, { title: 'مكتمل', value: 'completed' }, { title: 'موقوف', value: 'stopped' }, { title: 'ملغي', value: 'cancelled' }]
const debtStatuses = [{ title: 'مفتوح', value: 'open' }, { title: 'جزئي', value: 'partial' }, { title: 'مسدد', value: 'paid' }, { title: 'ملغى', value: 'void' }]
const debtKinds = [{ title: 'جلسة', value: 'session' }, { title: 'اشتراك', value: 'subscription' }, { title: 'افتتاحي', value: 'opening' }]
const paymentTypes = [{ title: 'دفعة جلسة', value: 'session_payment' }, { title: 'دفعة اشتراك', value: 'subscription_payment' }, { title: 'شحن رصيد', value: 'wallet_topup' }, { title: 'تسديد دين', value: 'debt_payment' }, { title: 'استرجاع', value: 'refund' }]
const generatedAt = computed(() => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'long', timeStyle: 'short' }).format(new Date()))
onMounted(() => {
  if (!route.params.type) { router.replace('/reports/sessions'); return }
  load()
  loadFilters()
})
async function loadFilters() {
  loadingFilters.value = true
  filterError.value = ''
  try {
    const [doctorOptions, patientResponse, sponsorOptions] = await Promise.all([
      ['sessions', 'doctor-dues'].includes(type.value) ? getDoctorOptions() : [],
      ['sessions', 'collections', 'debts', 'subscriptions'].includes(type.value) ? listPatients({ per_page: 100 }) : { data: [] },
      ['collections', 'debts'].includes(type.value) ? getSponsorOptions() : [],
    ])
    doctors.value = doctorOptions
    patients.value = patientResponse.data
    sponsors.value = sponsorOptions
  } catch (cause) {
    filterError.value = cause.message
  } finally {
    loadingFilters.value = false
  }
}
async function load() {
  const version = ++loadVersion
  const reportType = type.value
  loading.value = true
  error.value = ''
  try {
    const params = reportParams()
    const response = await getReport(reportType, params)
    if (version !== loadVersion) return
    report.value = response.data
    await router.replace({ path: `/reports/${reportType}`, query: params })
  } catch (apiError) {
    if (version === loadVersion) error.value = apiError.message
  } finally {
    if (version === loadVersion) loading.value = false
  }
}
function clearFilters() {
  Object.assign(filters, { from: '', to: '', doctor_id: null, patient_id: null, sponsor_id: null, transaction_type: null, method: null, status: null, kind: null, near_end: null })
  load()
}
function totalLabel(key) { return ({ count: 'عدد السجلات', service_value: 'قيمة الخدمات', received: 'إجمالي المقبوض', refunds: 'الاسترجاعات', net: 'صافي المقبوضات', original: 'إجمالي الديون', paid: 'المسدد / المصروف', remaining: 'المتبقي', earned: 'المستحق المكتسب', active: 'الاشتراكات النشطة', remaining_sessions: 'الجلسات المتبقية', net_total: 'قيمة الاشتراكات' }[key] || key) }
function isMoneyTotal(key) { return ['service_value', 'received', 'refunds', 'net', 'original', 'paid', 'remaining', 'earned', 'net_total'].includes(key) }
const formatDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(value)); const paymentType = (value) => ({ wallet_topup: 'شحن رصيد', session_payment: 'دفعة جلسة', subscription_payment: 'دفعة اشتراك', debt_payment: 'تسديد دين', refund: 'استرجاع' }[value] || value)
function reportParams() {
  return Object.fromEntries(Object.entries({ ...filters, type: filters.transaction_type }).filter(([key, value]) => key !== 'transaction_type' && value !== null && value !== ''))
}
async function downloadExport(format) {
  downloading.value = format
  try {
    const file = await exportReport(type.value, format, reportParams())
    const url = URL.createObjectURL(file.blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = file.filename
    document.body.appendChild(anchor)
    anchor.click()
    anchor.remove()
    URL.revokeObjectURL(url)
    ui.showSnackbar(format === 'pdf' ? 'تم تنزيل تقرير PDF.' : 'تم تنزيل تقرير Excel.')
  } catch (apiError) {
    ui.showSnackbar(apiError.message, 'error')
  } finally {
    downloading.value = ''
  }
}
</script>

<style scoped>
.report-nav { display: flex; flex-wrap: wrap; gap: 8px; }.report-nav a { min-height: 44px; display: flex; align-items: center; gap: 8px; padding: 10px 16px; color: var(--color-ink-muted); border: 1px solid var(--color-hairline); border-radius: var(--radius-pill); }.report-nav a.active { color: var(--color-primary); border-color: var(--color-primary); background: var(--color-info-bg); }.report-filters { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 16px; align-items: end; }.report-totals { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-card); }.report-totals div { display: grid; gap: 5px; padding: 20px; border-inline-start: 1px solid var(--color-hairline); }.report-totals span { color: var(--color-ink-muted); font-size: 13px; }.report-totals strong, .report-totals :deep(.money-text) { font-size: 24px; font-weight: 600; }.report-table { overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-card); }.table-scroll { overflow-x: auto; }.table-scroll :deep(table) { min-width: 860px; }.report-note { margin: 0; color: var(--color-ink-muted); font-size: 13px; }
</style>
