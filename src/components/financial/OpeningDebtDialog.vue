<template>
  <v-dialog :model-value="modelValue" persistent max-width="540" @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">إضافة دين افتتاحي</v-card-title>
      <v-card-text class="dialog-content">
        <p>يُستخدم فقط لإدخال رصيد دين سابق عند بدء تشغيل النظام.</p>
        <InlineError v-if="error" :message="error" />
        <InlineError v-if="optionsError" :message="optionsError" retry @retry="loadOptions" />
        <v-text-field v-model="form.amount" label="المبلغ" type="text" inputmode="decimal" suffix="د.ل" :error-messages="errors.amount" />
        <v-select v-model="form.sponsor_id" label="الجهة الراعية (اختياري)" :items="sponsors || []" item-title="name" item-value="id" clearable :loading="loadingOptions" :disabled="loadingOptions || Boolean(optionsError)" />
        <v-textarea v-model="form.note" label="ملاحظة أو مرجع" rows="2" />
      </v-card-text>
      <v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn><v-btn color="primary" data-form-flow-primary :loading="saving" @click="save">تأكيد الإضافة</v-btn></v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { reactive, ref, watch } from 'vue'
import { createOpeningDebt } from '@/api/financial'
import { getSponsorOptions } from '@/api/sponsors'
import { useDialogOptions } from '@/composables/useDialogOptions'
import InlineError from '@/components/common/InlineError.vue'
import { decimal } from '@/composables/useMoney'
import { useIdempotency } from '@/composables/useIdempotency'
import { useUiStore } from '@/stores/ui'

const props = defineProps({ modelValue: Boolean, patientId: { type: [Number, String], required: true } })
const emit = defineEmits(['update:modelValue', 'saved']); const ui = useUiStore(); const { key, reset } = useIdempotency()
const form = reactive({ amount: null, sponsor_id: null, note: '' }); const error = ref(''); const errors = reactive({}); const saving = ref(false)
const { data: sponsors, loading: loadingOptions, error: optionsError, load: loadOptions } = useDialogOptions(() => props.modelValue, getSponsorOptions)
watch(() => props.modelValue, (open) => { if (open) { Object.assign(form, { amount: null, sponsor_id: null, note: '' }); error.value = ''; Object.keys(errors).forEach((item) => delete errors[item]); reset() } })
function close() { emit('update:modelValue', false) }
async function save() { saving.value = true; try { const response = await createOpeningDebt(props.patientId, { ...form, amount: decimal(form.amount) }, key.value); ui.showSnackbar('تم تسجيل الدين الافتتاحي مع الحفاظ على سجل مستقل.'); emit('saved', response.data); close() } catch (apiError) { error.value = apiError.message; Object.assign(errors, apiError.errors) } finally { saving.value = false } }
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: var(--font-section-size); font-weight: 600; }.dialog-content { display: grid; gap: 16px; padding: 12px 24px !important; }.dialog-content p { margin: 0; color: var(--color-ink-muted); }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
</style>
