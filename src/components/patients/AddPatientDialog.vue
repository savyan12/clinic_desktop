<template>
  <v-dialog :model-value="modelValue" persistent max-width="720" @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-heading">{{ dialogTitle }}</v-card-title>
      <v-card-text class="dialog-body">
        <p class="dialog-intro">{{ dialogIntro }}</p>
        <InlineError v-if="error" :message="error" />
        <InlineError v-if="optionsError" :message="optionsError" retry @retry="loadOptions" />
        <div v-if="duplicateWarning" class="duplicate-warning">
          <v-icon icon="mdi-account-alert-outline" />
          <div><strong>احتمال وجود سجل مكرر</strong><span>يوجد مريض برقم الهاتف نفسه. تحقق من البحث، أو أكد أن هذا مريض مختلف.</span></div>
        </div>
        <div class="form-grid">
          <AppField label="الاسم الكامل" required v-slot="{ id }"><v-text-field :id="id" v-model="form.name" aria-label="الاسم الكامل" :error-messages="errors.name" /></AppField>
          <AppField label="رقم الهاتف" required v-slot="{ id }"><v-text-field :id="id" v-model="form.phone" aria-label="رقم الهاتف" inputmode="tel" dir="ltr" :error-messages="errors.phone" /></AppField>
          <AppField label="العمر (اختياري)" v-slot="{ id }"><v-text-field :id="id" v-model.number="form.age" aria-label="العمر" type="number" min="0" clearable :error-messages="errors.age" /></AppField>
          <AppField label="الجنس (اختياري)" v-slot="{ id }"><v-select :id="id" v-model="form.gender" aria-label="الجنس" :items="genderItems" clearable :error-messages="errors.gender" /></AppField>
          <AppField label="المنطقة أو عنوان السكن (اختياري)" v-slot="{ id }"><v-text-field :id="id" v-model="form.area" aria-label="المنطقة أو عنوان السكن" /></AppField>
          <AppField label="هاتف الطوارئ (اختياري)" v-slot="{ id }"><v-text-field :id="id" v-model="form.emergency_phone" aria-label="هاتف الطوارئ" inputmode="tel" dir="ltr" /></AppField>
          <AppField class="grid-wide" label="الجهة الراعية (اختياري)" v-slot="{ id }"><v-select :id="id" v-model="form.sponsor_id" aria-label="الجهة الراعية" :items="sponsors || []" item-title="name" item-value="id" clearable :loading="loadingOptions" :disabled="loadingOptions || Boolean(optionsError)" /></AppField>
        </div>
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn>
        <v-btn color="primary" data-form-flow-primary :loading="saving" @click="save">{{ duplicateWarning ? 'تأكيد الحفظ' : submitLabel }}</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { createPatient, updatePatient } from '@/api/patients'
import { getSponsorOptions } from '@/api/sponsors'
import { useDialogOptions } from '@/composables/useDialogOptions'
import AppField from '@/components/common/AppField.vue'
import InlineError from '@/components/common/InlineError.vue'
import { useUiStore } from '@/stores/ui'

const props = defineProps({
  modelValue: Boolean,
  patient: { type: Object, default: null },
})
const emit = defineEmits(['update:modelValue', 'saved'])
const ui = useUiStore()
const { data: sponsors, loading: loadingOptions, error: optionsError, load: loadOptions } = useDialogOptions(() => props.modelValue, getSponsorOptions)
const saving = ref(false)
const error = ref('')
const errors = reactive({})
const duplicateWarning = ref(false)
const editMode = computed(() => Boolean(props.patient?.id))
const dialogTitle = computed(() => editMode.value ? 'تعديل بيانات المريض' : 'إضافة مريض جديد')
const dialogIntro = computed(() => editMode.value ? 'حدّث البيانات المطلوبة ثم احفظ التغييرات.' : 'الاسم ورقم الهاتف فقط إلزاميان، وبقية البيانات اختيارية.')
const submitLabel = computed(() => editMode.value ? 'حفظ التعديلات' : 'حفظ المريض')
const initialForm = (patient = null) => ({
  name: patient?.name || '',
  phone: patient?.phone || '',
  age: patient?.age ?? null,
  gender: patient?.gender ?? null,
  area: patient?.area || '',
  emergency_phone: patient?.emergency_phone || '',
  sponsor_id: patient?.sponsor_id ?? patient?.sponsor?.id ?? null,
  confirm_duplicate: false,
})
const form = reactive(initialForm())
const genderItems = [{ title: 'ذكر', value: 'male' }, { title: 'أنثى', value: 'female' }]

watch([() => props.modelValue, () => props.patient], ([open]) => {
  if (!open) return
  Object.assign(form, initialForm(props.patient))
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
    const payload = { ...form, confirm_duplicate: duplicateWarning.value }
    const response = editMode.value
      ? await updatePatient(props.patient.id, payload)
      : await createPatient(payload)
    ui.showSnackbar(editMode.value
      ? 'تم تحديث بيانات المريض ' + response.data.name + ' بنجاح.'
      : 'تمت إضافة المريض ' + response.data.name + ' بنجاح.')
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
