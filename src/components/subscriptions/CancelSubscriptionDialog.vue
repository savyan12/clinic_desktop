<template>
  <v-dialog :model-value="modelValue" persistent max-width="680" scrollable @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">إلغاء الاشتراك</v-card-title>
      <v-card-text class="dialog-body">
        <p>سيصبح الاشتراك ملغيًا نهائيًا، وتُسوّى قيمته حسب المدة المستهلكة للاشتراك الشهري أو الجلسات المستخدمة للباقات القديمة.</p>
        <v-skeleton-loader v-if="loadingPreview" type="paragraph, article" />
        <InlineError v-else-if="error" :message="error" retry @retry="loadPreview" />
        <template v-else-if="preview">
          <div class="preview-grid">
            <template v-if="preview.plan_type === 'monthly'">
              <div><span>أيام الاشتراك</span><strong>{{ preview.period_days }}</strong></div>
              <div><span>الأيام المستهلكة</span><strong>{{ preview.elapsed_days }}</strong></div>
              <div><span>الأيام المتبقية</span><strong>{{ preview.remaining_days }}</strong></div>
            </template>
            <template v-else>
              <div><span>الجلسات المستخدمة</span><strong>{{ preview.sessions_used }}</strong></div>
              <div><span>الجلسات المتبقية</span><strong>{{ preview.sessions_remaining }}</strong></div>
            </template>
            <div><span>القيمة المستهلكة</span><MoneyText :value="preview.consumed_value" /></div>
            <div><span>المدفوع الفعلي</span><MoneyText :value="preview.paid_total" /></div>
            <div class="refund"><span>الاسترجاع النقدي</span><MoneyText :value="preview.refund_amount" /></div>
            <div><span>الدين الملغى</span><MoneyText :value="preview.debt_voided_amount" /></div>
            <div><span>الدين بعد الإلغاء</span><MoneyText :value="preview.replacement_debt_amount" /></div>
          </div>
          <p class="preview-note" v-if="Number(preview.refund_amount) > 0">سيُسجل الاسترجاع نقدًا في سجل المقبوضات.</p>
          <p class="preview-note" v-else-if="Number(preview.replacement_debt_amount) > 0">سيُلغى الدين السابق ويُنشأ دين جديد بالقيمة المستهلكة المتبقية فقط.</p>
          <p class="preview-note" v-else>لن ينتج عن الإلغاء استرجاع أو دين جديد.</p>
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

watch(() => props.modelValue, (open) => {
  if (!open) return
  preview.value = null; error.value = ''; reason.value = ''; reasonError.value = ''; reset(); loadPreview()
})

async function loadPreview() {
  loadingPreview.value = true; error.value = ''
  try { preview.value = (await getSubscriptionCancellationPreview(props.subscriptionId)).data } catch (apiError) { error.value = apiError.message } finally { loadingPreview.value = false }
}
function close() { if (!saving.value) emit('update:modelValue', false) }
async function submit() {
  if (!reason.value.trim()) { reasonError.value = 'يرجى إدخال سبب الإلغاء.'; return }
  saving.value = true; reasonError.value = ''
  try {
    const response = await cancelSubscription(props.subscriptionId, { reason: reason.value.trim() }, key.value)
    emit('saved', response.data)
    emit('update:modelValue', false)
  } catch (apiError) { error.value = apiError.message } finally { saving.value = false }
}
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: 22px; font-weight: 600; }.dialog-body { display: grid; gap: 16px; padding: 12px 24px !important; }.dialog-body > p { margin: 0; color: var(--color-ink-muted); line-height: 1.7; }.preview-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); overflow: hidden; border: 1px solid var(--color-hairline); border-radius: 14px; }.preview-grid > div { display: grid; gap: 4px; min-height: 78px; padding: 14px 16px; border-inline-start: 1px solid var(--color-hairline); border-top: 1px solid var(--color-hairline); }.preview-grid span { color: var(--color-ink-muted); font-size: 13px; }.preview-grid strong, .preview-grid :deep(.money-text) { font-size: 19px; font-weight: 600; }.preview-grid .refund { color: var(--color-danger); background: var(--color-danger-bg); }.preview-note { padding: 12px 14px; border-radius: 12px; background: var(--color-parchment); }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
@media (max-width: 560px) { .preview-grid { grid-template-columns: 1fr; } }
</style>
