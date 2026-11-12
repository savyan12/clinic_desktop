<template>
  <div class="page-container page-stack">
    <PageHeader :title="auth.isAdmin ? 'الجلسات' : 'جلسات اليوم'" :subtitle="auth.isAdmin ? 'جميع الجلسات المسجلة مع البحث حسب التاريخ' : 'يعرض هذا القسم جلسات اليوم فقط؛ السجل السابق متاح من ملف المريض'" action-label="تسجيل جلسة جديدة" @action="sessionDialog = true" />
    <div class="filter-grid surface-card" data-form-flow-filter>
      <v-text-field v-model="filters.search" label="بحث" placeholder="المريض أو الهاتف أو رقم الجلسة" prepend-inner-icon="mdi-magnify" clearable @update:model-value="searchChanged" />
      <v-select v-model="filters.doctor_id" label="الطبيب" :items="doctors" item-title="name" item-value="id" clearable :loading="loadingDoctors" @update:model-value="filterChanged" />
      <v-select v-model="filters.status" label="الحالة" :items="statusItems" clearable @update:model-value="filterChanged" />
      <v-text-field v-if="auth.isAdmin" v-model="filters.date" label="التاريخ" type="date" clearable @update:model-value="filterChanged" />
    </div>
    <InlineError v-if="doctorError" :message="doctorError" retry @retry="loadDoctors" />
    <DataTableCard :headers="headers" :items="sessions" :loading="loading" :error="error" :meta="meta" :empty="{ icon: 'mdi-clipboard-search-outline', title: 'لا توجد جلسات مطابقة', description: 'غيّر البحث أو الفلاتر، أو افتح جلسة جديدة.' }" @retry="load" @page="changePage" @empty-action="sessionDialog = true">
      <template #rows="{ items }">
        <tr v-for="session in items" :key="session.id">
          <td><router-link class="record-link ltr-value" :to="`/sessions/${session.id}`">{{ session.code }}</router-link></td>
          <td><strong>{{ session.patient.name }}</strong><span class="cell-subtitle phone-value">{{ session.patient.phone }}</span></td>
          <td>{{ formatDate(session.opened_at) }}</td>
          <td>{{ session.doctor.name }}</td>
          <td><span class="services-cell">{{ session.services.map((item) => item.name).join('، ') }}</span></td>
          <td><StatusChip type="coverage" :value="session.settlement.coverage" /></td>
          <td><MoneyText :value="session.net_total" /></td>
          <td><StatusChip type="session" :value="session.status" /></td>
          <td @click.stop><v-btn :to="`/sessions/${session.id}`" variant="text" color="primary">التفاصيل</v-btn></td>
        </tr>
      </template>
    </DataTableCard>
  </div>
  <NewSessionDialog v-model="sessionDialog" @saved="load" />
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { getDoctorOptions } from '@/api/doctors'
import { listSessions } from '@/api/sessions'
import DataTableCard from '@/components/common/DataTableCard.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import NewSessionDialog from '@/components/sessions/NewSessionDialog.vue'
import { useDebounce } from '@/composables/useDebounce'
import { useAuthStore } from '@/stores/auth'

const auth = useAuthStore()
const debounce = useDebounce()
const sessions = ref([])
const doctors = ref([])
const loadingDoctors = ref(false)
const doctorError = ref('')
const meta = ref(null)
const loading = ref(true)
const error = ref('')
const sessionDialog = ref(false)
const filters = reactive({ search: '', doctor_id: null, status: null, date: '', page: 1, per_page: 15 })
const statusItems = [{ title: 'مفتوحة', value: 'open' }, { title: 'مغلقة', value: 'closed' }]
const headers = [
  { title: 'رقم الجلسة', key: 'code' }, { title: 'المريض', key: 'patient' }, { title: 'التاريخ والوقت', key: 'date' }, { title: 'الطبيب', key: 'doctor' },
  { title: 'الخدمات', key: 'services' }, { title: 'التغطية / الدفع', key: 'coverage' }, { title: 'الصافي', key: 'net' }, { title: 'الحالة', key: 'status' }, { title: 'الإجراءات', key: 'actions' },
]

onMounted(() => {
  load()
  loadDoctors()
})
async function loadDoctors() {
  loadingDoctors.value = true
  doctorError.value = ''
  try { doctors.value = await getDoctorOptions() } catch (cause) { doctorError.value = cause.message } finally { loadingDoctors.value = false }
}
let loadVersion = 0
async function load() {
  const version = ++loadVersion
  loading.value = true
  error.value = ''
  try {
    const response = await listSessions({ ...filters })
    if (version === loadVersion) { sessions.value = response.data; meta.value = response.meta }
  } catch (cause) {
    if (version === loadVersion) error.value = cause.message
  } finally {
    if (version === loadVersion) loading.value = false
  }
}
function searchChanged() { filters.page = 1; debounce(load) }
function filterChanged() { filters.page = 1; load() }
function changePage(page) { filters.page = page; load() }
const formatDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' }).format(new Date(value))
</script>

<style scoped>
.cell-subtitle { display: block; margin-top: 2px; color: var(--color-ink-muted); font-size: 13px; }
.services-cell { display: inline-block; max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
