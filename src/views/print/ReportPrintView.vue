<template>
  <div class="report-print-shell" dir="rtl">
    <v-skeleton-loader v-if="loading" class="print-loading" type="article, table" />
    <InlineError v-else-if="error" :message="error" />
    <article v-else-if="report" class="report-document">
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
const generatedAt = computed(() => formatReportDateTime(report.value?.meta?.generated_at))
const periodLabel = computed(() => reportPeriodLabel(route.query.from, route.query.to))
const summaryItems = computed(() => reportSummaryItems(type.value, report.value?.totals))

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
}
</style>
