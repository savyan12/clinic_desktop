<template>
  <v-dialog :model-value="modelValue" persistent max-width="520" @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">تسجيل استرجاع مالي</v-card-title>
      <v-card-text class="dialog-content">
        <p>الاسترجاع حركة عكسية مستقلة ولا يحذف المعاملة الأصلية.</p>
        <InlineError v-if="error" :message="error" />
        <v-select v-model="form.payment_id" label="الدفعة الأصلية" :items="refundablePayments" :item-title="paymentTitle" item-value="id" :error-messages="errors.payment_id" />
        <v-text-field v-model="form.amount" label="المبلغ" type="text" inputmode="decimal" suffix="د.ل" :hint="selectedPayment ? `المتاح للاسترجاع ${formatMoney(selectedAvailable)}` : ''" persistent-hint :error-messages="errors.amount" />
        <v-select v-model="form.method" label="طريقة الاسترجاع" :items="paymentMethods" :error-messages="errors.method" />
        <v-textarea v-model="form.reason" label="سبب الاسترجاع" rows="3" :error-messages="errors.reason" />
      </v-card-text>
      <v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn><v-btn color="error" :loading="saving" @click="save">تأكيد الاسترجاع</v-btn></v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { createRefund } from '@/api/financial'
import InlineError from '@/components/common/InlineError.vue'
import { paymentMethods } from '@/constants/labels'
import { decimal, formatMoney, numeric } from '@/composables/useMoney'
import { useIdempotency } from '@/composables/useIdempotency'
import { useUiStore } from '@/stores/ui'

const props = defineProps({ modelValue: Boolean, patientId: { type: [Number, String], required: true }, payments: { type: Array, default: () => [] } })
const emit = defineEmits(['update:modelValue', 'saved'])
const ui = useUiStore(); const { key, reset } = useIdempotency()
const form = reactive({ payment_id: null, amount: null, method: 'cash', reason: '' }); const error = ref(''); const errors = reactive({}); const saving = ref(false)
const refundedByPayment = computed(() => props.payments.filter((payment) => payment.type === 'refund' && payment.reverses_payment_id).reduce((totals, payment) => {
  totals[payment.reverses_payment_id] = (totals[payment.reverses_payment_id] || 0) + Math.abs(numeric(payment.amount))
  return totals
}, {}))
const refundablePayments = computed(() => props.payments.filter((payment) => payment.type !== 'refund' && numeric(payment.amount) - (refundedByPayment.value[payment.id] || 0) > 0.0005))
const selectedPayment = computed(() => refundablePayments.value.find((payment) => payment.id === Number(form.payment_id)) || null)
const selectedAvailable = computed(() => selectedPayment.value ? numeric(selectedPayment.value.amount) - (refundedByPayment.value[selectedPayment.value.id] || 0) : 0)
watch(() => props.modelValue, (open) => {
  if (open) {
    Object.assign(form, { payment_id: refundablePayments.value[0]?.id || null, amount: null, method: 'cash', reason: '' })
    error.value = ''
    Object.keys(errors).forEach((field) => delete errors[field])
    reset()
  }
})
function paymentTitle(payment) { return `#${payment.id} · ${paymentLabel(payment.type)} · ${formatMoney(payment.amount)}` }
function paymentLabel(type) { return ({ wallet_topup: 'شحن رصيد', session_payment: 'دفعة جلسة', subscription_payment: 'دفعة اشتراك', debt_payment: 'تسديد دين', adjustment: 'تسوية' }[type] || type) }
function close() { emit('update:modelValue', false) }
async function save() { saving.value = true; error.value = ''; Object.keys(errors).forEach((field) => delete errors[field]); try { const response = await createRefund({ ...form, patient_id: Number(props.patientId), amount: decimal(form.amount) }, key.value); ui.showSnackbar('تم تسجيل الاسترجاع كحركة عكسية مستقلة.'); emit('saved', response.data); close() } catch (apiError) { error.value = apiError.message; Object.assign(errors, apiError.errors) } finally { saving.value = false } }
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: 22px; font-weight: 600; }.dialog-content { display: grid; gap: 16px; padding: 12px 24px !important; }.dialog-content p { margin: 0; color: var(--color-ink-muted); }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
</style>
