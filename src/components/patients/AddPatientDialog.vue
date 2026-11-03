<template>
  <v-dialog :model-value="modelValue" persistent max-width="720" @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-heading">إضافة مريض جديد</v-card-title>
      <v-card-text class="dialog-body">
        <p class="dialog-intro">الاسم ورقم الهاتف فقط إلزاميان، وبقية البيانات اختيارية.</p>
        <InlineError v-if="error" :message="error" />
        <InlineError v-if="optionsError" :message="optionsError" retry @retry="loadOptions" />
        <div v-if="duplicateWarning" class="duplicate-warning">
          <v-icon icon="mdi-account-alert-outline" />
          <div><strong>احتمال وجود سجل مكرر</strong><span>يوجد مريض برقم الهاتف نفسه. تحقق من البحث، أو أكد أن هذا مريض مختلف.</span></div>
        </div>
        <div class="form-grid">
          <v-text-field v-model="form.name" label="الاسم الكامل" :error-messages="errors.name" />
          <v-text-field v-model="form.phone" label="رقم الهاتف" inputmode="tel" dir="ltr" :error-messages="errors.phone" />
          <v-text-field v-model.number="form.age" label="العمر (اختياري)" type="number" min="0" clearable :error-messages="errors.age" />
          <v-select v-model="form.gender" label="الجنس (اختياري)" :items="genderItems" clearable :error-messages="errors.gender" />
          <v-text-field v-model="form.area" label="المنطقة أو عنوان السكن (اختياري)" />
          <v-text-field v-model="form.emergency_phone" label="هاتف الطوارئ (اختياري)" inputmode="tel" dir="ltr" />
          <v-select v-model="form.sponsor_id" class="grid-wide" label="الجهة الراعية (اختياري)" :items="sponsors || []" item-title="name" item-value="id" clearable :loading="loadingOptions" :disabled="loadingOptions || Boolean(optionsError)" />
        </div>
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn>
        <v-btn color="primary" data-form-flow-primary :loading="saving" @click="save">{{ duplicateWarning ? 'تأكيد الحفظ' : 'حفظ المريض' }}</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { reactive, ref, watch } from 'vue'
import { createPatient } from '@/api/patients'
import { getSponsorOptions } from '@/api/sponsors'
import { useDialogOptions } from '@/composables/useDialogOptions'
import InlineError from '@/components/common/InlineError.vue'
import { useUiStore } from '@/stores/ui'

const props = defineProps({ modelValue: Boolean })
const emit = defineEmits(['update:modelValue', 'saved'])
const ui = useUiStore()
const { data: sponsors, loading: loadingOptions, error: optionsError, load: loadOptions } = useDialogOptions(() => props.modelValue, getSponsorOptions)
const saving = ref(false)
const error = ref('')
const errors = reactive({})
const duplicateWarning = ref(false)
const initialForm = () => ({ name: '', phone: '', age: null, gender: null, area: '', emergency_phone: '', sponsor_id: null, confirm_duplicate: false })
const form = reactive(initialForm())
const genderItems = [{ title: 'ذكر', value: 'male' }, { title: 'أنثى', value: 'female' }]

watch(() => props.modelValue, (open) => {
  if (!open) return
  Object.assign(form, initialForm())
  duplicateWarning.value = false
  error.value = ''
  clearErrors()
})

function clearErrors() { Object.keys(errors).forEach((key) => delete errors[key]) }
function close() { emit('update:modelValue', false) }

async function save() {
  saving.value = true
  error.value = ''
  clearErrors()
  try {
    const response = await createPatient({ ...form, confirm_duplicate: duplicateWarning.value })
    ui.showSnackbar(`تمت إضافة المريض ${response.data.name} بنجاح.`)
    emit('saved', response.data)
    close()
  } catch (apiError) {
    error.value = apiError.message
    Object.assign(errors, apiError.errors)
    if (apiError.errors?.confirm_duplicate) duplicateWarning.value = true
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.dialog-heading { padding: 24px 24px 4px; font-size: var(--font-section-size); font-weight: var(--font-section-weight); }
.dialog-body { display: grid; gap: 18px; padding: 8px 24px 10px !important; }
.dialog-intro { margin: 0; color: var(--color-ink-muted); }
.form-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }
.grid-wide { grid-column: 1 / -1; }
.duplicate-warning { display: flex; gap: 12px; padding: 14px; color: var(--color-warning); border: 1px solid var(--color-warning-border); border-radius: var(--radius-field); background: var(--color-warning-bg); }
.duplicate-warning div { display: grid; gap: 2px; }
.duplicate-warning strong { font-weight: 600; }
.duplicate-warning span { font-size: 14px; }
.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
@media (max-width: 640px) { .form-grid { grid-template-columns: 1fr; } .grid-wide { grid-column: auto; } }
</style>
