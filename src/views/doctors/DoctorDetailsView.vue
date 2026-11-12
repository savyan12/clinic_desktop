<template>
  <div class="page-container page-stack">
    <template v-if="loading"><v-skeleton-loader type="heading, image, table" /></template>
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="doctor">
      <PageHeader :title="doctor.name" :subtitle="`نسبة المستحق الحالية ${Number(doctor.commission_rate).toFixed(0)}% من صافي الجلسة`"><template #actions><v-btn color="primary" prepend-icon="mdi-cash-plus" :disabled="Number(doctor.remaining) <= 0" @click="payoutDialog = true">صرف مستحقات</v-btn></template></PageHeader>
      <section class="metric-grid"><StatCell label="عدد الجلسات" :value="doctor.sessions_count" caption="الجلسات المغلقة" /><StatCell label="المستحقات المكتسبة" :value="doctor.earned" money caption="تستحق عند إغلاق الجلسة" /><StatCell label="المبالغ المصروفة" :value="doctor.paid" money caption="دفعات مسجلة" /><StatCell label="المستحق المتبقي" :value="doctor.remaining" money caption="متاح للصرف" /></section>
      <section class="surface-card"><div class="section-row"><h2 class="section-heading">الجلسات المحتسبة</h2><span class="caption-text">تُحتسب النسبة من المبلغ المحصل، وتضاف حصة الدين عند تسديده</span></div><div v-if="doctor.sessions.length" class="table-scroll"><v-table><thead><tr><th>الجلسة</th><th>التاريخ</th><th>المريض</th><th>قيمة الجلسة</th><th>النسبة</th><th>المستحق</th></tr></thead><tbody><tr v-for="session in doctor.sessions" :key="session.id"><td><router-link :to="`/sessions/${session.id}`">{{ session.code }}</router-link></td><td>{{ formatDate(session.closed_at) }}</td><td>{{ session.patient.name }}</td><td><MoneyText :value="session.net_total" /></td><td>{{ session.commission_rate }}%</td><td><MoneyText :value="session.commission_amount" /></td></tr></tbody></v-table></div><EmptyState v-else title="لا توجد جلسات محتسبة" description="تظهر الجلسات هنا بعد تحصيل قيمتها أو تسديد دينها." /></section>
      <section class="surface-card"><h2 class="section-heading">دفعات الطبيب</h2><div v-if="doctor.payouts.length" class="payout-list"><div v-for="payout in doctor.payouts" :key="payout.id"><div><strong>{{ formatDateOnly(payout.date) }}</strong><span>{{ labelFor('paymentMethod', payout.method) }} · {{ payout.note || 'دون ملاحظة' }}</span></div><MoneyText :value="payout.amount" /></div></div><EmptyState v-else title="لا توجد دفعات" description="لم تُسجل دفعات لهذا الطبيب بعد." /></section>
    </template>
  </div>
  <PayoutDialog v-if="doctor" v-model="payoutDialog" :doctor="doctor" @saved="load" />
</template>

<script setup>
import { onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { getDoctor } from '@/api/doctors'
import EmptyState from '@/components/common/EmptyState.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatCell from '@/components/common/StatCell.vue'
import PayoutDialog from '@/components/financial/PayoutDialog.vue'
import { labelFor } from '@/constants/labels'

const route = useRoute(); const doctor = ref(null); const loading = ref(true); const error = ref(''); const payoutDialog = ref(false)
onMounted(load)
async function load() { loading.value = true; error.value = ''; try { doctor.value = (await getDoctor(route.params.id)).data } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
const formatDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value)); const formatDateOnly = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(`${value}T00:00:00`))
</script>

<style scoped>
.section-row { display: flex; justify-content: space-between; align-items: center; gap: 16px; }.table-scroll { overflow-x: auto; margin-top: 16px; }.table-scroll :deep(table) { min-width: 760px; }.payout-list { display: grid; margin-top: 16px; }.payout-list > div { min-height: 64px; display: flex; align-items: center; justify-content: space-between; border-top: 1px solid var(--color-hairline); }.payout-list > div:first-child { border-top: 0; }.payout-list strong, .payout-list span { display: block; }.payout-list strong { font-weight: 600; }.payout-list span { color: var(--color-ink-muted); font-size: 13px; }
</style>

