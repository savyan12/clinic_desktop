<template>
  <div class="page-container page-stack">
    <PageHeader title="الاشتراكات" subtitle="باقات جلسات بسعر إجمالي ثابت ورصيد مشترك للخدمات المغطاة" action-label="إضافة اشتراك" @action="dialog = true" />
    <div class="subscription-toolbar surface-card" data-form-flow-filter>
      <v-text-field v-model="filters.search" label="بحث بالمريض أو الهاتف" prepend-inner-icon="mdi-magnify" clearable @update:model-value="searchChanged" />
      <v-select v-model="filters.status" label="الحالة" :items="statusItems" clearable @update:model-value="statusChanged" />
    </div>
    <DataTableCard :headers="headers" :items="subscriptions" :loading="loading" :error="error" :meta="meta" :empty="{ icon: 'mdi-wallet-membership', title: 'لا توجد اشتراكات مطابقة', description: 'غيّر البحث أو الفلاتر، أو أنشئ اشتراكًا جديدًا.', actionLabel: 'إضافة اشتراك' }" @retry="load" @page="changePage" @empty-action="dialog = true">
      <template #rows="{ items }">
        <tr v-for="item in items" :key="item.id" class="clickable-row" @click="router.push(`/subscriptions/${item.id}`)">
          <td><strong>{{ item.patient.name }}</strong><span class="cell-subtitle phone-value">{{ item.patient.phone }}</span></td>
          <td><strong>{{ item.name }}</strong><span class="cell-subtitle">{{ item.services.map((service) => service.name).join('، ') }}</span></td>
          <td><strong>{{ item.remaining }} متبقية</strong><span class="cell-subtitle">{{ item.sessions_used }} من {{ item.sessions_count }} جلسة</span></td>
          <td>{{ formatDate(item.start_date) }}</td>
          <td><MoneyText :value="item.total_price" /></td><td><MoneyText :value="item.received_amount" /></td><td><MoneyText :value="item.outstanding_amount" /></td>
          <td><StatusChip type="subscription" :value="item.status" /></td>
          <td><v-btn :to="`/subscriptions/${item.id}`" variant="text">التفاصيل</v-btn></td>
        </tr>
      </template>
    </DataTableCard>
  </div>
  <CreateSubscriptionDialog v-model="dialog" @saved="load" />
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { listSubscriptions } from '@/api/subscriptions'
import DataTableCard from '@/components/common/DataTableCard.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import CreateSubscriptionDialog from '@/components/subscriptions/CreateSubscriptionDialog.vue'
import { useDebounce } from '@/composables/useDebounce'

const router = useRouter(); const debounce = useDebounce()
const subscriptions = ref([]); const meta = ref(null); const loading = ref(true); const error = ref(''); const dialog = ref(false)
const filters = reactive({ search: '', status: null, page: 1, per_page: 15 })
const statusItems = [{ title: 'نشط', value: 'active' }, { title: 'مكتمل', value: 'completed' }, { title: 'ملغي', value: 'cancelled' }]
const headers = [{ title: 'المريض', key: 'patient' }, { title: 'الاشتراك', key: 'subscription' }, { title: 'الجلسات', key: 'sessions' }, { title: 'البداية', key: 'start' }, { title: 'القيمة', key: 'value' }, { title: 'المحصل', key: 'paid' }, { title: 'المستحق', key: 'amount' }, { title: 'الحالة', key: 'status' }, { title: '', key: 'actions' }]
onMounted(load)
async function load() { loading.value = true; error.value = ''; try { const response = await listSubscriptions(filters); subscriptions.value = response.data; meta.value = response.meta } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
function searchChanged() { filters.page = 1; debounce(load) }
function statusChanged() { filters.page = 1; load() }
function changePage(page) { filters.page = page; load() }
const formatDate = (value) => value ? new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(`${value}T00:00:00`)) : 'غير محدد'
</script>

<style scoped>
.subscription-toolbar { display: grid; grid-template-columns: 2fr 1fr; gap: 16px; align-items: center; padding: 18px 20px; }.cell-subtitle { display: block; margin-top: 2px; color: var(--color-ink-muted); font-size: 12px; }
@media (max-width: 760px) { .subscription-toolbar { grid-template-columns: 1fr; } }
</style>
