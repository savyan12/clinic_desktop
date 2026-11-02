<template>
  <div class="page-container page-stack">
    <PageHeader title="الأطباء ومستحقاتهم" subtitle="المستحق المكتسب منفصل عن المبلغ المصروف" action-label="إضافة طبيب" action-icon="mdi-doctor" @action="openCreate" />
    <div class="surface-card doctor-filter"><v-text-field v-model="search" label="بحث بالاسم أو الهاتف" prepend-inner-icon="mdi-magnify" clearable @update:model-value="searchChanged" /></div>
    <DataTableCard :headers="headers" :items="doctors" :loading="loading" :error="error" :meta="meta" @retry="load" @page="changePage">
      <template #rows="{ items }"><tr v-for="doctor in items" :key="doctor.id" class="clickable-row" @click="router.push(`/doctors/${doctor.id}`)"><td><strong>{{ doctor.name }}</strong></td><td><span class="ltr-value">{{ doctor.phone }}</span></td><td><StatusChip type="active" :value="doctor.is_active" /></td><td>{{ doctor.sessions_count }}</td><td><span class="ltr-value">{{ Number(doctor.commission_rate).toFixed(0) }}%</span></td><td><MoneyText :value="doctor.earned" /></td><td><MoneyText :value="doctor.paid" /></td><td><strong><MoneyText :value="doctor.remaining" /></strong></td><td @click.stop><v-menu><template #activator="{ props }"><v-btn v-bind="props" icon="mdi-dots-vertical" variant="text" /></template><v-list density="compact"><v-list-item prepend-icon="mdi-eye-outline" title="التفاصيل" :to="`/doctors/${doctor.id}`" /><v-list-item prepend-icon="mdi-pencil-outline" title="تعديل" @click="openEdit(doctor)" /></v-list></v-menu></td></tr></template>
    </DataTableCard>
  </div>
  <v-dialog v-model="dialog" persistent max-width="600"><v-card><v-card-title class="dialog-title">{{ editing ? 'تعديل بيانات الطبيب' : 'إضافة طبيب' }}</v-card-title><v-card-text class="dialog-content"><InlineError v-if="dialogError" :message="dialogError" /><v-text-field v-model="form.name" label="الاسم" :error-messages="fieldErrors.name" /><v-text-field v-model="form.phone" label="رقم الهاتف" dir="ltr" /><v-text-field v-model="form.commission_rate" label="نسبة المستحق" type="text" inputmode="decimal" suffix="%" /><p class="rate-note"><v-icon icon="mdi-information-outline" /> تُحسب النسبة من صافي قيمة الجلسة بعد الخصم (D-01)، وتحفظ مع كل جلسة.</p><v-switch v-model="form.is_active" label="الطبيب نشط ويمكن إسناد جلسات جديدة إليه" color="primary" hide-details /><v-textarea v-if="editing" v-model="form.reason" label="سبب تغيير النسبة أو البيانات" rows="2" /></v-card-text><v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="saving" @click="dialog = false">إلغاء</v-btn><v-btn color="primary" :loading="saving" @click="save">حفظ</v-btn></v-card-actions></v-card></v-dialog>
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { createDoctor, listDoctors, updateDoctor } from '@/api/doctors'
import DataTableCard from '@/components/common/DataTableCard.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import { decimal } from '@/composables/useMoney'
import { useDebounce } from '@/composables/useDebounce'
import { useUiStore } from '@/stores/ui'

const router = useRouter(); const ui = useUiStore(); const debounce = useDebounce()
const doctors = ref([]); const meta = ref(null); const search = ref(''); const page = ref(1); const loading = ref(true); const error = ref(''); const dialog = ref(false); const editing = ref(false); const selected = ref(null); const saving = ref(false); const dialogError = ref(''); const fieldErrors = reactive({})
const form = reactive({ name: '', phone: '', commission_rate: 25, is_active: true, reason: '' })
const headers = [{ title: 'الطبيب', key: 'name' }, { title: 'الهاتف', key: 'phone' }, { title: 'الحالة', key: 'status' }, { title: 'الجلسات', key: 'sessions' }, { title: 'النسبة', key: 'rate' }, { title: 'المكتسب', key: 'earned' }, { title: 'المصروف', key: 'paid' }, { title: 'المتبقي', key: 'remaining' }, { title: '', key: 'actions' }]
onMounted(load)
async function load() { loading.value = true; error.value = ''; try { const response = await listDoctors({ search: search.value, page: page.value }); doctors.value = response.data; meta.value = response.meta } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
function openCreate() { editing.value = false; selected.value = null; Object.assign(form, { name: '', phone: '', commission_rate: 25, is_active: true, reason: '' }); dialogError.value = ''; dialog.value = true }
function openEdit(doctor) { editing.value = true; selected.value = doctor; Object.assign(form, { name: doctor.name, phone: doctor.phone, commission_rate: Number(doctor.commission_rate), is_active: doctor.is_active, reason: '' }); dialogError.value = ''; dialog.value = true }
async function save() { saving.value = true; try { const payload = { ...form, commission_rate: decimal(form.commission_rate) }; if (editing.value) await updateDoctor(selected.value.id, payload); else await createDoctor(payload); ui.showSnackbar(editing.value ? 'تم تحديث بيانات الطبيب وحفظ التغيير في سجل التدقيق.' : 'تمت إضافة الطبيب.'); dialog.value = false; await load() } catch (apiError) { dialogError.value = apiError.message; Object.assign(fieldErrors, apiError.errors) } finally { saving.value = false } }
function searchChanged() { page.value = 1; debounce(load) } function changePage(value) { page.value = value; load() }
</script>

<style scoped>
.doctor-filter { max-width: 560px; padding: 18px 20px; }.dialog-title { padding: 24px 24px 4px; font-size: 22px; font-weight: 600; }.dialog-content { display: grid; gap: 16px; padding: 12px 24px !important; }.rate-note { display: flex; gap: 8px; margin: -4px 0 0; padding: 12px; color: var(--color-primary); border-radius: 12px; background: var(--color-info-bg); font-size: 14px; }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
</style>
