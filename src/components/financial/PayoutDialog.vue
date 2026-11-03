<template>
  <v-dialog :model-value="modelValue" persistent max-width="560" @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">صرف مستحقات الطبيب</v-card-title>
      <v-card-text class="dialog-content">
        <InlineError v-if="error" :message="error" />
        <div class="due-row"><span>المستحق المتبقي</span><MoneyText :value="doctor?.remaining" /></div>
        <v-text-field v-model="form.amount" label="مبلغ الصرف" type="text" inputmode="decimal" suffix="د.ل" :error-messages="errors.amount" />
        <v-text-field v-model="form.date" label="التاريخ" type="date" />
        <v-select v-model="form.method" label="طريقة الدفع" :items="paymentMethods" />
        <v-textarea v-model="form.note" label="ملاحظة أو مرجع" rows="2" />
      </v-card-text>
      <v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn><v-btn color="primary" data-form-flow-primary :loading="saving" @click="save">تأكيد الصرف</v-btn></v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { reactive, ref, watch } from 'vue'
import { createPayout } from '@/api/doctors'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import { paymentMethods } from '@/constants/labels'
import { decimal } from '@/composables/useMoney'
import { useIdempotency } from '@/composables/useIdempotency'
import { useUiStore } from '@/stores/ui'
import { clinicDate } from '@/utils/date'

const props = defineProps({ modelValue: Boolean, doctor: { type: Object, default: null } })
const emit = defineEmits(['update:modelValue', 'saved'])
const ui = useUiStore(); const { key, reset } = useIdempotency()
const today = () => clinicDate()
const form = reactive({ amount: null, date: today(), method: 'cash', note: '' }); const error = ref(''); const errors = reactive({}); const saving = ref(false)
watch(() => props.modelValue, (open) => { if (open) { Object.assign(form, { amount: null, date: today(), method: 'cash', note: '' }); error.value = ''; Object.keys(errors).forEach((item) => delete errors[item]); reset() } })
function close() { emit('update:modelValue', false) }
async function save() { saving.value = true; try { const response = await createPayout(props.doctor.id, { ...form, amount: decimal(form.amount) }, key.value); ui.showSnackbar('تم تسجيل دفعة الطبيب وتحديث المستحق المتبقي.'); emit('saved', response.data); close() } catch (apiError) { error.value = apiError.message; Object.assign(errors, apiError.errors) } finally { saving.value = false } }
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: 22px; font-weight: 600; }.dialog-content { display: grid; gap: 16px; padding: 12px 24px !important; }.due-row { display: flex; justify-content: space-between; padding: 14px; border-radius: 12px; background: var(--color-info-bg); }.due-row :deep(.money-text) { color: var(--color-primary); font-size: 20px; font-weight: 600; }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
</style>
