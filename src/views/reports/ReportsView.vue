<template>
  <div class="page-container page-stack">
    <PageHeader :title="currentReport.title" :subtitle="currentReport.description">
      <template #actions>
        <v-btn :to="{ path: `/print/report/${type}`, query: reportParams() }" color="primary" prepend-icon="mdi-printer-outline">طباعة</v-btn>
        <v-btn variant="outlined" prepend-icon="mdi-file-pdf-box" :loading="downloading === 'pdf'" :disabled="!report?.rows?.length || Boolean(downloading)" @click="downloadExport('pdf')">تنزيل PDF</v-btn>
        <v-btn variant="outlined" prepend-icon="mdi-microsoft-excel" :loading="downloading === 'xlsx'" :disabled="!report?.rows?.length || Boolean(downloading)" @click="downloadExport('xlsx')">تنزيل Excel</v-btn>
      </template>
    </PageHeader>

    <nav class="report-nav" aria-label="أنواع التقارير">
      <router-link v-for="item in reportTypes" :key="item.value" :to="`/reports/${item.value}`" :class="{ active: type === item.value }"><v-icon :icon="item.icon" />{{ item.title }}</router-link>
    </nav>

    <section class="surface-card report-filters" data-form-flow-filter>
      <v-text-field v-model="filters.from" label="من تاريخ" type="date" />
      <v-text-field v-model="filters.to" label="إلى تاريخ" type="date" />
      <v-select v-if="['sessions', 'doctor-dues', 'subscriptions'].includes(type)" v-model="filters.doctor_id" label="الطبيب" :items="doctors" item-title="name" item-value="id" clearable :loading="loadingFilters" />
      <v-autocomplete v-if="['sessions', 'collections', 'debts', 'subscriptions'].includes(type)" v-model="filters.patient_id" label="المريض" :items="patients" item-title="name" item-value="id" clearable :loading="loadingFilters" />
      <v-select v-if="['collections', 'debts'].includes(type)" v-model="filters.sponsor_id" label="الجهة الراعية" :items="sponsors" item-title="name" item-value="id" clearable :loading="loadingFilters" />
      <v-select v-if="type === 'sessions'" v-model="filters.status" label="حالة الجلسة" :items="sessionStatuses" clearable />
      <v-select v-if="type === 'collections'" v-model="filters.transaction_type" label="نوع المعاملة" :items="paymentTypes" clearable />
      <v-select v-if="type === 'expenses'" v-model="filters.transaction_type" label="نوع المصروف" :items="expenseTypes" clearable />
      <v-select v-if="['collections', 'expenses'].includes(type)" v-model="filters.method" label="طريقة الدفع" :items="paymentMethods" clearable />
      <v-select v-if="type === 'debts'" v-model="filters.status" label="حالة الدين" :items="debtStatuses" clearable />
      <v-select v-if="type === 'debts'" v-model="filters.kind" label="نوع الدين" :items="debtKinds" clearable />
      <v-select v-if="type === 'subscriptions'" v-model="filters.status" label="الحالة" :items="subscriptionStatuses" clearable />
      <v-select v-if="type === 'expenses'" v-model="filters.status" label="الحالة" :items="expenseStatuses" clearable />
      <v-btn color="primary" prepend-icon="mdi-filter-outline" data-form-flow-filter-action @click="load">تطبيق الفلاتر</v-btn>
      <v-btn variant="text" @click="clearFilters">مسح</v-btn>
    </section>
    <InlineError v-if="filterError" :message="filterError" retry @retry="loadFilters" />

    <v-skeleton-loader v-if="loading" type="image, table" />
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="report">
      <section class="report-totals">
        <div v-for="item in summaryItems" :key="item.key"><span>{{ item.label }}</span><MoneyText v-if="item.money" :value="item.value" /><strong v-else class="ltr-value">{{ item.value }}</strong></div>
      </section>
      <section class="report-table">
        <ReportTable v-if="report.rows.length" :type="type" :rows="report.rows" />
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
import ReportTable from '@/components/reports/ReportTable.vue'
import { expenseTypes, paymentMethods } from '@/constants/labels'
import { formatReportDateTime, getReportDefinition, reportSummaryItems, reportTypes } from '@/constants/reports'
import { useUiStore } from '@/stores/ui'

const route = useRoute(); const router = useRouter(); const ui = useUiStore()
const type = computed(() => reportTypes.some((item) => item.value === route.params.type) ? route.params.type : 'sessions')
const currentReport = computed(() => getReportDefinition(type.value))
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
})
const sessionStatuses = [{ title: 'مفتوحة', value: 'open' }, { title: 'مغلقة', value: 'closed' }, { title: 'ملغاة', value: 'cancelled' }]
const subscriptionStatuses = [{ title: 'نشط', value: 'active' }, { title: 'مكتمل', value: 'completed' }, { title: 'ملغي', value: 'cancelled' }]
const debtStatuses = [{ title: 'مفتوح', value: 'open' }, { title: 'جزئي', value: 'partial' }, { title: 'مسدد', value: 'paid' }, { title: 'ملغى', value: 'void' }]
const debtKinds = [{ title: 'جلسة', value: 'session' }, { title: 'اشتراك', value: 'subscription' }, { title: 'افتتاحي', value: 'opening' }]
const expenseStatuses = [{ title: 'نشط', value: 'active' }, { title: 'ملغي', value: 'cancelled' }]
const paymentTypes = [{ title: 'دفعة جلسة', value: 'session_payment' }, { title: 'دفعة اشتراك', value: 'subscription_payment' }, { title: 'شحن رصيد', value: 'wallet_topup' }, { title: 'تسديد دين', value: 'debt_payment' }, { title: 'استرجاع', value: 'refund' }]
const generatedAt = computed(() => formatReportDateTime(report.value?.meta?.generated_at))
const summaryItems = computed(() => reportSummaryItems(type.value, report.value?.totals))
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
      ['sessions', 'doctor-dues', 'subscriptions'].includes(type.value) ? getDoctorOptions() : [],
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
  Object.assign(filters, { from: '', to: '', doctor_id: null, patient_id: null, sponsor_id: null, transaction_type: null, method: null, status: null, kind: null })
  load()
}
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
.report-nav { display: flex; flex-wrap: wrap; gap: 4px; padding: 4px; border: 1px solid var(--color-border); border-radius: var(--radius-lg); background: var(--color-surface); }.report-nav a { min-height: 42px; display: flex; align-items: center; gap: 8px; padding: 9px 14px; color: var(--color-ink-muted); border: 1px solid transparent; border-radius: var(--radius-sm); }.report-nav a:hover { color: var(--color-foreground); background: var(--color-surface-subtle); }.report-nav a.active { color: var(--color-primary); border-color: var(--color-primary-border); background: var(--color-primary-subtle); box-shadow: inset 0 -2px 0 var(--color-primary); }.report-filters { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 16px; align-items: end; }.report-totals { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-card); background: var(--color-surface); }.report-totals div { display: grid; gap: 5px; padding: 20px; border-inline-start: 1px solid var(--color-hairline); }.report-totals span { color: var(--color-ink-muted); font-size: 13px; }.report-totals strong, .report-totals :deep(.money-text) { font-size: 24px; font-weight: 600; }.report-table { overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-card); }.report-note { margin: 0; color: var(--color-ink-muted); font-size: 13px; }
</style>
