<template>
  <v-dialog :model-value="modelValue" persistent max-width="680" scrollable @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">إلغاء الاشتراك</v-card-title>
      <v-card-text class="dialog-body">
        <p>حدد مبلغ الاسترجاع الفعلي. لن يحسب النظام استرجاعًا تلقائيًا من عدد الجلسات المستخدمة.</p>
        <v-skeleton-loader v-if="loadingPreview" type="paragraph, article" />
        <InlineError v-else-if="error" :message="error" retry @retry="loadPreview" />
        <template v-else-if="preview">
          <div class="preview-grid">
            <div><span>الجلسات المستخدمة</span><strong>{{ preview.sessions_used }}</strong></div>
            <div><span>الجلسات المتبقية</span><strong>{{ preview.sessions_remaining }}</strong></div>
            <div><span>قيمة الاشتراك</span><MoneyText :value="preview.total_price" /></div>
            <div><span>المبلغ المحصل</span><MoneyText :value="preview.received_amount" /></div>
            <div><span>المسترجع سابقًا</span><MoneyText :value="preview.refunded_amount" /></div>
            <div><span>الدين الحالي</span><MoneyText :value="preview.outstanding_amount" /></div>
            <div class="refund"><span>الحد الأقصى للاسترجاع</span><MoneyText :value="preview.maximum_refund" /></div>
          </div>
          <div class="form-grid">
            <v-text-field v-model="refundAmount" label="مبلغ الاسترجاع الفعلي" type="text" inputmode="decimal" suffix="د.ل" :error-messages="refundError" />
            <v-select v-model="refundMethod" label="طريقة الاسترجاع" :items="paymentMethods" :disabled="numeric(refundAmount) <= 0" />
          </div>
          <v-textarea v-model="reason" label="سبب الإلغاء" rows="3" counter="1000" :error-messages="reasonError" />
        </template>
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="saving" @click="close">تراجع</v-btn>
        <v-btn color="error" data-form-flow-primary :loading="saving" :disabled="saving || loadingPreview || !preview || Boolean(error)" @click="submit">تأكيد إلغاء الاشتراك</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { ref, watch } from 'vue'
import { cancelSubscription, getSubscriptionCancellationPreview } from '@/api/subscriptions'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import { numeric } from '@/composables/useMoney'
import { paymentMethods } from '@/constants/labels'
import { useIdempotency } from '@/composables/useIdempotency'

const props = defineProps({ modelValue: Boolean, subscriptionId: { type: [Number, String], required: true } })
const emit = defineEmits(['update:modelValue', 'saved'])
const { key, reset } = useIdempotency()
const preview = ref(null)
const loadingPreview = ref(false)
const saving = ref(false)
const error = ref('')
const reason = ref('')
const reasonError = ref('')
const refundAmount = ref('0')
const refundMethod = ref('cash')
const refundError = ref('')

watch(() => props.modelValue, (open) => {
  if (!open) return
  preview.value = null; error.value = ''; reason.value = ''; reasonError.value = ''; refundAmount.value = '0'; refundMethod.value = 'cash'; refundError.value = ''; reset(); loadPreview()
})

async function loadPreview() {
  loadingPreview.value = true; error.value = ''
  try { preview.value = (await getSubscriptionCancellationPreview(props.subscriptionId)).data } catch (apiError) { error.value = apiError.message } finally { loadingPreview.value = false }
}
function close() { if (!saving.value) emit('update:modelValue', false) }
async function submit() {
  reasonError.value = ''; refundError.value = ''
  if (!reason.value.trim()) reasonError.value = 'يرجى إدخال سبب الإلغاء.'
  const amount = numeric(refundAmount.value)
  if (amount < 0 || amount > numeric(preview.value.maximum_refund)) refundError.value = `الحد الأقصى للاسترجاع هو ${preview.value.maximum_refund}.`
  if (reasonError.value || refundError.value) return
  saving.value = true
  try {
    const response = await cancelSubscription(props.subscriptionId, { reason: reason.value.trim(), refund_amount: amount, refund_method: amount > 0 ? refundMethod.value : null }, key.value)
    emit('saved', response.data)
    emit('update:modelValue', false)
  } catch (apiError) { error.value = apiError.message; refundError.value = apiError.errors?.refund_amount || '' } finally { saving.value = false }
}
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: 22px; font-weight: 600; }.dialog-body { display: grid; gap: 16px; padding: 12px 24px !important; }.dialog-body > p { margin: 0; color: var(--color-ink-muted); line-height: 1.7; }.preview-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); overflow: hidden; border: 1px solid var(--color-hairline); border-radius: 14px; }.preview-grid > div { display: grid; gap: 4px; min-height: 78px; padding: 14px 16px; border-inline-start: 1px solid var(--color-hairline); border-top: 1px solid var(--color-hairline); }.preview-grid span { color: var(--color-ink-muted); font-size: 13px; }.preview-grid strong, .preview-grid :deep(.money-text) { font-size: 19px; font-weight: 600; }.preview-grid .refund { color: var(--color-danger); background: var(--color-danger-bg); }.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
@media (max-width: 560px) { .preview-grid, .form-grid { grid-template-columns: 1fr; } }
</style>
