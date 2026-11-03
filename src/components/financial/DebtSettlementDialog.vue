<template>
  <v-dialog :model-value="modelValue" persistent max-width="760" scrollable @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">تسوية الديون</v-card-title>
      <v-card-text class="dialog-content">
        <p>تُقترح التخصيصات من الأقدم إلى الأحدث، ويمكن تعديلها دون تجاوز المتبقي.</p>
        <InlineError v-if="error" :message="error" />
        <div class="form-grid">
          <v-text-field v-model="amount" label="مبلغ الدفعة" type="text" inputmode="decimal" suffix="د.ل" :error-messages="errors.amount" @update:model-value="prefill" />
          <v-select v-model="method" label="طريقة الدفع" :items="paymentMethods" />
        </div>
        <div class="allocation-table">
          <div class="allocation-head"><span>المستحق</span><span>المتبقي</span><span>التخصيص</span></div>
          <div v-for="allocation in allocations" :key="allocation.debt_id" class="allocation-row">
            <span>دين #{{ allocation.debt_id }} · {{ kindLabel(allocation.debt) }}</span>
            <MoneyText :value="allocation.debt.remaining" />
            <v-text-field v-model="allocation.amount" density="compact" type="text" inputmode="decimal" suffix="د.ل" hide-details />
          </div>
        </div>
        <div class="allocation-summary">
          <span>مبلغ الدفعة <MoneyText :value="amount" /></span>
          <span>المخصص <MoneyText :value="allocatedTotal" /></span>
          <span :class="{ warning: unallocated < 0 }">غير المخصص <MoneyText :value="unallocated" /></span>
        </div>
        <span v-if="errors.allocations" class="field-error">{{ errors.allocations[0] }}</span>
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn>
        <v-btn color="primary" data-form-flow-primary :loading="saving" @click="save">تأكيد التسوية</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { settleDebts } from '@/api/financial'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import { paymentMethods } from '@/constants/labels'
import { decimal, numeric } from '@/composables/useMoney'
import { useIdempotency } from '@/composables/useIdempotency'
import { useUiStore } from '@/stores/ui'

const props = defineProps({ modelValue: Boolean, debts: { type: Array, default: () => [] } })
const emit = defineEmits(['update:modelValue', 'saved'])
const ui = useUiStore()
const { key, reset } = useIdempotency()
const amount = ref(null)
const method = ref('cash')
const allocations = ref([])
const error = ref('')
const errors = reactive({})
const saving = ref(false)
const allocatedTotal = computed(() => allocations.value.reduce((total, item) => total + numeric(item.amount), 0))
const unallocated = computed(() => numeric(amount.value) - allocatedTotal.value)

watch(() => props.modelValue, (open) => {
  if (!open) return
  amount.value = null; method.value = 'cash'; error.value = ''; reset()
  Object.keys(errors).forEach((item) => delete errors[item])
  allocations.value = props.debts.filter((debt) => ['open', 'partial'].includes(debt.status)).sort((a, b) => a.created_at.localeCompare(b.created_at)).map((debt) => ({ debt_id: debt.id, debt, amount: 0 }))
})

function kindLabel(debt) { return debt.kind === 'subscription' ? 'اشتراك' : debt.kind === 'opening' ? 'رصيد افتتاحي' : 'جلسة' }
function prefill() {
  let remainingPayment = numeric(amount.value)
  allocations.value.forEach((allocation) => {
    const value = Math.min(remainingPayment, numeric(allocation.debt.remaining))
    allocation.amount = Number(value.toFixed(3))
    remainingPayment -= value
  })
}
function close() { emit('update:modelValue', false) }
async function save() {
  saving.value = true; error.value = ''; Object.keys(errors).forEach((item) => delete errors[item])
  try {
    const response = await settleDebts({ amount: decimal(amount.value), method: method.value, allocations: allocations.value.filter((item) => numeric(item.amount) > 0).map((item) => ({ debt_id: item.debt_id, amount: decimal(item.amount) })) }, key.value)
    ui.showSnackbar('تم تسجيل الدفعة وتخصيصها على الديون بنجاح.')
    emit('saved', response.data); close()
  } catch (apiError) { error.value = apiError.message; Object.assign(errors, apiError.errors) } finally { saving.value = false }
}
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: 22px; font-weight: 600; }
.dialog-content { display: grid; gap: 18px; padding: 12px 24px !important; }
.dialog-content > p { margin: 0; color: var(--color-ink-muted); }
.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
.allocation-table { overflow: hidden; border: 1px solid var(--color-hairline); border-radius: 14px; }
.allocation-head, .allocation-row { display: grid; grid-template-columns: minmax(180px, 1fr) 130px 180px; align-items: center; gap: 12px; padding: 12px 16px; }
.allocation-head { color: var(--color-ink-muted); background: var(--color-parchment); font-size: 13px; font-weight: 600; }
.allocation-row { border-top: 1px solid var(--color-hairline); }
.allocation-summary { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 12px; padding: 14px 16px; border-radius: 12px; background: var(--color-pearl); }
.allocation-summary span { display: flex; gap: 6px; }
.allocation-summary .warning { color: var(--color-danger); }
.field-error { color: var(--color-danger); font-size: 13px; }
.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
@media (max-width: 640px) { .form-grid { grid-template-columns: 1fr; } .allocation-head, .allocation-row { grid-template-columns: 1fr; } .allocation-head { display: none; } }
</style>
