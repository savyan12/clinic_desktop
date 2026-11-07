<template>
  <div class="report-print-shell" dir="rtl">
    <v-skeleton-loader v-if="loading" class="print-loading" type="article, table" />
    <InlineError v-else-if="error" :message="error" />
    <article
      v-else-if="report"
      class="report-document"
      :class="{ 'subscription-document': isSubscriptions }"
    >
      <template v-if="isSubscriptions">
        <header class="subscription-header">
          <div class="subscription-brand">
            <ClinicLogo v-if="settings.show_logo" :size="58" />
            <div>
              <h1>{{ settings.clinic_name }}</h1>
              <p>{{ settings.clinic_subtitle }}</p>
            </div>
          </div>
          <div class="subscription-title">
            <h2>تقرير الاشتراكات</h2>
            <span>تاريخ الإصدار: <bdi dir="ltr">{{ issuedOn }}</bdi></span>
          </div>
        </header>

        <div class="subscription-brand-rule"></div>
        <p class="subscription-period">
          فترة التقرير: <bdi dir="ltr">{{ subscriptionPeriod }}</bdi>
        </p>

        <section class="subscription-summary" aria-label="ملخص تقرير الاشتراكات">
          <div>
            <span>عدد الاشتراكات</span>
            <strong class="ltr-value">{{ report.totals.count ?? report.rows.length }}</strong>
          </div>
          <div>
            <span>الجلسات المكتملة خلال الفترة</span>
            <strong class="ltr-value">{{ report.totals.completed_sessions ?? 0 }}</strong>
          </div>
          <div>
            <span>الجلسات المتبقية</span>
            <strong class="ltr-value">{{ report.totals.remaining_sessions ?? 0 }}</strong>
          </div>
          <div>
            <span>إجمالي قيمة الاشتراكات</span>
            <MoneyText :value="report.totals.total_value ?? 0" />
          </div>
        </section>

        <section class="subscription-section">
          <h3>كشف الاشتراكات</h3>
          <div class="subscription-table-frame">
            <table class="subscription-table subscription-main-table">
              <colgroup>
                <col class="subscription-patient-column">
                <col class="subscription-name-column">
                <col class="subscription-count-column">
                <col class="subscription-remaining-column">
                <col class="subscription-price-column">
                <col class="subscription-status-column">
              </colgroup>
              <thead>
                <tr>
                  <th>المريض</th>
                  <th>الاشتراك</th>
                  <th>الجلسات</th>
                  <th>المتبقي</th>
                  <th>السعر</th>
                  <th>الحالة</th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="!report.rows.length">
                  <td colspan="6" class="subscription-empty">لا توجد بيانات ضمن الفترة المحددة</td>
                </tr>
                <tr v-for="(row, index) in report.rows" v-else :key="row.id ?? index">
                  <td>{{ row.patient?.name || '—' }}</td>
                  <td>{{ row.name || '—' }}</td>
                  <td class="numeric-cell">{{ row.sessions_count ?? 0 }}</td>
                  <td class="numeric-cell">{{ row.remaining ?? 0 }}</td>
                  <td><MoneyText :value="row.total_price" /></td>
                  <td>{{ subscriptionStatus(row.status) }}</td>
                </tr>
              </tbody>
              <tfoot>
                <tr>
                  <th>الإجمالي</th>
                  <th>{{ report.totals.count ?? report.rows.length }} اشتراكات</th>
                  <th class="numeric-cell">{{ report.totals.total_sessions ?? 0 }}</th>
                  <th class="numeric-cell">{{ report.totals.remaining_sessions ?? 0 }}</th>
                  <th><MoneyText :value="report.totals.total_value ?? 0" /></th>
                  <th></th>
                </tr>
              </tfoot>
            </table>
          </div>
          <p class="subscription-note">
            الجلسات: سعة الاشتراك الكلية، المتبقي: الرصيد بعد خصم الجلسات المكتملة حتى نهاية الفترة.
          </p>
        </section>

        <section class="subscription-section subscription-supervision-section">
          <h3>كشف إشراف الأطباء على جلسات الاشتراكات</h3>
          <div class="subscription-table-frame">
            <table class="subscription-table supervision-table">
              <colgroup>
                <col class="supervision-doctor-column">
                <col class="supervision-patient-column">
                <col class="supervision-codes-column">
                <col class="supervision-count-column">
              </colgroup>
              <thead>
                <tr>
                  <th>الطبيب</th>
                  <th>المريض</th>
                  <th>أرقام الجلسات</th>
                  <th>عدد الجلسات</th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="!supervisionRows.length">
                  <td colspan="4" class="subscription-empty">لا توجد جلسات مكتملة ضمن الفترة المحددة</td>
                </tr>
                <tr v-for="row in supervisionRows" v-else :key="row.key">
                  <td>{{ row.doctor }}</td>
                  <td>{{ row.patient }}</td>
                  <td><bdi dir="ltr">{{ row.sessionCodes }}</bdi></td>
                  <td class="numeric-cell">{{ row.count }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>

        <footer class="subscription-footer">
          <span>{{ subscriptionFooterText }}</span>
          <span class="subscription-page-number">صفحة 1 من 1</span>
        </footer>
      </template>

      <template v-else>
        <header class="report-header">
          <div class="report-brand">
            <ClinicLogo v-if="settings.show_logo" :size="58" />
            <div>
              <h1>{{ settings.clinic_name }}</h1>
              <p>{{ settings.clinic_subtitle }}</p>
            </div>
          </div>
          <div class="report-title">
            <h2>{{ definition.title }}</h2>
            <span>أُعد في <bdi dir="auto">{{ generatedAt }}</bdi></span>
          </div>
        </header>

        <div class="brand-rule"></div>

        <section class="report-meta" aria-label="بيانات التقرير">
          <div>
            <span>الفترة</span>
            <strong>{{ periodLabel }}</strong>
          </div>
          <div>
            <span>نوع التقرير</span>
            <strong>{{ definition.title }}</strong>
          </div>
          <div>
            <span>عدد النتائج</span>
            <strong class="ltr-value">{{ report.rows.length }}</strong>
          </div>
        </section>

        <section class="report-summary" aria-label="ملخص التقرير">
          <div v-for="item in summaryItems" :key="item.key">
            <span>{{ item.label }}</span>
            <MoneyText v-if="item.money" :value="item.value" />
            <strong v-else class="ltr-value">{{ item.value }}</strong>
          </div>
        </section>

        <section class="report-section">
          <h3>بيانات التقرير</h3>
          <div class="section-rule"></div>
          <ReportTable :type="type" :rows="report.rows" print />
        </section>

        <p class="report-note">هذا التقرير يعرض كل مفهوم مالي بصورة مستقلة، ولا يُعد تقرير أرباح.</p>

        <footer class="paper-footer">
          <div class="footer-contact">
            <span>{{ settings.address || '—' }}</span>
            <strong class="phone-value">{{ settings.phone || '—' }}</strong>
          </div>
          <p>{{ settings.receipt_footer }}</p>
        </footer>
      </template>
    </article>
  </div>
</template>

<script setup>
import { computed, inject, onMounted, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'
import { getReport } from '@/api/reports'
import ClinicLogo from '@/components/common/ClinicLogo.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import ReportTable from '@/components/reports/ReportTable.vue'
import { labelFor } from '@/constants/labels'
import {
  formatReportDateTime,
  getReportDefinition,
  reportPeriodLabel,
  reportSummaryItems,
} from '@/constants/reports'

const route = useRoute()
const report = ref(null)
const loading = ref(true)
const error = ref('')
const settings = inject('printSettings', reactive({
  clinic_name: 'عيادة عين الحياة',
  clinic_subtitle: 'للعلاج الطبيعي والتأهيل',
  phone: '',
  address: '',
  receipt_footer: '',
  show_logo: true,
}))

const definition = computed(() => getReportDefinition(route.params.type))
const type = computed(() => definition.value.value)
const isSubscriptions = computed(() => type.value === 'subscriptions')
const generatedAt = computed(() => formatReportDateTime(report.value?.meta?.generated_at))
const periodLabel = computed(() => reportPeriodLabel(route.query.from, route.query.to))
const summaryItems = computed(() => reportSummaryItems(type.value, report.value?.totals))
const numericDateFormatter = new Intl.DateTimeFormat('en-GB', {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
  timeZone: 'Africa/Tripoli',
})
const formatNumericDate = (value) => {
  if (!value) return ''
  const source = /^\d{4}-\d{2}-\d{2}$/.test(String(value)) ? `${value}T00:00:00+02:00` : value
  const date = new Date(source)
  return Number.isNaN(date.getTime()) ? String(value) : numericDateFormatter.format(date)
}
const issuedOn = computed(() => formatNumericDate(report.value?.meta?.generated_at || new Date()))
const subscriptionPeriod = computed(() => {
  const from = report.value?.meta?.from || route.query.from
  const to = report.value?.meta?.to || route.query.to
  if (from && to) return `${formatNumericDate(from)} - ${formatNumericDate(to)}`
  if (from) return `من ${formatNumericDate(from)}`
  if (to) return `حتى ${formatNumericDate(to)}`
  return 'كل الفترات'
})
const subscriptionStatus = (value) => labelFor('subscriptionStatus', value)
const supervisionRows = computed(() => {
  const groups = new Map()
  for (const subscription of report.value?.rows || []) {
    const patient = subscription.patient?.name || '—'
    const patientKey = subscription.patient?.id ?? patient
    for (const session of subscription.supervised_sessions || []) {
      if (session.status !== 'closed') continue
      const doctor = session.doctor?.name || '—'
      const doctorKey = session.doctor?.id ?? doctor
      const key = `${doctorKey}:${patientKey}`
      if (!groups.has(key)) groups.set(key, { key, doctor, patient, codes: [] })
      const code = String(session.code || session.id || '—').replace(/^S-/i, '')
      groups.get(key).codes.push(code)
    }
  }
  return [...groups.values()].map((group) => ({
    ...group,
    sessionCodes: group.codes.join('، '),
    count: group.codes.length,
  }))
})
const subscriptionFooterText = computed(() => {
  const identity = [settings.clinic_name, settings.clinic_subtitle].filter(Boolean).join(' ')
  return `${identity} · ${settings.receipt_footer || 'تقرير إداري للمراجعة'}`
})

onMounted(async () => {
  try {
    report.value = (await getReport(type.value, route.query)).data
  } catch (apiError) {
    error.value = apiError.message
  } finally {
    loading.value = false
  }
})
</script>

<style scoped>
.report-print-shell {
  display: grid;
  place-items: start center;
}

.print-loading,
.report-document {
  width: min(100%, 210mm);
}

.subscription-document {
  display: block;
  padding: 12mm 11mm 9mm;
  font-size: 10px;
  -webkit-print-color-adjust: exact;
  print-color-adjust: exact;
}

.subscription-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
}

.subscription-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.subscription-brand h1,
.subscription-brand p,
.subscription-title h2,
.subscription-period,
.subscription-section h3,
.subscription-note {
  margin: 0;
}

.subscription-brand h1 {
  color: var(--color-print-brand-dark);
  font-size: 20px;
  line-height: 1.25;
}

.subscription-brand p,
.subscription-title span {
  color: var(--color-print-muted);
  font-weight: 500;
}

.subscription-brand p {
  font-size: 10px;
}

.subscription-title {
  flex: 0 0 auto;
  text-align: left;
}

.subscription-title h2 {
  font-size: 18px;
  line-height: 1.3;
}

.subscription-title span {
  font-size: 8px;
}

.subscription-brand-rule {
  height: 1.5px;
  margin-top: 12px;
  background: var(--color-print-brand);
}

.subscription-period {
  margin-top: 8px;
  font-size: 8px;
  font-weight: 600;
  text-align: right;
}

.subscription-summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  margin-top: 10px;
  border: 1px solid #d7e4dd;
  background: var(--color-print-tint);
}

.subscription-summary > div {
  min-width: 0;
  padding: 8px 6px 7px;
  text-align: center;
}

.subscription-summary > div + div {
  border-inline-start: 1px solid #d7e4dd;
}

.subscription-summary span,
.subscription-summary strong,
.subscription-summary :deep(.money-text) {
  display: block;
}

.subscription-summary span {
  color: var(--color-print-muted);
  font-size: 8px;
  white-space: nowrap;
}

.subscription-summary strong,
.subscription-summary :deep(.money-text) {
  margin-top: 3px;
  color: var(--color-print-brand-dark);
  font-size: 16px;
  font-weight: 700;
}

.subscription-section {
  margin-top: 10px;
}

.subscription-section h3 {
  padding-bottom: 4px;
  border-bottom: 1.5px solid var(--color-print-brand);
  font-size: 14px;
  line-height: 1.3;
}

.subscription-table-frame {
  margin-top: 5px;
  border: 1px solid #d7e4dd;
}

.subscription-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.subscription-table th,
.subscription-table td {
  padding: 6px 4px;
  border-bottom: 1px solid #d7e4dd;
  border-inline-start: 1px solid #d7e4dd;
  font-size: 9px;
  line-height: 1.35;
  text-align: center;
  overflow-wrap: anywhere;
}

.subscription-table tr > :first-child {
  border-inline-start: 0;
}

.subscription-table thead th {
  background: var(--color-print-brand);
  color: var(--color-canvas);
  font-weight: 700;
}

.subscription-table tbody tr:nth-child(even) td {
  background: #f4f8f6;
}

.subscription-table tbody tr:last-child td,
.subscription-table tfoot tr:last-child th {
  border-bottom: 0;
}

.subscription-table tfoot th {
  background: var(--color-print-tint);
  color: var(--color-ink);
  font-weight: 700;
}

.subscription-table :deep(.money-text) {
  color: inherit;
  font-size: inherit;
  font-weight: inherit;
}

.subscription-patient-column { width: 17%; }
.subscription-name-column { width: 24%; }
.subscription-count-column { width: 11%; }
.subscription-remaining-column { width: 11%; }
.subscription-price-column { width: 20%; }
.subscription-status-column { width: 17%; }
.supervision-doctor-column { width: 24%; }
.supervision-patient-column { width: 25%; }
.supervision-codes-column { width: 35%; }
.supervision-count-column { width: 16%; }

.numeric-cell {
  direction: ltr;
  font-variant-numeric: tabular-nums;
}

.subscription-empty {
  padding: 12px !important;
  color: var(--color-print-muted);
}

.subscription-note {
  margin-top: 5px;
  color: var(--color-print-muted);
  font-size: 6.5px;
}

.subscription-supervision-section {
  margin-top: 12px;
}

.subscription-footer {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  margin-top: 14px;
  padding-top: 7px;
  border-top: 1px solid #d7e4dd;
  color: var(--color-print-muted);
  font-size: 6.5px;
}

.subscription-page-number {
  direction: rtl;
  white-space: nowrap;
}

.report-document {
  min-height: 297mm;
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 14mm 13mm 10mm;
  border: 1px solid var(--color-print-border);
  background: var(--color-canvas);
  box-shadow: 0 4px 24px rgb(20 35 30 / 8%);
  color: var(--color-ink);
  font-size: 11px;
}

.report-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
}

.report-brand {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.report-brand h1,
.report-brand p,
.report-title h2 {
  margin: 0;
}

.report-brand h1 {
  color: var(--color-print-brand-dark);
  font-size: 24px;
  line-height: 1.25;
}

.report-brand p,
.report-title span {
  color: var(--color-print-muted);
  font-weight: 600;
}

.report-brand p {
  font-size: 13px;
}

.report-title {
  flex: 0 0 auto;
  text-align: left;
}

.report-title h2 {
  font-size: 21px;
}

.report-title span {
  font-size: 10px;
}

.brand-rule,
.section-rule {
  height: 2px;
  background: var(--color-print-brand);
}

.report-meta {
  display: grid;
  grid-template-columns: 1.35fr 1fr .65fr;
  border: 1px solid var(--color-print-border);
}

.report-meta > div {
  min-width: 0;
  padding: 9px 11px;
}

.report-meta > div + div {
  border-inline-start: 1px solid var(--color-print-border);
}

.report-meta span,
.report-meta strong,
.report-summary span {
  display: block;
}

.report-meta span,
.report-summary span {
  color: var(--color-print-muted);
  font-size: 10px;
  font-weight: 600;
}

.report-meta strong {
  margin-top: 2px;
  font-size: 12px;
}

.report-summary {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(0, 1fr));
  border: 1px solid var(--color-print-border);
  background: var(--color-print-tint);
}

.report-summary > div {
  min-width: 0;
  padding: 10px 12px;
}

.report-summary > div + div {
  border-inline-start: 1px solid var(--color-print-border);
}

.report-summary strong,
.report-summary :deep(.money-text) {
  display: block;
  margin-top: 3px;
  color: var(--color-ink);
  font-size: 16px;
  font-weight: 600;
}

.report-section h3 {
  margin: 0;
  font-size: 17px;
}

.section-rule {
  margin: 4px 0 8px;
}

.report-note {
  margin: 0;
  color: var(--color-print-muted);
  font-size: 9px;
}

.paper-footer {
  margin-top: auto;
  padding-top: 8px;
  border-top: 1px solid var(--color-print-border);
  color: var(--color-print-muted);
  font-size: 9px;
  text-align: center;
}

.footer-contact {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}

.paper-footer p {
  margin: 6px 0 0;
  font-size: 10px;
  font-weight: 600;
}

@media (max-width: 760px) {
  .report-document {
    min-height: auto;
    padding: 24px 18px;
  }

  .report-header {
    align-items: flex-start;
  }

  .report-brand h1 {
    font-size: 19px;
  }

  .report-title h2 {
    font-size: 17px;
  }
}

@media print {
  @page {
    size: A4 portrait;
    margin: 0;
  }

  .report-document {
    width: 210mm;
    min-height: 297mm;
    padding: 12mm 11mm 9mm;
    border: 0;
    box-shadow: none;
  }

  .subscription-document {
    padding: 12mm 11mm 9mm;
  }

  .subscription-table thead {
    display: table-header-group;
  }

  .subscription-table tr,
  .subscription-summary,
  .subscription-section h3,
  .subscription-note,
  .subscription-footer {
    break-inside: avoid;
  }
}
</style>
