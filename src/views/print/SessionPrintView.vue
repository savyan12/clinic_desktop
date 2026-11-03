<template>
  <div class="receipt-shell" dir="rtl">
    <v-skeleton-loader v-if="loading" class="print-loading" type="article, table" />
    <InlineError v-else-if="error" :message="error" />
    <article v-else-if="session" class="receipt-document">
      <header class="receipt-header">
        <div class="receipt-brand">
          <ClinicLogo v-if="settings.show_logo" :size="68" />
          <div>
            <h1>{{ settings.clinic_name }}</h1>
            <p>{{ settings.clinic_subtitle }}</p>
          </div>
        </div>
        <div class="receipt-title">
          <span class="document-badge">إيصال جلسة</span>
          <h2><bdi dir="ltr">{{ session.code }}</bdi></h2>
          <span>أُصدر في <bdi dir="auto">{{ formattedDate }}</bdi></span>
        </div>
      </header>

      <div class="brand-rule"></div>

      <section class="client-card" aria-label="بيانات المريض والجلسة">
        <div>
          <span>المريض</span>
          <strong>{{ session.patient.name }}</strong>
        </div>
        <div>
          <span>رقم الهاتف</span>
          <strong><bdi class="phone-value" dir="ltr">{{ session.patient.phone || '—' }}</bdi></strong>
        </div>
        <div>
          <span>الطبيب / الأخصائي</span>
          <strong>{{ session.doctor.name }}</strong>
        </div>
        <div>
          <span>رقم ملف المريض</span>
          <strong><bdi dir="ltr">#{{ session.patient.id }}</bdi></strong>
        </div>
      </section>

      <section class="services-section" aria-label="الخدمات">
        <table class="services-table">
          <thead>
            <tr>
              <th class="service-name">الخدمة / البيان</th>
              <th class="service-details">التفاصيل</th>
              <th class="service-total">الإجمالي</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="service in session.services" :key="service.id">
              <td class="service-name">{{ service.name }}</td>
              <td class="service-details">{{ serviceDescription(service) }}</td>
              <td class="service-total"><MoneyText :value="service.price" /></td>
            </tr>
            <tr v-if="!session.services.length">
              <td colspan="3" class="empty-line">لا توجد خدمات مسجلة</td>
            </tr>
          </tbody>
        </table>
      </section>

      <section class="receipt-financial-grid">
        <div class="totals-card">
          <h3>ملخص المبالغ</h3>
          <dl>
            <div>
              <dt>إجمالي الخدمات</dt>
              <dd><MoneyText :value="session.services_total" /></dd>
            </div>
            <div>
              <dt>الخصم</dt>
              <dd><MoneyText :value="session.discount" /></dd>
            </div>
            <div class="net-amount">
              <dt>الصافي المطلوب</dt>
              <dd><MoneyText :value="session.net_total" /></dd>
            </div>
          </dl>
        </div>

        <div class="payment-card">
          <h3>بيانات الجلسة والدفع</h3>
          <dl>
            <div>
              <dt>طريقة الدفع / التغطية</dt>
              <dd><bdi dir="auto">{{ coverageDescription }}</bdi></dd>
            </div>
            <div>
              <dt>نوع الجلسة</dt>
              <dd>{{ sessionTypeDescription }}</dd>
            </div>
            <div>
              <dt>ملاحظة</dt>
              <dd>{{ settlementNote }}</dd>
            </div>
          </dl>
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
import { getSession } from '@/api/sessions'
import ClinicLogo from '@/components/common/ClinicLogo.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import { labelFor } from '@/constants/labels'

const route = useRoute()
const session = ref(null)
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

const formattedDate = computed(() => {
  if (!session.value?.opened_at) return '—'
  return new Intl.DateTimeFormat('ar-LY-u-nu-latn', {
    dateStyle: 'long',
    timeStyle: 'short',
  }).format(new Date(session.value.opened_at))
})

const coverageDescription = computed(() => {
  if (!session.value) return '—'
  const { coverage, method } = session.value.settlement
  if (['cash', 'transfer'].includes(coverage) && method) {
    return labelFor('paymentMethod', method)
  }
  return labelFor('coverage', coverage)
})

const sessionTypeDescription = computed(() => {
  if (!session.value) return '—'
  return labelFor('sessionType', session.value.type)
})

const settlementNote = computed(() => {
  if (!session.value) return 'لا يوجد'
  if (session.value.type === 'subscription') return 'الاشتراك يغطي التكلفة'
  if (Number(session.value.settlement.debt_amount) > 0) return 'يتضمن المبلغ دينًا مسجلاً على الحساب'
  if (Number(session.value.settlement.wallet_amount) > 0) return 'تم استخدام الرصيد المدفوع مسبقًا'
  return 'لا يوجد'
})

const serviceDescription = (service) => {
  const details = []
  const unitName = service.unit_name || 'وحدة'
  if (service.item_type === 'extra_units') {
    details.push(`${service.extra_units || service.total_units || 0} ${unitName} إضافية`)
  } else {
    if (Number(service.included_units) > 0) details.push(`يشمل ${service.included_units} ${unitName}`)
    if (Number(service.extra_units) > 0) details.push(`${service.extra_units} ${unitName} إضافية`)
  }
  if (service.is_addition) details.push('إضافة لاحقة')
  return details.join('، ') || 'الخدمة الأساسية'
}

onMounted(async () => {
  try {
    session.value = (await getSession(route.params.id)).data
  } catch (apiError) {
    error.value = apiError.message
  } finally {
    loading.value = false
  }
})
</script>

<style scoped>
.receipt-shell {
  display: grid;
  place-items: start center;
  direction: rtl;
}

.print-loading,
.receipt-document {
  width: min(100%, 210mm);
}

.receipt-document {
  min-height: 297mm;
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 14mm 13mm 10mm;
  border: 1px solid var(--color-print-border);
  background: var(--color-canvas);
  box-shadow: 0 4px 24px rgb(20 35 30 / 8%);
  color: var(--color-ink);
  font-size: 12px;
  -webkit-print-color-adjust: exact;
  print-color-adjust: exact;
}

.receipt-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
}

.receipt-brand {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
}

.receipt-brand h1,
.receipt-brand p,
.receipt-title h2,
.receipt-title span {
  margin: 0;
}

.receipt-brand h1 {
  color: var(--color-print-brand-dark);
  font-size: 26px;
  line-height: 1.25;
}

.receipt-brand p {
  margin-top: 2px;
  color: var(--color-print-muted);
  font-size: 14px;
  font-weight: 600;
}

.receipt-title {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  text-align: left;
}

.document-badge {
  display: inline-block;
  margin-bottom: 3px !important;
  padding: 3px 9px;
  border: 1px solid #cce2d8;
  border-radius: 5px;
  background: var(--color-print-tint);
  color: var(--color-print-brand-dark);
  font-size: 11px;
  font-weight: 700;
}

.receipt-title h2 {
  font-size: 20px;
  line-height: 1.35;
}

.receipt-title > span:last-child {
  color: var(--color-print-muted);
  font-size: 10px;
  font-weight: 600;
}

.brand-rule {
  height: 2px;
  background: var(--color-print-brand);
}

.client-card {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px 28px;
  padding: 13px 15px;
  border: 1px solid var(--color-print-border);
  background: #fbfcfd;
}

.client-card > div {
  min-width: 0;
}

.client-card span,
.client-card strong {
  display: block;
}

.client-card span,
.totals-card dt,
.payment-card dt {
  color: var(--color-print-muted);
  font-size: 10px;
  font-weight: 600;
}

.client-card strong {
  margin-top: 3px;
  font-size: 13px;
  overflow-wrap: anywhere;
}

.services-section {
  overflow: hidden;
  border: 1px solid var(--color-print-border);
}

.services-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.services-table thead {
  background: var(--color-print-brand);
  color: var(--color-canvas);
}

.services-table th {
  padding: 10px 12px;
  font-size: 11px;
  font-weight: 700;
  text-align: right;
}

.services-table th + th {
  border-inline-start: 1px solid rgb(255 255 255 / 28%);
}

.services-table td {
  padding: 11px 12px;
  border-bottom: 1px solid var(--color-print-border);
  font-size: 11px;
  line-height: 1.55;
  vertical-align: middle;
  overflow-wrap: anywhere;
}

.services-table tbody tr:last-child td {
  border-bottom: 0;
}

.services-table td + td {
  border-inline-start: 1px solid var(--color-print-border);
}

.service-name {
  width: 45%;
  font-weight: 700;
}

.service-details {
  width: 35%;
}

td.service-details {
  color: var(--color-print-muted);
}

.service-total {
  width: 20%;
  text-align: center !important;
}

.service-total :deep(.money-text) {
  display: inline-block;
  color: inherit;
  font-size: 12px;
  font-weight: 700;
}

.empty-line {
  padding: 18px 10px !important;
  color: var(--color-print-muted);
  font-weight: 500;
  text-align: center;
}

.receipt-financial-grid {
  display: grid;
  grid-template-columns: minmax(0, .8fr) minmax(0, 1.2fr);
  gap: 14px;
  align-items: start;
}

.totals-card,
.payment-card {
  overflow: hidden;
  border: 1px solid var(--color-print-border);
}

.totals-card h3,
.payment-card h3 {
  margin: 0;
  padding: 8px 11px;
  background: #fbfcfd;
  border-bottom: 1px solid var(--color-print-border);
  font-size: 12px;
}

.totals-card dl,
.payment-card dl {
  margin: 0;
}

.totals-card dl > div,
.payment-card dl > div {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-height: 38px;
  padding: 8px 11px;
}

.totals-card dl > div + div,
.payment-card dl > div + div {
  border-top: 1px solid var(--color-print-border);
}

.totals-card dt,
.totals-card dd,
.payment-card dt,
.payment-card dd {
  margin: 0;
}

.totals-card dd,
.payment-card dd {
  max-width: 64%;
  font-size: 11px;
  font-weight: 700;
  text-align: left;
  overflow-wrap: anywhere;
}

.totals-card :deep(.money-text) {
  color: inherit;
  font-size: 12px;
}

.totals-card .net-amount {
  background: var(--color-print-tint);
  border-top: 1px solid var(--color-print-brand);
}

.net-amount dt,
.net-amount dd,
.net-amount :deep(.money-text) {
  color: var(--color-print-brand-dark);
  font-size: 15px;
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
  .receipt-document {
    min-height: auto;
    padding: 24px 18px;
  }

  .receipt-header {
    align-items: flex-start;
  }

  .receipt-brand h1 {
    font-size: 20px;
  }

  .receipt-title h2 {
    font-size: 17px;
  }

  .receipt-financial-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 520px) {
  .receipt-header,
  .footer-contact {
    flex-direction: column;
  }

  .receipt-title {
    align-items: flex-start;
    text-align: right;
  }

  .client-card {
    grid-template-columns: 1fr;
  }
}

@media print {
  @page {
    size: A4 portrait;
    margin: 0;
  }

  .receipt-document {
    width: 210mm;
    min-height: 297mm;
    padding: 12mm 11mm 9mm;
    border: 0;
    box-shadow: none;
  }

  .receipt-header,
  .client-card,
  .receipt-financial-grid,
  .paper-footer {
    break-inside: avoid;
  }

  .services-table thead {
    display: table-header-group;
  }

  .services-table tr {
    break-inside: avoid;
  }
}
</style>
