<template>
  <div class="page-container page-stack">
    <template v-if="loading"><v-skeleton-loader type="heading, image, table" /></template>
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="sponsor">
      <PageHeader :title="sponsor.name"><template #subtitle>{{ sponsor.address || '—' }} · <bdi class="phone-value" dir="ltr">{{ sponsor.phone || '—' }}</bdi></template><template #actions><v-btn color="primary" prepend-icon="mdi-cash-check" :disabled="!openDebts.length" @click="settlementDialog = true">تسديد المستحقات</v-btn><v-btn :to="`/print/statement/sponsor/${sponsor.id}`" variant="outlined" prepend-icon="mdi-printer-outline">طباعة كشف الحساب</v-btn></template></PageHeader>
      <section class="metric-grid"><StatCell label="المرضى المرتبطون" :value="sponsor.patients.length" caption="سجلات مرضى" /><StatCell label="إجمالي المستحقات" :value="sponsor.total_due" money caption="قيمة أصلية" /><StatCell label="المبالغ المسددة" :value="sponsor.total_paid" money caption="دفعات مسجلة" /><StatCell label="المتبقي" :value="sponsor.remaining" money caption="مستحق على الجهة" /></section>
      <section class="surface-card"><h2 class="section-heading">المرضى التابعون للجهة</h2><div v-if="sponsor.patients.length" class="patient-grid"><router-link v-for="patient in sponsor.patients" :key="patient.id" :to="`/patients/${patient.id}`"><div class="patient-initial">{{ patient.name.charAt(0) }}</div><div><strong>{{ patient.name }}</strong><span class="phone-value">{{ patient.phone }}</span></div></router-link></div><EmptyState v-else title="لا يوجد مرضى مرتبطون" description="لا توجد سجلات مرضى مرتبطة بهذه الجهة." /></section>
      <section class="surface-card"><div class="section-row"><h2 class="section-heading">كشف الجلسات والخدمات</h2><span class="caption-text">التاريخ والخدمات والقيمة دون بيانات الطبيب</span></div><div v-if="sponsor.sessions.length" class="table-scroll"><v-table><thead><tr><th>التاريخ</th><th>المريض</th><th>الخدمات</th><th>القيمة</th><th>الحالة</th></tr></thead><tbody><tr v-for="session in sponsor.sessions" :key="session.id"><td>{{ formatDate(session.opened_at) }}</td><td>{{ session.patient.name }}</td><td>{{ session.services.map((service) => service.name).join('، ') }}</td><td><MoneyText :value="session.net_total" /></td><td><StatusChip type="session" :value="session.status" /></td></tr></tbody></v-table></div><EmptyState v-else title="لا توجد جلسات" description="لم تُسجل جلسات لمرضى الجهة بعد." /></section>
      <section class="two-column"><div class="surface-card"><h2 class="section-heading">المستحقات</h2><div class="record-list"><div v-for="debt in sponsor.debts" :key="debt.id"><div><strong>دين #{{ debt.id }}</strong><span>{{ debt.kind === 'subscription' ? 'اشتراك' : debt.kind === 'opening' ? 'افتتاحي' : 'جلسة' }}</span></div><div class="record-amount"><MoneyText :value="debt.remaining" /><StatusChip type="debt" :value="debt.status" /></div></div></div></div><div class="surface-card"><h2 class="section-heading">المدفوعات المرتبطة</h2><div class="record-list"><div v-for="payment in sponsor.payments" :key="payment.id"><div><strong>{{ paymentLabel(payment.type) }}</strong><span>{{ formatDate(payment.created_at) }}</span></div><MoneyText :value="payment.amount" /></div></div></div></section>
    </template>
  </div>
  <DebtSettlementDialog v-if="sponsor" v-model="settlementDialog" :debts="sponsor.debts" @saved="load" />
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { getSponsor } from '@/api/sponsors'
import EmptyState from '@/components/common/EmptyState.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatCell from '@/components/common/StatCell.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import DebtSettlementDialog from '@/components/financial/DebtSettlementDialog.vue'

const route = useRoute(); const sponsor = ref(null); const loading = ref(true); const error = ref(''); const settlementDialog = ref(false)
const openDebts = computed(() => sponsor.value?.debts.filter((item) => ['open', 'partial'].includes(item.status)) || [])
onMounted(load)
async function load() { loading.value = true; error.value = ''; try { sponsor.value = (await getSponsor(route.params.id)).data } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
const formatDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(value)); const paymentLabel = (type) => ({ wallet_topup: 'شحن رصيد', session_payment: 'دفعة جلسة', subscription_payment: 'دفعة اشتراك', debt_payment: 'تسديد دين', refund: 'استرجاع' }[type] || type)
</script>

<style scoped>
.section-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; }.patient-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 12px; margin-top: 16px; }.patient-grid a { display: flex; align-items: center; gap: 12px; padding: 14px; color: var(--color-ink); border: 1px solid var(--color-hairline); border-radius: var(--radius-lg); }.patient-grid a:hover { border-color: var(--color-primary); background: var(--color-primary-subtle); }.patient-initial { width: 42px; height: 42px; display: grid; place-items: center; color: var(--color-primary); border-radius: var(--radius-md); background: var(--color-primary-subtle); border: 1px solid var(--color-primary-border); font-weight: 600; }.patient-grid strong, .patient-grid span { display: block; }.patient-grid strong { font-weight: 600; }.patient-grid span { color: var(--color-ink-muted); font-size: 13px; }.table-scroll { overflow-x: auto; margin-top: 16px; }.table-scroll :deep(table) { min-width: 780px; }.two-column { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 24px; }.record-list { display: grid; margin-top: 16px; }.record-list > div { min-height: 64px; display: flex; align-items: center; justify-content: space-between; gap: 12px; border-top: 1px solid var(--color-hairline); }.record-list > div:first-child { border-top: 0; }.record-list strong, .record-list span { display: block; }.record-list strong { font-weight: 600; }.record-list span { color: var(--color-ink-muted); font-size: 13px; }.record-amount { display: flex; align-items: center; gap: 8px; }
@media (max-width: 800px) { .two-column { grid-template-columns: 1fr; } }
</style>
