<template>
  <div class="statement-shell">
    <v-skeleton-loader v-if="loading" class="print-loading" type="article, table" />
    <InlineError v-else-if="error" :message="error" />
    <article v-else-if="patient" class="statement-document">
      <header class="statement-header">
        <div class="statement-brand">
          <ClinicLogo v-if="settings.show_logo" :size="52" />
          <div>
            <h1>{{ settings.clinic_name }}</h1>
            <p>{{ settings.clinic_subtitle }}</p>
          </div>
        </div>
        <div class="statement-title">
          <h2>كشف حساب مريض</h2>
          <span>أُعد في {{ generatedAt }}</span>
        </div>
      </header>

      <div class="brand-rule"></div>

      <section class="patient-meta">
        <div>
          <span>المريض</span>
          <strong>{{ patient.name }}</strong>
        </div>
        <div>
          <span>الهاتف</span>
          <strong class="ltr-value">{{ patient.phone }}</strong>
        </div>
        <div>
          <span>الجهة الراعية</span>
          <strong>{{ patient.sponsor?.name || 'لا توجد' }}</strong>
        </div>
      </section>

      <section class="balance-strip">
        <div>
          <span>الرصيد المدفوع مسبقًا</span>
          <MoneyText :value="patient.wallet_balance" />
        </div>
        <div>
          <span>الديون القائمة</span>
          <MoneyText :value="patient.debt_total" />
        </div>
      </section>

      <section class="statement-section debts-section">
        <h3>الديون</h3>
        <p v-if="!patient.debts.length" class="empty-line">لا توجد ديون قائمة</p>
        <div v-else class="compact-table debts-table">
          <div class="table-head">
            <span>المرجع</span>
            <span>النوع</span>
            <span>الأصل</span>
            <span>المسدد</span>
            <span>المتبقي</span>
            <span>الحالة</span>
          </div>
          <div v-for="debt in patient.debts" :key="debt.id" class="table-row">
            <span>#{{ debt.id }}</span>
            <span>{{ labelFor('debtKind', debt.kind) }}</span>
            <MoneyText :value="debt.amount" />
            <MoneyText :value="debt.paid_amount" />
            <MoneyText :value="debt.remaining" />
            <span>{{ labelFor('debtStatus', debt.status) }}</span>
          </div>
        </div>
      </section>

      <section class="statement-section payments-section">
        <h3>المدفوعات والاسترجاعات</h3>
        <p v-if="!sortedPayments.length" class="empty-line">لا توجد حركات مالية خلال الفترة</p>
        <div v-else class="compact-table payments-table">
          <div class="table-head">
            <span>التاريخ</span>
            <span>النوع</span>
            <span>المريض</span>
            <span>الطريقة</span>
            <span>المبلغ</span>
          </div>
          <div v-for="payment in sortedPayments" :key="payment.id" class="table-row">
            <span class="payment-date">
              <strong>{{ formatPaymentDate(payment.created_at) }}</strong>
              <small>{{ formatPaymentTime(payment.created_at) }}</small>
            </span>
            <span>{{ labelFor('paymentType', payment.type) }}</span>
            <span>{{ patient.name }}</span>
            <span>{{ labelFor('paymentMethod', payment.method) }}</span>
            <MoneyText :value="payment.amount" />
          </div>
        </div>
      </section>

      <section class="statement-totals">
        <div>
          <span>المقبوضات</span>
          <MoneyText :value="receiptsTotal" />
        </div>
        <div>
          <span>الاسترجاعات</span>
          <MoneyText :value="refundsTotal" />
        </div>
        <div class="net-total">
          <strong>الصافي</strong>
          <MoneyText :value="netTotal" />
        </div>
      </section>

      <footer class="paper-footer">
        <div class="footer-contact">
          <span>{{ settings.address || '—' }}</span>
          <strong class="ltr-value">{{ settings.phone || '—' }}</strong>
        </div>
        <p>{{ settings.receipt_footer }}</p>
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
}

.print-loading,
.statement-document {
  width: min(100%, 105mm);
}

.statement-document {
  min-height: 148mm;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 5mm 5.5mm 4mm;
  border: 1px solid var(--color-print-border);
  background: var(--color-canvas);
  color: var(--color-ink);
  font-size: 9px;
}

.statement-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.statement-brand {
  display: flex;
  align-items: center;
  gap: 7px;
}

.statement-brand h1,
.statement-brand p,
.statement-title h2 {
  margin: 0;
}

.statement-brand h1 {
  color: var(--color-print-brand-dark);
  font-size: 17px;
  line-height: 1.2;
}

.statement-brand p,
.statement-title span {
  color: var(--color-print-muted);
  font-weight: 600;
}

.statement-brand p {
  font-size: 10px;
}

.statement-title {
  text-align: left;
}

.statement-title h2 {
  font-size: 16px;
}

.statement-title span {
  font-size: 8px;
}

.brand-rule {
  height: 2px;
  background: var(--color-print-brand);
}

.patient-meta {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.patient-meta > div {
  padding: 5px 8px;
  text-align: center;
}

.patient-meta > div + div {
  border-right: 1px solid var(--color-print-border);
}

.patient-meta span,
.patient-meta strong,
.balance-strip span {
  display: block;
}

.patient-meta span,
.balance-strip span {
  color: var(--color-print-muted);
  font-weight: 600;
}

.patient-meta strong {
  margin-top: 2px;
  font-size: 10px;
}

.balance-strip {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  background: var(--color-print-tint);
}

.balance-strip > div {
  padding: 7px 10px;
}

.balance-strip > div + div {
  border-right: 1px solid var(--color-print-border);
}

.balance-strip :deep(.money-text) {
  display: block;
  margin-top: 2px;
  color: var(--color-ink);
  font-size: 16px;
  font-weight: 600;
}

.statement-section h3 {
  margin: 0 0 5px;
  padding-bottom: 2px;
  border-bottom: 2px solid var(--color-print-brand);
  font-size: 14px;
}

.empty-line {
  margin: 10px 0;
  color: var(--color-print-muted);
  text-align: center;
}

.compact-table {
  display: grid;
}

.table-head,
.table-row {
  display: grid;
  align-items: center;
  gap: 5px;
}

.table-head {
  padding: 4px 5px;
  background: var(--color-print-brand);
  color: var(--color-canvas);
  font-weight: 600;
}

.table-row {
  padding: 4px 5px;
  border-bottom: 1px solid var(--color-print-border);
}

.table-row :deep(.money-text) {
  color: inherit;
  font-size: inherit;
}

.debts-table .table-head,
.debts-table .table-row {
  grid-template-columns: 38px 52px repeat(3, minmax(48px, 1fr)) 42px;
}

.payments-table .table-head,
.payments-table .table-row {
  grid-template-columns: 55px 64px minmax(80px, 1fr) 46px 58px;
}

.payment-date strong,
.payment-date small {
  display: block;
  direction: ltr;
  text-align: center;
}

.payment-date small {
  color: var(--color-print-muted);
}

.statement-totals {
  width: 48%;
  margin-top: auto;
  margin-right: auto;
}

.statement-totals > div {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 2px 6px;
  border-bottom: 1px solid var(--color-print-border);
}

.statement-totals .net-total {
  background: var(--color-print-tint);
  color: var(--color-print-brand-dark);
  font-size: 11px;
}

.net-total :deep(.money-text) {
  color: var(--color-ink);
  font-weight: 600;
}

.paper-footer {
  padding-top: 5px;
  border-top: 1px solid var(--color-print-border);
  color: var(--color-print-muted);
  font-size: 7px;
  text-align: center;
}

.footer-contact {
  display: flex;
  justify-content: space-between;
  gap: 10px;
}

.paper-footer p {
  margin: 4px 0 0;
  font-size: 8px;
  font-weight: 600;
}

@media (max-width: 520px) {
  .statement-document {
    min-height: auto;
    padding: 18px 14px;
  }
}

@media print {
  @page {
    size: A6 portrait;
    margin: 0;
  }

  .statement-document {
    width: 105mm;
    min-height: 148mm;
    border: 0;
  }
}
</style>
