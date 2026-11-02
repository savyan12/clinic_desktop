<template>
  <div class="print-sheet"><v-skeleton-loader v-if="loading" type="article, table" /><InlineError v-else-if="error" :message="error" /><template v-else-if="sponsor"><div class="document-title"><h2>كشف حساب جهة راعية</h2><span>أُعد في {{ generatedAt }}</span></div><div class="print-meta"><div><span>الجهة</span><strong>{{ sponsor.name }}</strong></div><div><span>الهاتف</span><strong>{{ sponsor.phone }}</strong></div><div><span>العنوان</span><strong>{{ sponsor.address }}</strong></div></div><div class="summary-row"><span>إجمالي المستحق <MoneyText :value="sponsor.total_due" /></span><span>المدفوع <MoneyText :value="sponsor.total_paid" /></span><span>المتبقي <MoneyText :value="sponsor.remaining" /></span></div><v-table><thead><tr><th>التاريخ</th><th>المريض</th><th>الخدمات</th><th>القيمة</th></tr></thead><tbody><tr v-for="session in sponsor.sessions" :key="session.id"><td>{{ formatDate(session.opened_at) }}</td><td>{{ session.patient.name }}</td><td>{{ session.services.map((item) => item.name).join('، ') }}</td><td><MoneyText :value="session.net_total" /></td></tr></tbody></v-table><h3>تفاصيل المستحقات</h3><v-table><thead><tr><th>المرجع</th><th>الأصل</th><th>المسدد</th><th>المتبقي</th><th>الحالة</th></tr></thead><tbody><tr v-for="debt in sponsor.debts" :key="debt.id"><td>#{{ debt.id }}</td><td><MoneyText :value="debt.amount" /></td><td><MoneyText :value="debt.paid_amount" /></td><td><MoneyText :value="debt.remaining" /></td><td><StatusChip type="debt" :value="debt.status" /></td></tr></tbody></v-table></template></div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'; import { useRoute } from 'vue-router'; import { getSponsor, getSponsorStatement } from '@/api/sponsors'; import InlineError from '@/components/common/InlineError.vue'; import MoneyText from '@/components/common/MoneyText.vue'; import StatusChip from '@/components/common/StatusChip.vue'
const route = useRoute(); const sponsor = ref(null); const loading = ref(true); const error = ref(''); const generatedAt = computed(() => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'long', timeStyle: 'short' }).format(new Date())); onMounted(async () => {
  try {
    const [detailsResponse, statementResponse] = await Promise.all([
      getSponsor(route.params.id),
      getSponsorStatement(route.params.id, route.query),
    ])
    sponsor.value = {
      ...detailsResponse.data,
      ...statementResponse.data.sponsor,
      patients: statementResponse.data.patients,
      sessions: statementResponse.data.sessions,
      debts: statementResponse.data.debts,
      payments: statementResponse.data.payments,
      allocations: statementResponse.data.allocations,
      total_due: statementResponse.data.totals.due,
      total_paid: statementResponse.data.totals.paid,
      remaining: statementResponse.data.totals.remaining,
    }
  } catch (apiError) {
    error.value = apiError.message
  } finally {
    loading.value = false
  }
}); const formatDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(value))
</script>

<style scoped>
.print-sheet { display: grid; gap: 20px; padding: 24px 0; }.document-title { display: flex; justify-content: space-between; }.document-title h2 { margin: 0; }.document-title span { color: var(--color-ink-muted); }.print-meta { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }.print-meta div { padding: 12px; border: 1px solid var(--color-hairline); border-radius: 10px; }.print-meta span, .print-meta strong { display: block; }.print-meta span { color: var(--color-ink-muted); font-size: 12px; }.summary-row { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }.summary-row span { display: grid; gap: 4px; padding: 14px; border-radius: 10px; background: var(--color-pearl); }.summary-row :deep(.money-text) { font-size: 20px; font-weight: 600; }.print-sheet h3 { margin-bottom: -8px; }
</style>
