<template>
  <div class="page-container page-stack">
    <PageHeader title="الجهات الراعية" subtitle="المؤسسات المسؤولة عن تغطية تكاليف المرضى" action-label="إضافة جهة" action-icon="mdi-domain-plus" @action="openCreate" />
    <div class="surface-card sponsor-filter" data-form-flow-filter><v-text-field v-model="search" label="بحث باسم الجهة أو الهاتف" prepend-inner-icon="mdi-magnify" clearable @update:model-value="searchChanged" /></div>
    <DataTableCard :headers="headers" :items="sponsors" :loading="loading" :error="error" :meta="meta" @retry="load" @page="changePage"><template #rows="{ items }"><tr v-for="sponsor in items" :key="sponsor.id"><td><router-link class="record-link" :to="`/sponsors/${sponsor.id}`">{{ sponsor.name }}</router-link><span class="cell-subtitle">{{ sponsor.address }}</span></td><td><span class="phone-value">{{ sponsor.phone }}</span></td><td>{{ sponsor.patients_count }}</td><td><MoneyText :value="sponsor.total_due" /></td><td><MoneyText :value="sponsor.total_paid" /></td><td><strong><MoneyText :value="sponsor.remaining" /></strong></td><td><StatusChip type="active" :value="sponsor.is_active" /></td><td><v-menu><template #activator="{ props }"><v-btn v-bind="props" icon="mdi-dots-vertical" variant="text" aria-label="إجراءات الجهة الراعية" /></template><v-list density="compact"><v-list-item prepend-icon="mdi-eye-outline" title="التفاصيل" :to="`/sponsors/${sponsor.id}`" /><v-list-item prepend-icon="mdi-pencil-outline" title="تعديل" @click="openEdit(sponsor)" /></v-list></v-menu></td></tr></template></DataTableCard>
  </div>
  <v-dialog v-model="dialog" persistent max-width="600"><v-card><v-card-title class="dialog-title">{{ editing ? 'تعديل الجهة الراعية' : 'إضافة جهة راعية' }}</v-card-title><v-card-text class="dialog-content"><InlineError v-if="dialogError" :message="dialogError" /><v-text-field v-model="form.name" label="اسم المؤسسة" :error-messages="fieldErrors.name" /><v-text-field v-model="form.phone" label="بيانات الاتصال" dir="ltr" /><v-text-field v-model="form.address" label="العنوان" /><v-switch v-model="form.is_active" label="الجهة نشطة" color="primary" hide-details /></v-card-text><v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="saving" @click="dialog = false">إلغاء</v-btn><v-btn color="primary" data-form-flow-primary :loading="saving" @click="save">حفظ</v-btn></v-card-actions></v-card></v-dialog>
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { createSponsor, listSponsors, updateSponsor } from '@/api/sponsors'
import DataTableCard from '@/components/common/DataTableCard.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import { useDebounce } from '@/composables/useDebounce'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore(); const debounce = useDebounce()
const sponsors = ref([]); const meta = ref(null); const search = ref(''); const page = ref(1); const loading = ref(true); const error = ref(''); const dialog = ref(false); const editing = ref(false); const selected = ref(null); const saving = ref(false); const dialogError = ref(''); const fieldErrors = reactive({}); const form = reactive({ name: '', phone: '', address: '', is_active: true })
const headers = [{ title: 'الجهة', key: 'name' }, { title: 'الاتصال', key: 'phone' }, { title: 'المرضى', key: 'patients' }, { title: 'إجمالي المستحق', key: 'due' }, { title: 'المسدد', key: 'paid' }, { title: 'المتبقي', key: 'remaining' }, { title: 'الحالة', key: 'status' }, { title: 'الإجراءات', key: 'actions' }]
onMounted(load)
async function load() { loading.value = true; error.value = ''; try { const response = await listSponsors({ search: search.value, page: page.value }); sponsors.value = response.data; meta.value = response.meta } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
function clearFieldErrors() { Object.keys(fieldErrors).forEach((key) => delete fieldErrors[key]) }
function openCreate() { editing.value = false; selected.value = null; clearFieldErrors(); Object.assign(form, { name: '', phone: '', address: '', is_active: true }); dialogError.value = ''; dialog.value = true }
function openEdit(sponsor) { editing.value = true; selected.value = sponsor; clearFieldErrors(); Object.assign(form, sponsor); dialogError.value = ''; dialog.value = true }
async function save() { saving.value = true; dialogError.value = ''; clearFieldErrors(); try { if (editing.value) await updateSponsor(selected.value.id, form); else await createSponsor(form); ui.showSnackbar(editing.value ? 'تم تحديث بيانات الجهة.' : 'تمت إضافة الجهة الراعية.'); dialog.value = false; await load() } catch (apiError) { dialogError.value = apiError.message; Object.assign(fieldErrors, apiError.errors) } finally { saving.value = false } }
function searchChanged() { page.value = 1; debounce(load) } function changePage(value) { page.value = value; load() }
</script>

<style scoped>
.sponsor-filter { max-width: 560px; padding: 18px 20px; }.cell-subtitle { display: block; margin-top: 2px; color: var(--color-ink-muted); font-size: 13px; }.dialog-title { padding: 24px 24px 4px; font-size: var(--font-section-size); font-weight: 600; }.dialog-content { display: grid; gap: 16px; padding: 12px 24px !important; }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
</style>
