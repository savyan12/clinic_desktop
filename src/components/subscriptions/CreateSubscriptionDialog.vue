<template>
  <v-dialog :model-value="modelValue" persistent max-width="760" scrollable @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">إضافة اشتراك</v-card-title>
      <v-card-text class="dialog-body">
        <p>أنشئ باقة بعدد جلسات وسعر إجمالي ثابت. يبدأ الاشتراك تلقائيًا من تاريخ إنشائه.</p>
        <InlineError v-if="error" :message="error" />
        <InlineError v-if="optionsError" :message="optionsError" retry @retry="loadOptions" />
        <PatientAutocomplete v-model="form.patient_id" :initial-patient="initialPatient" :disabled="Boolean(initialPatientId)" :error-messages="errors.patient_id" :show-add="false" />
        <div class="form-grid">
          <v-text-field v-model="form.name" class="full-field" label="اسم الاشتراك" :error-messages="errors.name" />
          <v-select v-model="form.service_ids" class="full-field" label="الخدمات المغطاة" :items="services" item-title="name" item-value="id" multiple chips closable-chips :loading="loadingOptions" :disabled="loadingOptions || Boolean(optionsError)" :error-messages="errors.service_ids || errors['service_ids.0']" />
          <v-text-field v-model="form.sessions_count" label="إجمالي عدد الجلسات" type="number" min="1" :error-messages="errors.sessions_count" />
          <v-text-field v-model="form.total_price" label="سعر الاشتراك الإجمالي" type="text" inputmode="decimal" suffix="د.ل" :error-messages="errors.total_price" />
          <v-text-field v-model="form.paid_amount" label="المبلغ المدفوع عند الإنشاء" type="text" inputmode="decimal" suffix="د.ل" :error-messages="errors.paid_amount" />
          <v-select v-model="form.method" label="طريقة الدفع" :items="paymentMethods" :error-messages="errors.method" />
        </div>
        <div class="summary-grid">
          <div><span>إجمالي الجلسات</span><strong>{{ sessionsValue || '—' }}</strong></div>
          <div><span>قيمة الاشتراك</span><MoneyText :value="totalPriceValue" /></div>
          <div><span>المدفوع</span><MoneyText :value="paidValue" /></div>
          <div><span>المتبقي</span><MoneyText :value="remainingAmount" /></div>
        </div>
        <div class="subscription-note"><v-icon icon="mdi-information-outline" /><span>كل زيارة مكتملة تخصم جلسة واحدة مهما كان عدد الخدمات المغطاة المستخدمة فيها.</span></div>
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn>
        <v-btn color="primary" prepend-icon="mdi-wallet-membership" data-form-flow-primary :loading="saving" @click="save">إنشاء الاشتراك</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { getServiceOptions } from '@/api/services'
import { useDialogOptions } from '@/composables/useDialogOptions'
import { createSubscription } from '@/api/subscriptions'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PatientAutocomplete from '@/components/common/PatientAutocomplete.vue'
import { paymentMethods } from '@/constants/labels'
import { decimal, numeric } from '@/composables/useMoney'
import { useIdempotency } from '@/composables/useIdempotency'
import { useUiStore } from '@/stores/ui'

const props = defineProps({ modelValue: Boolean, initialPatientId: { type: [Number, String], default: null }, initialPatient: { type: Object, default: null } })
const emit = defineEmits(['update:modelValue', 'saved'])
const ui = useUiStore()
const { key, reset } = useIdempotency()
const { data: serviceOptions, loading: loadingOptions, error: optionsError, load: loadOptions } = useDialogOptions(() => props.modelValue, getServiceOptions)
const services = computed(() => (serviceOptions.value || []).filter((item) => item.is_active))
const saving = ref(false)
const error = ref('')
const errors = reactive({})
const initialForm = () => ({ patient_id: props.initialPatientId ? Number(props.initialPatientId) : null, name: '', service_ids: [], sessions_count: '', total_price: '', paid_amount: '', method: 'cash' })
const form = reactive(initialForm())
const sessionsValue = computed(() => Math.max(0, Number.parseInt(form.sessions_count, 10) || 0))
const totalPriceValue = computed(() => decimal(form.total_price))
const paidValue = computed(() => decimal(Math.min(numeric(form.paid_amount), numeric(form.total_price))))
const remainingAmount = computed(() => decimal(Math.max(0, numeric(form.total_price) - numeric(form.paid_amount))))

watch(() => props.modelValue, (open) => {
  if (!open) return
  Object.assign(form, initialForm())
  error.value = ''
  clearErrors()
  reset()
})
function clearErrors() { Object.keys(errors).forEach((name) => delete errors[name]) }
function close() { emit('update:modelValue', false) }
async function save() {
  saving.value = true
  error.value = ''
  clearErrors()
  if (!form.service_ids.length) errors.service_ids = ['اختر خدمة واحدة على الأقل.']
  if (!sessionsValue.value) errors.sessions_count = ['أدخل عدد جلسات أكبر من صفر.']
  if (Object.keys(errors).length) { saving.value = false; return }
  try {
    const response = await createSubscription({
      patient_id: form.patient_id,
      name: form.name.trim(),
      service_ids: form.service_ids.map(Number),
      sessions_count: sessionsValue.value,
      total_price: totalPriceValue.value,
      paid_amount: paidValue.value,
      method: numeric(paidValue.value) > 0 ? form.method : null,
    }, key.value)
    const subscription = response.data.subscription || response.data
    ui.showSnackbar('تم إنشاء الاشتراك وتسجيل الدفعة والمتبقي المالي.')
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
.form-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }.full-field { grid-column: 1 / -1; }.summary-grid { display: grid; grid-template-columns: repeat(4, 1fr); overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-field); }.summary-grid > div { display: grid; gap: 4px; padding: 14px; border-inline-start: 1px solid var(--color-hairline); }.summary-grid span { color: var(--color-ink-muted); font-size: 13px; }.summary-grid strong, .summary-grid :deep(.money-text) { font-size: 18px; font-weight: 600; }.subscription-note { display: flex; align-items: flex-start; gap: 10px; padding: 14px; color: var(--color-primary); border-radius: var(--radius-field); background: var(--color-info-bg); }.dialog-actions { justify-content: flex-end; gap: 8px; padding: 12px 24px 24px; }
@media (max-width: 660px) { .form-grid, .summary-grid { grid-template-columns: 1fr; }.full-field { grid-column: auto; } }
</style>
