<template>
  <v-dialog :model-value="modelValue" persistent max-width="760" scrollable @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">إضافة اشتراك شهري</v-card-title>
      <v-card-text class="dialog-body">
        <p>الاشتراك بقيمة ثابتة ومدة بداية ونهاية، ولا يعتمد على عدد جلسات أو قائمة خدمات.</p>
        <InlineError v-if="error" :message="error" />
        <InlineError v-if="optionsError" :message="optionsError" retry @retry="loadOptions" />
        <PatientAutocomplete v-model="form.patient_id" :initial-patient="initialPatient" :disabled="Boolean(initialPatientId)" :error-messages="errors.patient_id" :show-add="false" />
        <div class="form-grid">
          <v-text-field v-model="form.amount" label="قيمة الاشتراك" type="text" inputmode="decimal" suffix="د.ل" :error-messages="errors.amount" />
          <v-text-field v-model="form.paid_amount" label="المدفوع عند الإنشاء" type="text" inputmode="decimal" suffix="د.ل" :error-messages="errors.paid_amount" />
          <v-text-field v-model="form.start_date" label="تاريخ البداية" type="date" :error-messages="errors.start_date" />
          <v-text-field v-model="form.end_date" label="تاريخ النهاية" type="date" :error-messages="errors.end_date" />
          <v-select v-model="form.method" label="طريقة دفع المبلغ الحالي" :items="paymentMethods" :disabled="numeric(form.paid_amount) <= 0" />
          <v-select v-model="form.sponsor_id" label="الجهة الراعية للدين (اختياري)" :items="sponsors" item-title="name" item-value="id" clearable :loading="loadingOptions" :disabled="numeric(remainingAmount) <= 0 || loadingOptions || Boolean(optionsError)" />
        </div>
        <div class="summary-grid">
          <div><span>قيمة الاشتراك</span><MoneyText :value="amountValue" /></div>
          <div><span>المدفوع</span><MoneyText :value="paidValue" /></div>
          <div><span>المتبقي دين</span><MoneyText :value="remainingAmount" /></div>
          <div><span>المدة</span><strong>{{ durationLabel }}</strong></div>
        </div>
        <div class="subscription-note"><v-icon icon="mdi-information-outline" /><span>يمكن تسجيل أي عدد من الجلسات خلال مدة الاشتراك، وتُختار الخدمات الفعلية عند تسجيل كل جلسة.</span></div>
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn>
        <v-btn color="primary" prepend-icon="mdi-wallet-membership" :loading="saving" @click="save">إنشاء الاشتراك</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { getSponsorOptions } from '@/api/sponsors'
import { useDialogOptions } from '@/composables/useDialogOptions'
import { createSubscription } from '@/api/subscriptions'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PatientAutocomplete from '@/components/common/PatientAutocomplete.vue'
import { paymentMethods } from '@/constants/labels'
import { decimal, numeric } from '@/composables/useMoney'
import { useIdempotency } from '@/composables/useIdempotency'
import { useUiStore } from '@/stores/ui'
import { clinicDate } from '@/utils/date'

const props = defineProps({ modelValue: Boolean, initialPatientId: { type: [Number, String], default: null }, initialPatient: { type: Object, default: null } })
const emit = defineEmits(['update:modelValue', 'saved'])
const ui = useUiStore()
const { key, reset } = useIdempotency()
const { data: sponsorOptions, loading: loadingOptions, error: optionsError, load: loadOptions } = useDialogOptions(() => props.modelValue, getSponsorOptions)
const sponsors = computed(() => (sponsorOptions.value || []).filter((item) => item.is_active))
const saving = ref(false)
const error = ref('')
const errors = reactive({})
const initialForm = () => ({ patient_id: props.initialPatientId ? Number(props.initialPatientId) : null, amount: '', paid_amount: '', method: 'cash', start_date: clinicDate(), end_date: addMonth(clinicDate()), sponsor_id: null })
const form = reactive(initialForm())
const amountValue = computed(() => decimal(form.amount))
const paidValue = computed(() => decimal(Math.min(numeric(form.paid_amount), numeric(form.amount))))
const remainingAmount = computed(() => decimal(Math.max(0, numeric(form.amount) - numeric(form.paid_amount))))
const durationLabel = computed(() => {
  if (!form.start_date || !form.end_date) return '—'
  const start = new Date(form.start_date + 'T00:00:00')
  const end = new Date(form.end_date + 'T00:00:00')
  const days = Math.floor((end - start) / 86400000) + 1
  return days > 0 ? days + ' يوم' : 'تاريخ غير صالح'
})

watch(() => props.modelValue, (open) => {
  if (!open) return
  Object.assign(form, initialForm())
  error.value = ''
  clearErrors()
  reset()
})

function addMonth(value) {
  const [year, month, day] = value.split('-').map(Number)
  const date = new Date(Date.UTC(year, month - 1, day))
  date.setUTCMonth(date.getUTCMonth() + 1)
  date.setUTCDate(date.getUTCDate() - 1)
  return date.toISOString().slice(0, 10)
}
function clearErrors() { Object.keys(errors).forEach((name) => delete errors[name]) }
function close() { emit('update:modelValue', false) }
async function save() {
  saving.value = true
  error.value = ''
  clearErrors()
  try {
    const response = await createSubscription({
      patient_id: form.patient_id,
      plan_type: 'monthly',
      amount: amountValue.value,
      paid_amount: paidValue.value,
      method: numeric(paidValue.value) > 0 ? form.method : null,
      start_date: form.start_date,
      end_date: form.end_date,
      sponsor_id: numeric(remainingAmount.value) > 0 ? form.sponsor_id : null,
    }, key.value)
    const subscription = response.data.subscription
    ui.showSnackbar('تم إنشاء الاشتراك الشهري وتسجيل الدفعة أو الدين حسب المبالغ.')
    emit('saved', subscription)
    close()
  } catch (apiError) {
    error.value = apiError.message
    Object.assign(errors, apiError.errors)
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: var(--font-section-size); font-weight: 600; }.dialog-body { display: grid; gap: 18px; padding: 12px 24px !important; }.dialog-body > p { margin: 0; color: var(--color-ink-muted); }
.form-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }.summary-grid { display: grid; grid-template-columns: repeat(4, 1fr); overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-field); }.summary-grid > div { display: grid; gap: 4px; padding: 14px; border-inline-start: 1px solid var(--color-hairline); }.summary-grid span { color: var(--color-ink-muted); font-size: 13px; }.summary-grid strong, .summary-grid :deep(.money-text) { font-size: 18px; font-weight: 600; }.subscription-note { display: flex; align-items: flex-start; gap: 10px; padding: 14px; color: var(--color-primary); border-radius: var(--radius-field); background: var(--color-info-bg); }.dialog-actions { justify-content: flex-end; gap: 8px; padding: 12px 24px 24px; }
@media (max-width: 660px) { .form-grid, .summary-grid { grid-template-columns: 1fr; } }
</style>
