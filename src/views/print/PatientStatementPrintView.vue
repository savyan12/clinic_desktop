<template>
  <div class="statement-shell" dir="rtl">
    <v-skeleton-loader v-if="loading" class="print-loading" type="article, table" />
    <InlineError v-else-if="error" :message="error" />
    <article v-else-if="patient" class="statement-document">
      <header class="statement-header">
        <div class="statement-brand">
          <ClinicLogo v-if="settings.show_logo" :size="68" />
          <div>
            <h1>{{ settings.clinic_name }}</h1>
            <p>{{ settings.clinic_subtitle }}</p>
          </div>
        </div>
        <div class="statement-title">
          <h2>كشف حساب مريض</h2>
          <span>أُعد في <bdi dir="auto">{{ generatedAt }}</bdi></span>
        </div>
      </header>

      <div class="brand-rule"></div>

      <section class="patient-meta" aria-label="بيانات المريض">
        <div>
          <span>المريض</span>
          <strong>{{ patient.name }}</strong>
        </div>
        <div>
          <span>الهاتف</span>
          <strong><bdi class="phone-value" dir="ltr">{{ patient.phone || '—' }}</bdi></strong>
        </div>
        <div>
          <span>الجهة الراعية</span>
          <strong>{{ patient.sponsor?.name || 'لا توجد' }}</strong>
        </div>
      </section>

      <section class="balance-strip" aria-label="ملخص الأرصدة">
        <div>
          <span>الرصيد المدفوع مسبقًا</span>
          <MoneyText :value="patient.wallet_balance" classic />
        </div>
        <div>
          <span>الديون القائمة</span>
          <MoneyText :value="patient.debt_total" classic />
        </div>
      </section>

      <section class="statement-section debts-section">
        <h3>الديون</h3>
        <div class="section-rule"></div>
        <div class="table-wrap">
          <table class="statement-table debts-table">
            <thead>
              <tr>
                <th>المرجع</th>
                <th>النوع</th>
                <th>الأصل</th>
                <th>المسدد</th>
                <th>المتبقي</th>
                <th>الحالة</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="debt in patient.debts" :key="debt.id">
                <td><bdi dir="ltr">#{{ debt.id }}</bdi></td>
                <td>{{ labelFor('debtKind', debt.kind) }}</td>
                <td><MoneyText :value="debt.amount" classic /></td>
                <td><MoneyText :value="debt.paid_amount" classic /></td>
                <td><MoneyText :value="debt.remaining" classic /></td>
                <td>{{ labelFor('debtStatus', debt.status) }}</td>
              </tr>
              <tr v-if="!patient.debts.length">
                <td colspan="6" class="empty-line">لا توجد ديون قائمة</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section class="statement-section payments-section">
        <h3>المدفوعات والاسترجاعات</h3>
        <div class="section-rule"></div>
        <div class="table-wrap">
          <table class="statement-table payments-table">
            <thead>
              <tr>
                <th>التاريخ</th>
                <th>النوع</th>
                <th>المريض</th>
                <th>طريقة الدفع</th>
                <th>المبلغ</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="payment in sortedPayments" :key="payment.id">
                <td class="payment-date">
                  <strong>{{ formatPaymentDate(payment.created_at) }}</strong>
                  <small>{{ formatPaymentTime(payment.created_at) }}</small>
                </td>
                <td>{{ labelFor('paymentType', payment.type) }}</td>
                <td>{{ patient.name }}</td>
                <td>{{ labelFor('paymentMethod', payment.method) }}</td>
                <td class="payment-amount"><MoneyText :value="payment.amount" classic /></td>
              </tr>
              <tr v-if="!sortedPayments.length">
                <td colspan="5" class="empty-line">لا توجد حركات مالية خلال الفترة</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section class="statement-totals" aria-label="ملخص المدفوعات">
        <div>
          <span>المقبوضات</span>
          <MoneyText :value="receiptsTotal" classic />
        </div>
        <div>
          <span>الاسترجاعات</span>
          <MoneyText :value="refundsTotal" classic />
        </div>
        <div class="net-total">
          <strong>الصافي</strong>
          <MoneyText :value="netTotal" classic />
        </div>
      </section>

      <footer class="paper-footer">
        <div class="footer-contact">
          <span>{{ settings.address || '—' }}</span>
          <strong><bdi class="phone-value" dir="ltr">{{ settings.phone || '—' }}</bdi></strong>
        </div>
        <p>{{ settings.receipt_footer || 'نتمنى لكم دوام الصحة والعافية' }}</p>
      </footer>
    </article>
  </div>
</template>

<script setup>
import { computed, inject, onMounted, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'
import { getPatient, getPatientStatement } from '@/api/patients'
import ClinicLogo from '@/components/common/ClinicLogo.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import { numeric } from '@/composables/useMoney'
import { labelFor } from '@/constants/labels'

const route = useRoute()
const patient = ref(null)
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

const generatedAt = computed(() => new Intl.DateTimeFormat('ar-LY-u-nu-latn', {
  dateStyle: 'long',
  timeStyle: 'short',
}).format(new Date()))

const sortedPayments = computed(() => [...(patient.value?.payments || [])]
  .sort((left, right) => new Date(right.created_at) - new Date(left.created_at)))

const receiptsTotal = computed(() => sortedPayments.value
  .filter((payment) => numeric(payment.amount) > 0)
  .reduce((total, payment) => total + numeric(payment.amount), 0)
  .toFixed(3))

const refundsTotal = computed(() => Math.abs(sortedPayments.value
  .filter((payment) => numeric(payment.amount) < 0)
  .reduce((total, payment) => total + numeric(payment.amount), 0))
  .toFixed(3))

const netTotal = computed(() => sortedPayments.value
  .reduce((total, payment) => total + numeric(payment.amount), 0)
  .toFixed(3))

onMounted(async () => {
  try {
    const [detailsResponse, statementResponse] = await Promise.all([
      getPatient(route.params.id),
      getPatientStatement(route.params.id, route.query),
    ])
    patient.value = {
      ...detailsResponse.data,
      ...statementResponse.data.patient,
      wallet_balance: statementResponse.data.wallet_balance,
      wallet_movements: statementResponse.data.wallet_movements,
      debts: statementResponse.data.debts,
      payments: statementResponse.data.payments,
      allocations: statementResponse.data.allocations,
    }
  } catch (apiError) {
    error.value = apiError.message
  } finally {
    loading.value = false
  }
})

const formatPaymentDate = (value) => new Intl.DateTimeFormat('en-CA', {
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  timeZone: 'Africa/Tripoli',
}).format(new Date(value))

const formatPaymentTime = (value) => new Intl.DateTimeFormat('en-GB', {
  hour: '2-digit',
  minute: '2-digit',
  hour12: false,
  timeZone: 'Africa/Tripoli',
}).format(new Date(value))
</script>

<style scoped>
.statement-shell {
  display: grid;
  place-items: start center;
  direction: rtl;
}

.print-loading,
.statement-document {
  width: min(100%, 210mm);
}

.statement-document {
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
  -webkit-print-color-adjust: exact;
  print-color-adjust: exact;
}

.statement-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
}

.statement-brand {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
}

.statement-brand h1,
.statement-brand p,
.statement-title h2 {
  margin: 0;
}

.statement-brand h1 {
  color: var(--color-print-brand-dark);
  font-size: 26px;
  line-height: 1.25;
}

.statement-brand p,
.statement-title span {
  color: var(--color-print-muted);
  font-weight: 600;
}

.statement-brand p {
  margin-top: 2px;
  font-size: 14px;
}

.statement-title {
  flex: 0 0 auto;
  text-align: left;
}

.statement-title h2 {
  font-size: 21px;
}

.statement-title span {
  font-size: 10px;
}

.brand-rule,
.section-rule {
  height: 2px;
  background: var(--color-print-brand);
}

.patient-meta {
  display: grid;
  grid-template-columns: 1.2fr .8fr 1fr;
  border: 1px solid var(--color-print-border);
}

.patient-meta > div {
  min-width: 0;
  padding: 11px 13px;
}

.patient-meta > div + div {
  border-inline-start: 1px solid var(--color-print-border);
}

.patient-meta span,
.patient-meta strong,
.balance-strip span {
  display: block;
}

.patient-meta span,
.balance-strip span {
  color: var(--color-print-muted);
  font-size: 10px;
  font-weight: 600;
}

.patient-meta strong {
  margin-top: 3px;
  font-size: 13px;
  overflow-wrap: anywhere;
}

.balance-strip {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  border: 1px solid var(--color-print-border);
  background: var(--color-print-tint);
}

.balance-strip > div {
  min-width: 0;
  padding: 11px 13px;
}

.balance-strip > div + div {
  border-inline-start: 1px solid var(--color-print-border);
}

.balance-strip :deep(.money-text) {
  display: block;
  margin-top: 3px;
  color: var(--color-ink);
  font-size: 18px;
  font-weight: 700;
}

.statement-section {
  margin-top: 4px;
}

.statement-section h3 {
  margin: 0;
  font-size: 17px;
}

.section-rule {
  margin: 4px 0 8px;
}

.table-wrap {
  overflow: hidden;
  border: 1px solid var(--color-print-border);
}

.statement-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.statement-table thead {
  background: var(--color-print-brand);
  color: var(--color-canvas);
}

.statement-table th {
  padding: 8px 6px;
  border-inline-start: 1px solid rgb(255 255 255 / 28%);
  font-size: 10px;
  font-weight: 700;
  text-align: center;
}

.statement-table th:first-child {
  border-inline-start: 0;
}

.statement-table td {
  padding: 9px 6px;
  border-bottom: 1px solid var(--color-print-border);
  font-size: 10px;
  line-height: 1.5;
  text-align: center;
  vertical-align: middle;
  overflow-wrap: anywhere;
}

.statement-table td + td {
  border-inline-start: 1px solid var(--color-print-border);
}

.statement-table tbody tr:last-child td {
  border-bottom: 0;
}

.statement-table :deep(.money-text) {
  display: inline-block;
  color: inherit;
  font-size: inherit;
}

.debts-table th {
  width: 16.666%;
}

.payments-table th:nth-child(1) {
  width: 17%;
}

.payments-table th:nth-child(2) {
  width: 16%;
}

.payments-table th:nth-child(3) {
  width: 27%;
}

.payments-table th:nth-child(4) {
  width: 17%;
}

.payments-table th:nth-child(5) {
  width: 23%;
}

.payment-date strong,
.payment-date small {
  display: block;
  direction: ltr;
  text-align: center;
}

.payment-date strong {
  font-size: 10px;
}

.payment-date small {
  margin-top: 2px;
  color: var(--color-print-muted);
  font-size: 9px;
}

.payment-amount {
  text-align: left !important;
}

.payment-amount :deep(.money-text) {
  font-size: 11px;
  font-weight: 700;
}

.empty-line {
  padding: 18px 8px !important;
  color: var(--color-print-muted);
  font-weight: 500;
  text-align: center;
}

.statement-totals {
  width: 270px;
  max-width: 100%;
  align-self: flex-start;
  border: 1px solid var(--color-print-border);
}

.statement-totals > div {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 7px 11px;
}

.statement-totals > div + div {
  border-top: 1px solid var(--color-print-border);
}

.statement-totals :deep(.money-text) {
  color: inherit;
  font-size: 11px;
  font-weight: 600;
}

.statement-totals .net-total {
  background: var(--color-print-tint);
  border-top: 1px solid var(--color-print-brand);
  color: var(--color-print-brand-dark);
}

.net-total strong,
.net-total :deep(.money-text) {
  font-size: 13px;
  font-weight: 700;
}

.paper-footer {
  margin-top: auto;
  padding-top: 10px;
  border-top: 1px solid var(--color-print-border);
  color: var(--color-print-muted);
  font-size: 9px;
  text-align: center;
}

.footer-contact {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
}

.footer-contact strong {
  color: var(--color-ink);
}

.paper-footer p {
  margin: 7px 0 0;
  font-size: 11px;
  font-weight: 600;
}

@media (max-width: 760px) {
  .statement-document {
    min-height: auto;
    padding: 24px 18px;
  }

  .statement-header {
    align-items: flex-start;
  }

  .statement-brand h1 {
    font-size: 20px;
  }

  .statement-title h2 {
    font-size: 17px;
  }
}

@media (max-width: 560px) {
  .statement-header,
  .footer-contact {
    flex-direction: column;
  }

  .statement-title {
    text-align: right;
  }

  .patient-meta {
    grid-template-columns: 1fr;
  }

  .patient-meta > div + div {
    border-inline-start: 0;
    border-top: 1px solid var(--color-print-border);
  }

  .statement-table th,
  .statement-table td {
    padding-inline: 3px;
    font-size: 9px;
  }
}

@media print {
  @page {
    size: A4 portrait;
    margin: 0;
  }

  .statement-document {
    width: 210mm;
    min-height: 297mm;
    padding: 12mm 11mm 9mm;
    border: 0;
    box-shadow: none;
  }

  .statement-header,
  .patient-meta,
  .balance-strip,
  .statement-totals,
  .paper-footer {
    break-inside: avoid;
  }

  .statement-table thead {
    display: table-header-group;
  }

  .statement-table tr {
    break-inside: avoid;
  }
}
</style>
