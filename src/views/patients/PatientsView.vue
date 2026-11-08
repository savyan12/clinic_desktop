<template>
  <div class="page-container page-stack">
    <PageHeader title="المرضى" action-label="إضافة مريض" action-icon="mdi-account-plus-outline" @action="patientDialog = true" />
    <DataTableCard :headers="headers" :items="patients" :loading="loading" :error="error" :meta="meta" :empty="{ icon: 'mdi-account-search-outline', title: 'لا توجد سجلات مطابقة', description: 'جرّب بحثًا آخر أو أضف المريض إذا لم يكن مسجلًا.' }" @retry="load" @page="changePage">
      <template #header>
        <div class="patient-toolbar" data-form-flow-filter>
          <div class="table-title"><h2>سجل المرضى</h2><span>{{ meta?.total || 0 }} سجل</span></div>
          <v-text-field v-model="filters.search" label="بحث" placeholder="الاسم أو رقم الهاتف" prepend-inner-icon="mdi-magnify" clearable @update:model-value="searchChanged" />
          <v-select v-model="filters.subscription_status" label="حالة الاشتراك" :items="subscriptionFilters" clearable @update:model-value="filterChanged" />
          <v-btn v-if="filters.search || filters.subscription_status" variant="text" size="small" @click="clearFilters">مسح</v-btn>
        </div>
      </template>
      <template #rows="{ items }">
        <tr v-for="patient in items" :key="patient.id">
          <td><div class="person-cell"><span class="person-avatar" aria-hidden="true">{{ patient.name.charAt(0) }}</span><span><router-link class="record-link" :to="`/patients/${patient.id}`">{{ patient.name }}</router-link><small class="phone-value">{{ patient.phone }}</small></span></div></td>
          <td><strong>{{ labelFor('gender', patient.gender) }}</strong><small class="secondary-line">{{ patient.area || 'لم تحدد المنطقة' }}</small></td>
          <td><StatusChip v-if="patient.subscription_status" type="subscription" :value="patient.subscription_status" /><span v-else class="muted-text">لا يوجد اشتراك</span></td>
          <td>{{ patient.sponsor?.name || '—' }}</td>
          <td><v-btn :to="`/patients/${patient.id}`" icon="mdi-arrow-left" variant="text" aria-label="فتح ملف المريض" /></td>
        </tr>
      </template>
      <template #cards="{ items }">
        <router-link v-for="patient in items" :key="patient.id" :to="`/patients/${patient.id}`" class="patient-mobile-card">
          <div class="person-cell"><span class="person-avatar" aria-hidden="true">{{ patient.name.charAt(0) }}</span><span><strong>{{ patient.name }}</strong><small class="phone-value">{{ patient.phone }}</small></span></div>
          <StatusChip v-if="patient.subscription_status" type="subscription" :value="patient.subscription_status" />
          <span>{{ patient.sponsor?.name || 'بدون جهة راعية' }}</span>
          <v-icon icon="mdi-arrow-left" size="18" aria-hidden="true" />
        </router-link>
      </template>
    </DataTableCard>
  </div>
  <AddPatientDialog v-model="patientDialog" @saved="load" />
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { listPatients } from '@/api/patients'
import DataTableCard from '@/components/common/DataTableCard.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import AddPatientDialog from '@/components/patients/AddPatientDialog.vue'
import { labelFor } from '@/constants/labels'
import { useDebounce } from '@/composables/useDebounce'

const debounce = useDebounce()
const patients = ref([])
const meta = ref(null)
const loading = ref(true)
const error = ref('')
const patientDialog = ref(false)
const filters = reactive({ search: '', subscription_status: null, page: 1, per_page: 15 })
const subscriptionFilters = [{ title: 'اشتراك نشط', value: 'active' }, { title: 'بدون اشتراك', value: 'none' }]
const headers = [{ title: 'المريض', key: 'name' }, { title: 'البيانات', key: 'details' }, { title: 'الاشتراك', key: 'subscription' }, { title: 'الجهة الراعية', key: 'sponsor' }, { title: '', key: 'actions' }]

onMounted(load)
async function load() {
  loading.value = true
  error.value = ''
  try { const response = await listPatients(filters); patients.value = response.data; meta.value = response.meta } catch (apiError) { error.value = apiError.message } finally { loading.value = false }
}
function searchChanged() { filters.page = 1; debounce(load) }
function filterChanged() { filters.page = 1; load() }
function changePage(page) { filters.page = page; load() }
function clearFilters() { filters.search = ''; filters.subscription_status = null; filters.page = 1; load() }
</script>

<style scoped>
.patient-toolbar { display: grid; grid-template-columns: minmax(180px, 1fr) minmax(240px, 1.4fr) 190px auto; gap: 12px; align-items: start; }
.table-title h2 { margin: 0; font-size: 17px; font-weight: 600; }
.table-title span { color: var(--text-muted); font-size: 12px; }
.person-cell { min-width: 0; display: flex; align-items: center; gap: 10px; }
.person-cell > span:last-child { min-width: 0; }
.person-cell strong, .person-cell small { display: block; }
.person-avatar { width: 38px; height: 38px; display: grid; flex: 0 0 38px; place-items: center; color: var(--accent); background: var(--accent-tint); border-radius: 50%; font-weight: 600; }
.person-cell small, .secondary-line { display: block; margin-top: 2px; color: var(--text-muted); font-size: 12px; }
.patient-mobile-card { min-width: 0; display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 14px; color: var(--text); background: var(--surface-subtle); border: 1px solid var(--border); border-radius: 14px; }
.patient-mobile-card > span:nth-child(3) { color: var(--text-muted); font-size: 12px; }
@media (max-width: 1000px) { .patient-toolbar { grid-template-columns: 1fr 1fr; } }
@media (max-width: 700px) { .patient-toolbar { grid-template-columns: 1fr; } .table-title { display: none; } }
</style>
