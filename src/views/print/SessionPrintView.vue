<template>
  <div class="receipt-shell">
    <v-skeleton-loader v-if="loading" class="print-loading" type="article, table" />
    <InlineError v-else-if="error" :message="error" />
    <article v-else-if="session" class="receipt-document">
      <header class="receipt-header">
        <ClinicLogo v-if="settings.show_logo" :size="58" />
        <div>
          <h1>{{ settings.clinic_name }}</h1>
          <p>{{ settings.clinic_subtitle }}</p>
        </div>
      </header>

      <div class="brand-rule"></div>
      <p class="receipt-reference">إيصال جلسة · {{ session.code }}</p>

      <section class="receipt-meta">
        <div class="meta-item">
          <span>المريض</span>
          <strong>{{ session.patient.name }}</strong>
        </div>
        <div class="meta-item">
          <span>رقم الهاتف</span>
          <strong class="ltr-value">{{ session.patient.phone }}</strong>
        </div>
        <div class="meta-item">
          <span>الطبيب</span>
          <strong>{{ session.doctor.name }}</strong>
        </div>
        <div class="meta-item">
          <span>التاريخ</span>
          <strong>{{ formattedDate }}</strong>
        </div>
      </section>

      <section class="receipt-services">
        <div class="services-head">
          <span>الخدمة</span>
          <span>السعر</span>
        </div>
        <div
          v-for="service in session.services"
          :key="service.id"
          class="service-row"
        >
          <strong>{{ service.name }}<small v-if="service.extra_units"> · {{ service.extra_units }} {{ service.unit_name || 'وحدة' }} إضافية</small><small v-if="service.is_addition"> · إضافة لاحقة</small></strong>
          <MoneyText :value="service.price" />
        </div>
      </section>

      <section class="receipt-totals">
        <div>
          <span>إجمالي الخدمات</span>
          <MoneyText :value="session.services_total" />
        </div>
        <div>
          <span>الخصم</span>
          <MoneyText :value="session.discount" />
        </div>
        <div class="total-net">
          <strong>الصافي</strong>
          <MoneyText :value="session.net_total" />
        </div>
      </section>

      <section class="receipt-details">
        <div>
          <span>التغطية / الدفع</span>
          <strong>{{ coverageDescription }}</strong>
        </div>
        <div>
          <span>ملاحظة</span>
          <strong>{{ settlementNote }}</strong>
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
  const coverage = labelFor('coverage', session.value.settlement.coverage)
  if (session.value.type === 'subscription') {
    return `${coverage} #${session.value.subscription_id}`
  }
  const method = session.value.settlement.method
    ? labelFor('paymentMethod', session.value.settlement.method)
    : null
  return method ? `${coverage} · ${method}` : coverage
})

const settlementNote = computed(() => {
  if (!session.value) return 'لا يوجد'
  if (session.value.type === 'subscription') return 'لا يوجد قبض جديد لهذه الجلسة'
  if (Number(session.value.settlement.debt_amount) > 0) return 'يتضمن المبلغ دينًا مسجلاً على الحساب'
  if (Number(session.value.settlement.wallet_amount) > 0) return 'تم استخدام الرصيد المدفوع مسبقًا'
  return 'لا يوجد'
})

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
}

.print-loading,
.receipt-document {
  width: min(100%, 105mm);
}

.receipt-document {
  min-height: 148mm;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 7mm 6mm 5mm;
  overflow: hidden;
  border: 1px solid var(--color-print-border);
  background: var(--color-canvas);
  color: var(--color-ink);
  font-size: 12px;
}

.receipt-header {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  text-align: right;
}

.receipt-header h1,
.receipt-header p {
  margin: 0;
}

.receipt-header h1 {
  color: var(--color-print-brand-dark);
  font-size: 23px;
  line-height: 1.2;
}

.receipt-header p {
  color: var(--color-print-muted);
  font-size: 14px;
  font-weight: 600;
}

.brand-rule {
  height: 2px;
  background: var(--color-print-brand);
}

.receipt-reference {
  margin: -5px 0 0;
  color: var(--color-print-muted);
  font-size: 10px;
  text-align: center;
}

.receipt-meta {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px 22px;
}

.meta-item span,
.meta-item strong {
  display: block;
}

.meta-item span,
.receipt-details span {
  color: var(--color-print-muted);
  font-size: 10px;
  font-weight: 600;
}

.meta-item strong {
  margin-top: 1px;
  font-size: 13px;
  line-height: 1.35;
}

.receipt-services {
  border: 1px solid var(--color-print-brand);
}

.services-head,
.service-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 95px;
  align-items: center;
  gap: 12px;
  padding: 7px 10px;
}

.services-head {
  background: var(--color-print-brand);
  color: var(--color-canvas);
  font-weight: 600;
}

.service-row + .service-row {
  border-top: 1px solid var(--color-print-border);
}

.service-row :deep(.money-text),
.receipt-totals :deep(.money-text) {
  justify-self: end;
  color: inherit;
}

.receipt-totals {
  display: grid;
}

.receipt-totals > div {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 5px 8px;
}

.receipt-totals span {
  color: var(--color-print-muted);
  font-weight: 600;
}

.receipt-totals .total-net {
  background: var(--color-print-tint);
  color: var(--color-print-brand-dark);
  font-size: 17px;
}

.total-net :deep(.money-text) {
  color: var(--color-ink);
  font-weight: 600;
}

.receipt-details {
  display: grid;
}

.receipt-details > div {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 16px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--color-print-border);
}

.receipt-details strong {
  text-align: left;
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
  margin: 7px 0 0;
  font-size: 11px;
  font-weight: 600;
}

@media (max-width: 520px) {
  .receipt-document {
    min-height: auto;
    padding: 22px 18px;
  }
}

@media print {
  @page {
    size: A6 portrait;
    margin: 0;
  }

  .receipt-document {
    width: 105mm;
    min-height: 148mm;
    border: 0;
  }
}
</style>
