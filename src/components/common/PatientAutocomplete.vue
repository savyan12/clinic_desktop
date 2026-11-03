<template>
  <div class="patient-autocomplete">
    <v-autocomplete
      v-model="selectedId"
      v-model:search="search"
      :items="suggestions"
      item-title="name"
      item-value="id"
      :loading="loading"
      :disabled="disabled"
      label="المريض"
      placeholder="ابحث بالاسم أو رقم الهاتف"
      prepend-inner-icon="mdi-magnify"
      clearable
      no-filter
      :error-messages="errorMessages"
      @update:model-value="emitSelection"
      @update:focused="loadOnFocus"
    >
      <template #item="{ props: itemProps, item }">
        <v-list-item v-bind="itemProps">
          <template #subtitle><span class="phone-value">{{ item.raw.phone }}</span></template>
          <template #append><span class="caption-text">{{ item.raw.area }}</span></template>
        </v-list-item>
      </template>
    </v-autocomplete>
    <InlineError v-if="loadError" :message="loadError" retry @retry="loadPatients" />
    <div v-if="showSummary && selected" class="patient-summary">
      <div><span>الهاتف</span><strong class="phone-value">{{ selected.phone }}</strong></div>
      <div><span>الرصيد</span><MoneyText :value="selected.wallet_balance" /></div>
      <div><span>الديون</span><MoneyText :value="selected.debt_total" /></div>
      <div><span>الجهة الراعية</span><strong>{{ selected.sponsor?.name || 'لا توجد' }}</strong></div>
    </div>
    <v-btn v-if="showAdd" variant="text" prepend-icon="mdi-account-plus-outline" @click="$emit('add')">إضافة مريض جديد</v-btn>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listPatients } from '@/api/patients'
import { useDebounce } from '@/composables/useDebounce'
import MoneyText from './MoneyText.vue'
import InlineError from './InlineError.vue'

const props = defineProps({ modelValue: { type: [Number, String], default: null }, initialPatient: { type: Object, default: null }, errorMessages: { type: [Array, String], default: () => [] }, showAdd: { type: Boolean, default: true }, showSummary: { type: Boolean, default: true }, disabled: Boolean })
const emit = defineEmits(['update:modelValue', 'select', 'add'])
const patients = ref([])
const selectedId = ref(props.modelValue ? Number(props.modelValue) : null)
const search = ref('')
const loading = ref(false)
const loadError = ref('')
const chosenPatient = ref(props.initialPatient)
let activeRequest
const debounce = useDebounce(250)
const suggestions = computed(() => chosenPatient.value && !patients.value.some((item) => item.id === chosenPatient.value.id)
  ? [chosenPatient.value, ...patients.value] : patients.value)
const selected = computed(() => suggestions.value.find((item) => item.id === Number(selectedId.value)) || null)

async function loadPatients() {
  activeRequest?.abort()
  const controller = new AbortController()
  activeRequest = controller
  loading.value = true
  loadError.value = ''
  const query = search.value === selected.value?.name ? '' : search.value
  try {
    const response = await listPatients({ search: query, per_page: 30 }, controller.signal)
    if (!controller.signal.aborted) patients.value = response.data
  } catch (cause) {
    if (!controller.signal.aborted) loadError.value = cause.message || 'تعذر البحث عن المرضى. أعد المحاولة.'
  } finally {
    if (!controller.signal.aborted) loading.value = false
  }
}
function emitSelection(value) {
  chosenPatient.value = suggestions.value.find((item) => item.id === Number(value)) || null
  emit('update:modelValue', value)
  emit('select', chosenPatient.value)
}
function loadOnFocus(focused) {
  if (focused && !patients.value.length && !loading.value) loadPatients()
}
watch(search, (value) => {
  activeRequest?.abort()
  loading.value = false
  loadError.value = ''
  debounce(() => {
    if (props.disabled || value === selected.value?.name || (!value && selectedId.value)) return
    loadPatients()
  })
})
watch(() => props.modelValue, (value) => {
  selectedId.value = value ? Number(value) : null
  if (!value) chosenPatient.value = null
})
watch(() => props.initialPatient, (patient) => {
  if (patient && patient.id === Number(props.modelValue)) chosenPatient.value = patient
})
onMounted(() => { if (!selectedId.value && !props.disabled) loadPatients() })
onBeforeUnmount(() => activeRequest?.abort())
</script>

<style scoped>
.patient-autocomplete { display: grid; gap: 10px; }
.patient-summary { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; padding: 14px; border: 1px solid var(--color-hairline); border-radius: var(--radius-field); background: var(--color-pearl); }
.patient-summary div { min-width: 0; }
.patient-summary span, .patient-summary strong { display: block; }
.patient-summary span { color: var(--color-ink-muted); font-size: 12px; }
.patient-summary strong { margin-top: 2px; font-size: 14px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
@media (max-width: 640px) { .patient-summary { grid-template-columns: repeat(2, 1fr); } }
</style>
