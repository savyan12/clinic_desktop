<template>
  <div class="page-container page-stack">
    <PageHeader title="الأطباء ومستحقاتهم" action-label="إضافة طبيب" action-icon="mdi-doctor" @action="openCreate" />

    <section class="doctor-metrics" aria-label="ملخص مستحقات الأطباء">
      <StatCell label="إجمالي المكتسب" :value="totals.earned" money caption="مستحقات جلسات مغلقة" icon="mdi-trending-up" tone="violet" />
      <StatCell label="إجمالي المصروف" :value="totals.paid" money caption="دفعات مسجلة" icon="mdi-cash-check" tone="green" />
      <StatCell label="المتبقي للصرف" :value="totals.remaining" money caption="متاح للصرف" icon="mdi-wallet-outline" tone="amber" />
    </section>

    <DataTableCard :headers="headers" :items="doctors" :loading="loading" :error="error" :meta="meta" :empty="{ icon: 'mdi-doctor', title: 'لا يوجد أطباء', description: 'أضف أول طبيب لبدء إسناد الجلسات.' }" @retry="load" @page="changePage">
      <template #header>
        <div class="doctor-toolbar" data-form-flow-filter>
          <div class="table-title"><h2>قائمة الأطباء</h2><span>{{ meta?.total || 0 }} طبيب</span></div>
          <v-text-field v-model="search" label="بحث" placeholder="الاسم أو رقم الهاتف" prepend-inner-icon="mdi-magnify" clearable @update:model-value="searchChanged" />
          <v-btn v-if="search" variant="text" size="small" @click="clearSearch">مسح</v-btn>
        </div>
      </template>
      <template #rows="{ items }">
        <tr v-for="doctor in items" :key="doctor.id">
          <td><div class="person-cell"><span class="person-avatar" aria-hidden="true">{{ doctor.name.charAt(0) }}</span><span><router-link class="record-link" :to="`/doctors/${doctor.id}`">{{ doctor.name }}</router-link><small class="phone-value">{{ doctor.phone }}</small></span></div></td>
          <td><StatusChip type="active" :value="doctor.is_active" /></td>
          <td><span class="ltr-value">{{ doctor.sessions_count }}</span></td>
          <td><span class="rate-chip ltr-value">{{ Number(doctor.commission_rate).toFixed(0) }}%</span></td>
          <td><MoneyText :value="doctor.earned" /></td>
          <td><MoneyText :value="doctor.paid" /></td>
          <td class="remaining-cell"><MoneyText :value="doctor.remaining" /></td>
          <td><v-menu><template #activator="{ props }"><v-btn v-bind="props" icon="mdi-dots-vertical" variant="text" aria-label="إجراءات الطبيب" /></template><v-list density="compact"><v-list-item prepend-icon="mdi-eye-outline" title="التفاصيل" :to="`/doctors/${doctor.id}`" /><v-list-item prepend-icon="mdi-pencil-outline" title="تعديل" @click="openEdit(doctor)" /></v-list></v-menu></td>
        </tr>
      </template>
      <template #cards="{ items }">
        <article v-for="doctor in items" :key="doctor.id" class="doctor-mobile-card">
          <div class="person-cell"><span class="person-avatar" aria-hidden="true">{{ doctor.name.charAt(0) }}</span><span><router-link class="record-link" :to="`/doctors/${doctor.id}`">{{ doctor.name }}</router-link><small class="phone-value">{{ doctor.phone }}</small></span></div>
          <StatusChip type="active" :value="doctor.is_active" />
          <div><span>المتبقي</span><MoneyText :value="doctor.remaining" /></div>
          <v-btn :to="`/doctors/${doctor.id}`" variant="text" append-icon="mdi-arrow-left">التفاصيل</v-btn>
        </article>
      </template>
    </DataTableCard>
  </div>

  <v-dialog v-model="dialog" persistent max-width="600">
    <v-card>
      <v-card-title class="dialog-title">{{ editing ? 'تعديل بيانات الطبيب' : 'إضافة طبيب' }}</v-card-title>
      <v-card-text class="dialog-content">
        <InlineError v-if="dialogError" :message="dialogError" />
        <v-text-field v-model="form.name" label="الاسم" :error-messages="fieldErrors.name" />
        <v-text-field v-model="form.phone" label="رقم الهاتف" dir="ltr" />
        <v-text-field v-model="form.commission_rate" label="نسبة المستحق" type="text" inputmode="decimal" suffix="%" />
        <p class="rate-note"><v-icon icon="mdi-information-outline" aria-hidden="true" /> تُحسب النسبة من صافي قيمة الجلسة بعد الخصم، وتحفظ تاريخيًا مع كل جلسة.</p>
        <v-switch v-model="form.is_active" label="الطبيب نشط ويمكن إسناد جلسات جديدة إليه" color="primary" hide-details />
        <v-textarea v-if="editing" v-model="form.reason" label="سبب تغيير النسبة أو البيانات" rows="2" />
      </v-card-text>
      <v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="saving" @click="dialog = false">إلغاء</v-btn><v-btn color="primary" data-form-flow-primary :loading="saving" @click="save">حفظ</v-btn></v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { createDoctor, listDoctors, updateDoctor } from '@/api/doctors'
import DataTableCard from '@/components/common/DataTableCard.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatCell from '@/components/common/StatCell.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import { decimal, numeric } from '@/composables/useMoney'
import { useDebounce } from '@/composables/useDebounce'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
const debounce = useDebounce()
const doctors = ref([])
const allDoctors = ref([])
const meta = ref(null)
const search = ref('')
const page = ref(1)
const loading = ref(true)
const error = ref('')
const dialog = ref(false)
const editing = ref(false)
const selected = ref(null)
const saving = ref(false)
const dialogError = ref('')
const fieldErrors = reactive({})
const form = reactive({ name: '', phone: '', commission_rate: 25, is_active: true, reason: '' })
const headers = [{ title: 'الطبيب', key: 'name' }, { title: 'الحالة', key: 'status' }, { title: 'الجلسات', key: 'sessions' }, { title: 'النسبة', key: 'rate' }, { title: 'المكتسب', key: 'earned' }, { title: 'المصروف', key: 'paid' }, { title: 'المتبقي', key: 'remaining' }, { title: '', key: 'actions' }]
const totals = computed(() => allDoctors.value.reduce((sum, doctor) => ({ earned: sum.earned + numeric(doctor.earned), paid: sum.paid + numeric(doctor.paid), remaining: sum.remaining + numeric(doctor.remaining) }), { earned: 0, paid: 0, remaining: 0 }))

onMounted(load)
async function load() {
  loading.value = true
  error.value = ''
  try {
    const [response, totalsResponse] = await Promise.all([listDoctors({ search: search.value, page: page.value }), listDoctors({ per_page: 100 })])
    doctors.value = response.data
    meta.value = response.meta
    allDoctors.value = totalsResponse.data
  } catch (apiError) { error.value = apiError.message } finally { loading.value = false }
}
function clearFieldErrors() { Object.keys(fieldErrors).forEach((key) => delete fieldErrors[key]) }
function openCreate() { editing.value = false; selected.value = null; clearFieldErrors(); Object.assign(form, { name: '', phone: '', commission_rate: 25, is_active: true, reason: '' }); dialogError.value = ''; dialog.value = true }
function openEdit(doctor) { editing.value = true; selected.value = doctor; clearFieldErrors(); Object.assign(form, { name: doctor.name, phone: doctor.phone, commission_rate: Number(doctor.commission_rate), is_active: doctor.is_active, reason: '' }); dialogError.value = ''; dialog.value = true }
async function save() { saving.value = true; dialogError.value = ''; clearFieldErrors(); try { const payload = { ...form, commission_rate: decimal(form.commission_rate) }; if (editing.value) await updateDoctor(selected.value.id, payload); else await createDoctor(payload); ui.showSnackbar(editing.value ? 'تم تحديث بيانات الطبيب وحفظ التغيير في سجل التدقيق.' : 'تمت إضافة الطبيب.'); dialog.value = false; await load() } catch (apiError) { dialogError.value = apiError.message; Object.assign(fieldErrors, apiError.errors) } finally { saving.value = false } }
function searchChanged() { page.value = 1; debounce(load) }
function changePage(value) { page.value = value; load() }
function clearSearch() { search.value = ''; page.value = 1; load() }
</script>

<style scoped>
.doctor-metrics { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: var(--grid-gap); }
.doctor-toolbar { display: grid; grid-template-columns: minmax(180px, 1fr) minmax(260px, 1.4fr) auto; gap: 12px; align-items: start; }
.table-title h2 { margin: 0; font-size: 17px; font-weight: 600; }.table-title span { color: var(--text-muted); font-size: 12px; }
.person-cell { min-width: 0; display: flex; align-items: center; gap: 10px; }.person-cell > span:last-child { min-width: 0; }.person-cell strong, .person-cell small { display: block; }.person-avatar { width: 38px; height: 38px; display: grid; flex: 0 0 38px; place-items: center; color: var(--accent); background: var(--accent-tint); border-radius: 50%; font-weight: 600; }.person-cell small { margin-top: 2px; color: var(--text-muted); font-size: 12px; }
.rate-chip { padding: 3px 9px; color: var(--violet-fg); background: var(--violet-bg); border-radius: var(--r-chip); font-size: 13px; font-weight: 600; }.remaining-cell :deep(.money-text) { color: var(--ink); font-weight: 700; }
.doctor-mobile-card { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 14px; background: var(--surface-subtle); border: 1px solid var(--border); border-radius: 14px; }.doctor-mobile-card > div:nth-child(3) span { display: block; color: var(--text-muted); font-size: 12px; }.doctor-mobile-card > div:nth-child(3) :deep(.money-text) { color: var(--ink); font-size: 18px; font-weight: 700; }
.dialog-title { padding: 24px 24px 4px; }.dialog-content { display: grid; gap: 16px; padding: 12px 24px !important; }.rate-note { display: flex; align-items: flex-start; gap: 8px; margin: -4px 0 0; padding: 0 2px; color: var(--text-muted); font-size: 13px; }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
@media (max-width: 900px) { .doctor-metrics { grid-template-columns: 1fr; } }
@media (max-width: 700px) { .doctor-toolbar { grid-template-columns: 1fr; } .table-title { display: none; } }
</style>
