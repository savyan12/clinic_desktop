<template>
  <div class="page-container page-stack">
    <PageHeader title="سجل التدقيق" subtitle="سجل للقراءة فقط يوضح من نفذ العملية وما الذي تغير" />

    <div class="filter-grid surface-card" data-form-flow-filter>
      <v-select
        v-model="filters.user_id"
        label="المستخدم"
        :items="users"
        item-title="name"
        item-value="id"
        clearable
        :loading="loadingUsers"
        @update:model-value="filterChanged"
      />
      <v-select
        v-model="filters.action"
        label="العملية"
        :items="auditActionItems"
        clearable
        @update:model-value="filterChanged"
      />
      <v-select
        v-model="filters.entity_type"
        label="نوع السجل"
        :items="auditEntityItems"
        clearable
        @update:model-value="filterChanged"
      />
      <v-text-field
        v-model="filters.date"
        label="التاريخ"
        type="date"
        clearable
        @update:model-value="filterChanged"
      />
    </div>

    <InlineError v-if="userError" :message="userError" retry @retry="loadUsers" />

    <DataTableCard
      :headers="headers"
      :items="entries"
      :loading="loading"
      :error="error"
      :meta="meta"
      @retry="load"
      @page="changePage"
    >
      <template #rows="{ items }">
        <tr v-for="entry in items" :key="entry.id">
          <td>{{ formatAuditTimestamp(entry.created_at) }}</td>
          <td><strong>{{ auditUserLabel(entry) }}</strong></td>
          <td>{{ auditActionLabel(entry.action) }}</td>
          <td>{{ auditRecordLabel(entry.entity_type, entry.entity_id) }}</td>
          <td>{{ formatAuditReason(entry.reason) }}</td>
          <td>
            <v-btn variant="text" color="primary" @click="openDetails(entry)">
              عرض التغيير
            </v-btn>
          </td>
        </tr>
      </template>
    </DataTableCard>
  </div>

  <v-navigation-drawer
    v-model="drawer"
    location="left"
    temporary
    :width="680"
    class="audit-drawer"
  >
    <div v-if="selected" class="drawer-content" dir="rtl">
      <div class="drawer-head">
        <div>
          <h2>تفاصيل العملية</h2>
          <span>{{ formatAuditTimestamp(selected.created_at) }}</span>
        </div>
        <v-btn icon="mdi-close" variant="text" aria-label="إغلاق التفاصيل" @click="drawer = false" />
      </div>

      <div class="audit-meta">
        <div>
          <span>المستخدم</span>
          <strong>{{ auditUserLabel(selected) }}</strong>
        </div>
        <div>
          <span>العملية</span>
          <strong>{{ auditActionLabel(selected.action) }}</strong>
        </div>
        <div>
          <span>السجل</span>
          <strong>{{ auditRecordLabel(selected.entity_type, selected.entity_id) }}</strong>
        </div>
        <div class="reason-meta">
          <span>السبب</span>
          <strong>{{ formatAuditReason(selected.reason) }}</strong>
        </div>
      </div>

      <section class="changes-section">
        <div class="section-heading">
          <h3>التغييرات</h3>
          <span>{{ selectedChanges.rows.length }} حقل</span>
        </div>

        <div v-if="!selectedChanges.rows.length" class="no-changes">
          <v-icon icon="mdi-file-check-outline" />
          <span>لا توجد قيم متغيرة مسجلة لهذه العملية.</span>
        </div>

        <div v-else class="changes-table-wrap">
          <table class="audit-changes-table">
            <thead>
              <tr>
                <th scope="col">الحقل</th>
                <th v-if="selectedChanges.showOld" scope="col">القيمة السابقة</th>
                <th v-if="selectedChanges.showNew" scope="col">القيمة الجديدة</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="change in selectedChanges.rows" :key="change.key">
                <th scope="row">{{ change.label }}</th>
                <td v-if="selectedChanges.showOld">
                  <span class="change-value old-value" dir="auto">{{ change.oldValue }}</span>
                </td>
                <td v-if="selectedChanges.showNew">
                  <span class="change-value new-value" dir="auto">{{ change.newValue }}</span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <div class="read-only-note">
        <v-icon icon="mdi-lock-outline" />
        سجل التدقيق غير قابل للتعديل أو الحذف.
      </div>
    </div>
  </v-navigation-drawer>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { listAuditLog } from '@/api/audit'
import { listUsers } from '@/api/users'
import DataTableCard from '@/components/common/DataTableCard.vue'
import InlineError from '@/components/common/InlineError.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import {
  auditActionItems,
  auditActionLabel,
  auditEntityItems,
  auditRecordLabel,
  buildAuditChanges,
  formatAuditReason,
  formatAuditTimestamp,
} from '@/constants/audit'
import { useAuthStore } from '@/stores/auth'

const auth = useAuthStore()
const entries = ref([])
const meta = ref(null)
const users = ref([])
const loading = ref(true)
const error = ref('')
const drawer = ref(false)
const selected = ref(null)
const loadingUsers = ref(false)
const userError = ref('')
const filters = reactive({ user_id: null, action: null, entity_type: null, date: '', page: 1 })
const headers = [
  { title: 'التاريخ والوقت', key: 'date' },
  { title: 'المستخدم', key: 'user' },
  { title: 'العملية', key: 'action' },
  { title: 'السجل المتأثر', key: 'entity' },
  { title: 'السبب', key: 'reason' },
  { title: 'التفاصيل', key: 'details' },
]
const selectedChanges = computed(() => buildAuditChanges(selected.value || {}))

onMounted(() => {
  load()
  loadUsers()
})

async function loadUsers() {
  loadingUsers.value = true
  userError.value = ''
  try {
    const response = await listUsers({ per_page: 100 })
    users.value = [auth.user, ...response.data].filter(
      (user, index, items) => user && items.findIndex((item) => item.id === user.id) === index,
    )
  } catch (cause) {
    userError.value = cause.message
  } finally {
    loadingUsers.value = false
  }
}

async function load() {
  loading.value = true
  error.value = ''
  try {
    const response = await listAuditLog(filters)
    entries.value = response.data
    meta.value = response.meta
  } catch (apiError) {
    error.value = apiError.message
  } finally {
    loading.value = false
  }
}

function filterChanged() {
  filters.page = 1
  load()
}

function changePage(page) {
  filters.page = page
  load()
}

function openDetails(entry) {
  selected.value = entry
  drawer.value = true
}

function auditUserLabel(entry) {
  return entry.user?.name || 'مستخدم غير معروف'
}
</script>

<style scoped>
.audit-drawer {
  width: min(680px, 100vw) !important;
  border-right: 1px solid var(--color-hairline) !important;
  background: var(--color-canvas) !important;
}

.drawer-content {
  height: 100%;
  min-width: 0;
  display: grid;
  align-content: start;
  gap: 22px;
  overflow-y: auto;
  padding: 24px;
  scrollbar-gutter: stable;
}

.drawer-head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
  padding-bottom: 18px;
  border-bottom: 1px solid var(--color-hairline);
}

.drawer-head h2 {
  margin: 0;
  font-size: 24px;
}

.drawer-head span {
  color: var(--color-ink-muted);
  font-size: 13px;
}

.audit-meta {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.audit-meta > div {
  min-width: 0;
  padding: 12px;
  border-radius: 10px;
  background: var(--color-pearl);
}

.audit-meta span,
.audit-meta strong {
  display: block;
}

.audit-meta span {
  color: var(--color-ink-muted);
  font-size: 12px;
}

.audit-meta strong {
  margin-top: 3px;
  font-size: 14px;
  font-weight: 600;
  overflow-wrap: anywhere;
}

.reason-meta {
  grid-column: 1 / -1;
}

.changes-section {
  min-width: 0;
}

.section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.section-heading h3 {
  margin: 0;
  font-size: 16px;
}

.section-heading span {
  color: var(--color-ink-muted);
  font-size: 13px;
}

.changes-table-wrap {
  max-height: min(52vh, 520px);
  overflow: auto;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-lg);
  overscroll-behavior: contain;
}

.audit-changes-table {
  width: 100%;
  min-width: 560px;
  border-collapse: separate;
  border-spacing: 0;
  table-layout: fixed;
  text-align: right;
}

.audit-changes-table th,
.audit-changes-table td {
  padding: 12px;
  vertical-align: top;
  border-bottom: 1px solid var(--color-hairline);
  overflow-wrap: anywhere;
}

.audit-changes-table thead th {
  position: sticky;
  top: 0;
  z-index: 1;
  color: var(--color-ink-muted);
  background: var(--color-parchment);
  font-size: 12px;
  font-weight: 600;
}

.audit-changes-table tbody th {
  width: 34%;
  color: var(--color-ink);
  background: var(--color-pearl);
  font-size: 13px;
  font-weight: 600;
}

.audit-changes-table tr:last-child th,
.audit-changes-table tr:last-child td {
  border-bottom: 0;
}

.change-value {
  min-height: 34px;
  display: block;
  padding: 8px 10px;
  border-radius: 8px;
  white-space: pre-wrap;
}

.old-value {
  color: var(--color-danger);
  background: var(--color-danger-bg);
}

.new-value {
  color: var(--color-success);
  background: var(--color-success-bg);
}

.no-changes,
.read-only-note {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px;
  color: var(--color-ink-muted);
  border-radius: 10px;
  background: var(--color-pearl);
}

@media (max-width: 640px) {
  .drawer-content {
    padding: 18px;
  }

  .audit-meta {
    grid-template-columns: 1fr;
  }

  .reason-meta {
    grid-column: auto;
  }
}
</style>
