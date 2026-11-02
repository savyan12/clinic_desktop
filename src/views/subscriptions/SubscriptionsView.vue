<template>
  <div class="page-container page-stack">
    <PageHeader title="الاشتراكات" subtitle="اشتراكات شهرية بقيمة ثابتة ومدة بداية ونهاية" action-label="إضافة اشتراك شهري" @action="dialog = true" />
    <div class="subscription-toolbar surface-card">
      <v-text-field v-model="filters.search" label="بحث بالمريض أو الهاتف" prepend-inner-icon="mdi-magnify" clearable @update:model-value="searchChanged" />
      <v-select v-model="filters.status" label="الحالة" :items="statusItems" clearable @update:model-value="statusChanged" />
      <v-checkbox v-model="nearEnd" label="قريبة من الانتهاء" color="primary" hide-details @update:model-value="nearChanged" />
    </div>
    <DataTableCard :headers="headers" :items="subscriptions" :loading="loading" :error="error" :meta="meta" :empty="{ icon: 'mdi-wallet-membership', title: 'لا توجد اشتراكات مطابقة', description: 'غيّر البحث أو الفلاتر، أو أنشئ اشتراكًا جديدًا.', actionLabel: 'إضافة اشتراك' }" @retry="load" @page="changePage" @empty-action="dialog = true">
      <template #rows="{ items }">
        <tr v-for="item in items" :key="item.id" class="clickable-row" @click="router.push(`/subscriptions/${item.id}`)">
          <td><strong>{{ item.patient.name }}</strong><span class="cell-subtitle ltr-value">{{ item.patient.phone }}</span></td>
          <td><strong>{{ item.plan_type === 'monthly' ? 'شهري' : 'باقة قديمة' }}</strong><span v-if="item.plan_type !== 'monthly'" class="cell-subtitle">{{ item.sessions_used }} من {{ item.sessions_count }} جلسة</span></td>
          <td>{{ formatDate(item.start_date) }}<span class="cell-subtitle">إلى {{ formatDate(item.end_date || item.expected_end_date) }}</span></td>
          <td><MoneyText :value="item.net_total" /></td><td><MoneyText :value="item.paid_amount" /></td><td><MoneyText :value="item.remaining_amount" /></td>
          <td><StatusChip type="subscription" :value="item.status" /><span v-if="item.plan_type === 'monthly' && item.status === 'active'" class="cell-subtitle" :class="{ near: item.remaining_days <= 7 }">متبقي {{ item.remaining_days }} يوم</span></td>
          <td><v-btn :to="`/subscriptions/${item.id}`" variant="text">التفاصيل</v-btn></td>
        </tr>
      </template>
    </DataTableCard>
  </div>
  <CreateSubscriptionDialog v-model="dialog" @saved="load" />
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { listSubscriptions } from '@/api/subscriptions'
import DataTableCard from '@/components/common/DataTableCard.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import CreateSubscriptionDialog from '@/components/subscriptions/CreateSubscriptionDialog.vue'
import { useDebounce } from '@/composables/useDebounce'

const route = useRoute(); const router = useRouter(); const debounce = useDebounce()
const subscriptions = ref([]); const meta = ref(null); const loading = ref(true); const error = ref(''); const dialog = ref(false)
const nearEnd = ref(route.query.near_end === '1')
const filters = reactive({ search: '', status: null, near_end: nearEnd.value ? 1 : null, page: 1, per_page: 15 })
const statusItems = [{ title: 'نشط', value: 'active' }, { title: 'مكتمل', value: 'completed' }, { title: 'موقوف', value: 'stopped' }, { title: 'ملغي', value: 'cancelled' }]
const headers = [{ title: 'المريض', key: 'patient' }, { title: 'النوع', key: 'type' }, { title: 'المدة', key: 'period' }, { title: 'القيمة', key: 'value' }, { title: 'المدفوع', key: 'paid' }, { title: 'المتبقي المالي', key: 'amount' }, { title: 'الحالة', key: 'status' }, { title: '', key: 'actions' }]
onMounted(load)
async function load() { loading.value = true; error.value = ''; try { const response = await listSubscriptions(filters); subscriptions.value = response.data; meta.value = response.meta } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
function searchChanged() { filters.page = 1; debounce(load) }
function statusChanged() { filters.page = 1; load() }
function nearChanged(value) { filters.near_end = value ? 1 : null; filters.page = 1; load() }
function changePage(page) { filters.page = page; load() }
const formatDate = (value) => value ? new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(`${value}T00:00:00`)) : 'غير محدد'
</script>

<style scoped>
.subscription-toolbar { display: grid; grid-template-columns: 2fr 1fr auto; gap: 16px; align-items: center; padding: 18px 20px; }.cell-subtitle { display: block; margin-top: 2px; color: var(--color-ink-muted); font-size: 12px; }.near { color: var(--color-warning); font-weight: 600; }
@media (max-width: 760px) { .subscription-toolbar { grid-template-columns: 1fr; } }
</style>
