<template>
  <v-dialog :model-value="modelValue" persistent max-width="520" @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">شحن رصيد المريض</v-card-title>
      <v-card-text class="dialog-content">
        <p>الرصيد المدفوع مسبقًا مستقل عن الديون. الشحن يُسجل كمقبوض حقيقي.</p>
        <InlineError v-if="error" :message="error" />
        <div class="balance-row"><span>الرصيد الحالي</span><MoneyText :value="currentBalance" /></div>
        <v-text-field v-model="form.amount" label="مبلغ الشحن" type="text" inputmode="decimal" suffix="د.ل" :error-messages="errors.amount" />
        <v-select v-model="form.method" label="طريقة الدفع" :items="paymentMethods" />
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn>
        <v-btn color="primary" data-form-flow-primary :loading="saving" @click="save">تأكيد الشحن</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { reactive, ref, watch } from 'vue'
import { topUpWallet } from '@/api/patients'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import { paymentMethods } from '@/constants/labels'
import { decimal } from '@/composables/useMoney'
import { useIdempotency } from '@/composables/useIdempotency'
import { useUiStore } from '@/stores/ui'

const props = defineProps({ modelValue: Boolean, patientId: { type: [Number, String], required: true }, currentBalance: { type: [String, Number], default: '0.000' } })
const emit = defineEmits(['update:modelValue', 'saved'])
const ui = useUiStore()
const { key, reset } = useIdempotency()
const form = reactive({ amount: null, method: 'cash' })
const errors = reactive({})
const error = ref('')
const saving = ref(false)
watch(() => props.modelValue, (open) => { if (open) { form.amount = null; form.method = 'cash'; error.value = ''; Object.keys(errors).forEach((item) => delete errors[item]); reset() } })
function close() { emit('update:modelValue', false) }
async function save() {
  saving.value = true; error.value = ''
  try {
    const response = await topUpWallet(props.patientId, { amount: decimal(form.amount), method: form.method }, key.value)
    ui.showSnackbar('تم شحن الرصيد وتسجيل المقبوض بنجاح.')
    emit('saved', response.data)
    close()
  } catch (apiError) { error.value = apiError.message; Object.assign(errors, apiError.errors) } finally { saving.value = false }
}
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: 22px; font-weight: 600; }
.dialog-content { display: grid; gap: 16px; padding: 12px 24px !important; }
.dialog-content p { margin: 0; color: var(--color-ink-muted); }
.balance-row { display: flex; justify-content: space-between; padding: 14px; border-radius: 12px; background: var(--color-info-bg); }
.balance-row :deep(.money-text) { color: var(--color-primary); font-size: 20px; font-weight: 600; }
.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
</style>
