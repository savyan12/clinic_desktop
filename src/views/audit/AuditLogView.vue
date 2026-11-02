<template>
  <div class="page-container page-stack">
    <PageHeader title="سجل التدقيق" subtitle="سجل للقراءة فقط يوضح من نفذ العملية وما الذي تغير" />
    <div class="filter-grid surface-card">
      <v-select v-model="filters.user_id" label="المستخدم" :items="users" item-title="name" item-value="id" clearable :loading="loadingUsers" @update:model-value="filterChanged" />
      <v-select v-model="filters.action" label="العملية" :items="actionItems" clearable @update:model-value="filterChanged" />
      <v-select v-model="filters.entity_type" label="نوع السجل" :items="entityItems" clearable @update:model-value="filterChanged" />
      <v-text-field v-model="filters.date" label="التاريخ" type="date" clearable @update:model-value="filterChanged" />
    </div>
    <InlineError v-if="userError" :message="userError" retry @retry="loadUsers" />
    <DataTableCard :headers="headers" :items="entries" :loading="loading" :error="error" :meta="meta" @retry="load" @page="changePage"><template #rows="{ items }"><tr v-for="entry in items" :key="entry.id" class="clickable-row" @click="openDetails(entry)"><td>{{ formatDate(entry.created_at) }}</td><td><strong>{{ entry.user?.name }}</strong></td><td>{{ labelFor('auditAction', entry.action) }}</td><td>{{ entityLabel(entry.entity_type) }} #{{ entry.entity_id }}</td><td>{{ entry.reason || '—' }}</td><td><v-btn variant="text" color="primary" @click.stop="openDetails(entry)">عرض التغيير</v-btn></td></tr></template></DataTableCard>
  </div>
  <v-navigation-drawer v-model="drawer" location="left" temporary :width="520" class="audit-drawer">
    <div v-if="selected" class="drawer-content"><div class="drawer-head"><div><h2>تفاصيل العملية</h2><span>{{ formatDate(selected.created_at) }}</span></div><v-btn icon="mdi-close" variant="text" @click="drawer = false" /></div><div class="audit-meta"><div><span>المستخدم</span><strong>{{ selected.user?.name }}</strong></div><div><span>العملية</span><strong>{{ labelFor('auditAction', selected.action) }}</strong></div><div><span>السجل</span><strong>{{ entityLabel(selected.entity_type) }} #{{ selected.entity_id }}</strong></div><div><span>السبب</span><strong>{{ selected.reason || 'غير مطلوب' }}</strong></div></div><section><h3>القيم السابقة</h3><pre>{{ pretty(selected.old_values) }}</pre></section><section><h3>القيم الجديدة</h3><pre>{{ pretty(selected.new_values) }}</pre></section><div class="read-only-note"><v-icon icon="mdi-lock-outline" /> سجل التدقيق غير قابل للتعديل أو الحذف.</div></div>
  </v-navigation-drawer>
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { listAuditLog } from '@/api/audit'
import { listUsers } from '@/api/users'
import DataTableCard from '@/components/common/DataTableCard.vue'
import InlineError from '@/components/common/InlineError.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import { labelFor } from '@/constants/labels'
import { useAuthStore } from '@/stores/auth'

const auth = useAuthStore()
const entries = ref([]); const meta = ref(null); const users = ref([]); const loading = ref(true); const error = ref(''); const drawer = ref(false); const selected = ref(null)
const loadingUsers = ref(false)
const userError = ref('')
const filters = reactive({ user_id: null, action: null, entity_type: null, date: '', page: 1 })
const actionItems = [{ title: 'إنشاء جلسة', value: 'session_created' }, { title: 'إغلاق جلسة', value: 'session_closed' }, { title: 'إلغاء جلسة', value: 'session_cancelled' }, { title: 'إضافة مريض', value: 'patient_created' }, { title: 'إنشاء اشتراك', value: 'subscription_created' }, { title: 'شحن رصيد', value: 'wallet_topped_up' }, { title: 'تسوية دين', value: 'debt_settled' }, { title: 'صرف طبيب', value: 'doctor_paid' }, { title: 'تعديل خدمة', value: 'service_updated' }]
const entityItems = [{ title: 'جلسة', value: 'session' }, { title: 'مريض', value: 'patient' }, { title: 'اشتراك', value: 'subscription' }, { title: 'خدمة', value: 'service' }, { title: 'طبيب', value: 'doctor' }, { title: 'مستخدم', value: 'user' }, { title: 'دفعة', value: 'payment' }]
const headers = [{ title: 'التاريخ والوقت', key: 'date' }, { title: 'المستخدم', key: 'user' }, { title: 'العملية', key: 'action' }, { title: 'السجل المتأثر', key: 'entity' }, { title: 'السبب', key: 'reason' }, { title: '', key: 'details' }]
onMounted(() => {
  load()
  loadUsers()
})
async function loadUsers() {
  loadingUsers.value = true
  userError.value = ''
  try {
    const response = await listUsers({ per_page: 100 })
    users.value = [auth.user, ...response.data].filter((user, index, items) => user && items.findIndex((item) => item.id === user.id) === index)
  } catch (cause) {
    userError.value = cause.message
  } finally {
    loadingUsers.value = false
  }
}
async function load() { loading.value = true; error.value = ''; try { const response = await listAuditLog(filters); entries.value = response.data; meta.value = response.meta } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
function filterChanged() { filters.page = 1; load() }
function changePage(page) { filters.page = page; load() } function openDetails(entry) { selected.value = entry; drawer.value = true }
const formatDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value)); const entityLabel = (value) => ({ session: 'جلسة', patient: 'مريض', subscription: 'اشتراك', service: 'خدمة', doctor: 'طبيب', user: 'مستخدم', payment: 'دفعة', debt: 'دين', settings: 'إعدادات' }[value] || value); const pretty = (value) => value ? JSON.stringify(value, null, 2) : 'لا توجد قيم'
</script>

<style scoped>
.audit-drawer { border-right: 1px solid var(--color-hairline) !important; background: var(--color-canvas) !important; }.drawer-content { display: grid; gap: 22px; padding: 24px; }.drawer-head { display: flex; justify-content: space-between; align-items: flex-start; padding-bottom: 18px; border-bottom: 1px solid var(--color-hairline); }.drawer-head h2 { margin: 0; font-size: 24px; }.drawer-head span { color: var(--color-ink-muted); font-size: 13px; }.audit-meta { display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px; }.audit-meta div { padding: 12px; border-radius: 10px; background: var(--color-pearl); }.audit-meta span, .audit-meta strong { display: block; }.audit-meta span { color: var(--color-ink-muted); font-size: 12px; }.audit-meta strong { margin-top: 3px; font-size: 14px; font-weight: 600; }.drawer-content h3 { margin: 0 0 8px; font-size: 16px; }.drawer-content pre { direction: ltr; overflow: auto; margin: 0; padding: 14px; color: var(--color-ink); border: 1px solid var(--color-hairline); border-radius: 10px; background: var(--color-parchment); font-family: ui-monospace, monospace; font-size: 12px; text-align: left; }.read-only-note { display: flex; align-items: center; gap: 8px; padding: 12px; color: var(--color-ink-muted); border-radius: 10px; background: var(--color-pearl); }
</style>
