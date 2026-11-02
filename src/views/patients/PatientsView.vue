<template>
  <div class="page-container page-stack">
    <PageHeader title="المرضى" subtitle="البحث في السجلات والوصول إلى الملف الموحد" action-label="إضافة مريض" action-icon="mdi-account-plus-outline" @action="patientDialog = true" />
    <div class="patient-toolbar surface-card">
      <v-text-field v-model="filters.search" label="بحث بالاسم أو رقم الهاتف" prepend-inner-icon="mdi-magnify" clearable @update:model-value="searchChanged" />
      <v-select v-model="filters.subscription_status" label="حالة الاشتراك" :items="subscriptionFilters" clearable @update:model-value="filterChanged" />
    </div>
    <DataTableCard :headers="headers" :items="patients" :loading="loading" :error="error" :meta="meta" :empty="{ icon: 'mdi-account-search-outline', title: 'لا توجد سجلات مطابقة', description: 'جرّب بحثًا آخر أو أضف المريض إذا لم يكن مسجلًا.', actionLabel: 'إضافة مريض' }" @retry="load" @page="changePage" @empty-action="patientDialog = true">
      <template #rows="{ items }">
        <tr v-for="patient in items" :key="patient.id" class="clickable-row" @click="router.push(`/patients/${patient.id}`)">
          <td><strong>{{ patient.name }}</strong></td>
          <td><span class="ltr-value">{{ patient.phone }}</span></td>
          <td>{{ labelFor('gender', patient.gender) }}</td>
          <td>{{ patient.area || '—' }}</td>
          <td><StatusChip v-if="patient.subscription_status" type="subscription" :value="patient.subscription_status" /><span v-else class="muted-text">لا يوجد</span></td>
          <td>{{ patient.sponsor?.name || '—' }}</td>
          <td><v-btn :to="`/patients/${patient.id}`" color="primary" variant="text">فتح الملف</v-btn></td>
        </tr>
      </template>
    </DataTableCard>
  </div>
  <AddPatientDialog v-model="patientDialog" @saved="load" />
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { listPatients } from '@/api/patients'
import DataTableCard from '@/components/common/DataTableCard.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import AddPatientDialog from '@/components/patients/AddPatientDialog.vue'
import { labelFor } from '@/constants/labels'
import { useDebounce } from '@/composables/useDebounce'

const router = useRouter(); const debounce = useDebounce()
const patients = ref([]); const meta = ref(null); const loading = ref(true); const error = ref(''); const patientDialog = ref(false)
const filters = reactive({ search: '', subscription_status: null, page: 1, per_page: 15 })
const subscriptionFilters = [{ title: 'اشتراك نشط', value: 'active' }, { title: 'بدون اشتراك', value: 'none' }]
const headers = [{ title: 'المريض', key: 'name' }, { title: 'رقم الهاتف', key: 'phone' }, { title: 'الجنس', key: 'gender' }, { title: 'المنطقة', key: 'area' }, { title: 'حالة الاشتراك', key: 'subscription' }, { title: 'الجهة الراعية', key: 'sponsor' }, { title: 'الإجراءات', key: 'actions' }]
onMounted(load)
async function load() { loading.value = true; error.value = ''; try { const response = await listPatients(filters); patients.value = response.data; meta.value = response.meta } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
function searchChanged() { filters.page = 1; debounce(load) }
function filterChanged() { filters.page = 1; load() }
function changePage(page) { filters.page = page; load() }
</script>

<style scoped>
.patient-toolbar { display: grid; grid-template-columns: 2fr 1fr; gap: 16px; padding: 18px 20px; }
@media (max-width: 700px) { .patient-toolbar { grid-template-columns: 1fr; } }
</style>
