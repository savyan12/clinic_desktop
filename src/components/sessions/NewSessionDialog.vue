<template>
  <v-dialog :model-value="modelValue" persistent max-width="980" scrollable @update:model-value="$emit('update:modelValue', $event)">
    <v-card class="session-dialog">
      <v-card-title class="dialog-heading">
        <div><span>تسجيل جلسة جديدة</span><small>تُغلق الجلسة وتُثبت آثارها المالية مباشرة بعد الحفظ.</small></div>
        <v-btn icon="mdi-close" variant="text" aria-label="إغلاق" :disabled="saving" @click="close" />
      </v-card-title>

      <div class="step-indicator">
        <button v-for="item in steps" :key="item.value" type="button" :class="{ active: step === item.value, complete: step > item.value }" @click="goTo(item.value)">
          <span>{{ item.value }}</span>{{ item.label }}
        </button>
      </div>

      <v-card-text ref="dialogBody" class="dialog-body">
        <InlineError v-if="error" :message="error" />
        <InlineError v-if="optionsError" :message="optionsError" retry @retry="loadOptions" />
        <v-window v-model="step">
          <v-window-item :value="1">
            <section class="step-section">
              <div><h3>اختيار المريض</h3><p>الاسم ورقم الهاتف هما البيانات الإلزامية لإضافة مريض جديد.</p></div>
              <PatientAutocomplete v-model="form.patient_id" :initial-patient="patientSelection" :show-summary="false" :error-messages="errors.patient_id" @select="selectPatient" @add="patientDialog = true" />
              <v-skeleton-loader v-if="loadingPatient" type="paragraph" />
              <InlineError v-else-if="patientError" :message="patientError" retry @retry="loadPatient(form.patient_id)" />
              <div v-else-if="patientDetails" class="account-summary">
                <div class="summary-head"><h4>ملخص حساب {{ patientDetails.name }}</h4><span>{{ patientDetails.area || 'لا يوجد عنوان مسجل' }}</span></div>
                <div class="summary-grid" dir="rtl">
                  <div class="summary-item"><span>اشتراكات صالحة</span><strong>{{ activeSubscriptions.length }}</strong></div>
                  <div class="summary-item"><span>الرصيد المتاح</span><MoneyText :value="patientDetails.wallet_balance" /></div>
                  <div class="summary-item"><span>الديون القائمة</span><MoneyText :value="patientDetails.debt_total" /></div>
                  <div class="summary-item"><span>الجهة الراعية</span><strong>{{ patientDetails.sponsor?.name || 'لا توجد' }}</strong></div>
                </div>
                <div v-if="activeSubscriptions.length" class="subscription-preview">
                  <div v-for="subscription in activeSubscriptions" :key="subscription.id">
                    <span>{{ subscriptionSummary(subscription) }}</span>
                    <strong>{{ subscriptionValidity(subscription) }}</strong>
                  </div>
                </div>
              </div>
            </section>
          </v-window-item>

          <v-window-item :value="2">
            <section class="step-section">
              <div><h3>بيانات الجلسة والخدمات</h3><p>يمكن إضافة وحدات إضافية للخدمات التي تدعم ملحقات مثل أكواب الحجامة.</p></div>
              <v-btn-toggle v-model="form.type" mandatory color="primary" variant="outlined" class="type-toggle">
                <v-btn value="individual" prepend-icon="mdi-account-outline">جلسة فردية</v-btn>
                <v-btn value="subscription" prepend-icon="mdi-wallet-membership">مغطاة باشتراك</v-btn>
              </v-btn-toggle>
              <v-select v-model="form.doctor_id" label="الطبيب المسؤول" :items="activeDoctors" item-title="name" item-value="id" :loading="loadingOptions" :disabled="loadingOptions || Boolean(optionsError)" :error-messages="errors.doctor_id" />
              <v-text-field v-model="form.session_date" label="تاريخ الجلسة" type="date" :error-messages="errors.session_date" />
              <v-select v-if="form.type === 'subscription'" v-model="form.subscription_id" label="الاشتراك" :items="activeSubscriptions" :item-title="subscriptionTitle" item-value="id" :error-messages="errors.subscription_id" />

              <template v-if="canChooseServices">
                <v-select v-model="form.service_ids" :label="form.type === 'subscription' ? 'الخدمات المغطاة المقدمة في الجلسة' : 'الخدمات المقدمة في هذه الجلسة'" :items="availableServices" item-title="name" item-value="id" multiple chips closable-chips :loading="loadingOptions" :disabled="loadingOptions || Boolean(optionsError)" :error-messages="errors.service_ids || errors.services" />
                <div v-if="selectedServices.length" class="service-lines">
                  <div v-for="service in selectedServices" :key="service.id" class="service-line">
                    <div class="service-copy">
                      <strong>{{ service.name }}</strong>
                      <span v-if="form.type === 'individual'">السعر الأساسي <MoneyText :value="service.price" /></span>
                      <span v-if="form.type === 'individual' && service.included_units">يشمل {{ service.included_units }} {{ service.unit_name || 'وحدة' }}</span>
                    </div>
                    <v-text-field
                      v-if="form.type === 'individual' && service.extra_unit_price !== null"
                      v-model.number="extraUnits[service.id]"
                      :label="(service.unit_name || 'وحدة') + ' إضافية'"
                      type="number"
                      min="0"
                      max="10000"
                      :hint="extraHint(service)"
                      persistent-hint
                    />
                    <MoneyText v-if="form.type === 'individual'" :value="serviceTotal(service)" />
                    <span v-else class="covered-label">مغطاة بالاشتراك</span>
                  </div>
                </div>
              </template>

              <v-text-field v-if="form.type === 'individual'" v-model="form.discount" label="خصم ثابت" type="text" inputmode="decimal" suffix="د.ل" :error-messages="errors.discount" />
            </section>
          </v-window-item>

          <v-window-item :value="3">
            <section class="step-section">
              <div>
                <h3>التسوية والتأكيد</h3>
                <p v-if="form.type === 'individual'">يمكن الجمع بين الرصيد والدفع الآن، ويُسجل المتبقي دينًا.</p>
                <p v-else>الجلسة مغطاة بالاشتراك ولا تنشئ مقبوضاً أو ديناً جديداً.</p>
              </div>
              <template v-if="form.type === 'individual'">
                <div class="settlement-grid">
                  <v-text-field v-model="form.wallet_amount" label="من الرصيد" type="text" inputmode="decimal" suffix="د.ل" :hint="'المتاح ' + formatMoney(availableWallet)" persistent-hint :error-messages="errors['settlement.wallet_amount']" />
                  <v-text-field v-model="form.paid_amount" label="مدفوع الآن" type="text" inputmode="decimal" suffix="د.ل" />
                  <v-select v-model="form.method" label="طريقة الدفع الآن" :items="paymentMethods" :disabled="numeric(form.paid_amount) <= 0" />
                  <v-select v-model="form.sponsor_id" label="الجهة الراعية للدين (اختياري)" :items="sponsors" item-title="name" item-value="id" clearable :disabled="debtAmount <= 0" />
                </div>
                <div v-if="walletInsufficient" class="wallet-warning"><v-icon icon="mdi-alert-outline" /> الرصيد غير كافٍ. اشحن الرصيد أولًا ثم أعد المحاولة.</div>
              </template>
              <div class="financial-summary">
                <div><span>إجمالي الخدمات</span><MoneyText :value="servicesTotalValue" /></div>
                <div><span>الخصم</span><MoneyText :value="discountValue" /></div>
                <div class="summary-net"><span>صافي قيمة الجلسة</span><MoneyText :value="netTotalValue" /></div>
                <div><span>من الرصيد</span><MoneyText :value="form.type === 'subscription' ? '0.000' : form.wallet_amount" /></div>
                <div><span>مدفوع الآن</span><MoneyText :value="form.type === 'subscription' ? '0.000' : form.paid_amount" /></div>
                <div><span>المتبقي دين</span><MoneyText :value="form.type === 'subscription' ? '0.000' : debtAmountValue" /></div>
              </div>
              <div class="coverage-note"><StatusChip type="coverage" :value="coverage" /><span>{{ coverageExplanation }}</span></div>
            </section>
          </v-window-item>
        </v-window>
      </v-card-text>

      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="step === 1 || saving" @click="step--">السابق</v-btn>
        <v-spacer />
        <v-btn variant="text" :disabled="saving" @click="close">إلغاء</v-btn>
        <v-btn v-if="step < 3" color="primary" data-form-flow-primary :loading="step === 1 ? loadingPatient : loadingOptions" :disabled="step === 1 ? Boolean(patientError) : Boolean(optionsError)" @click="next">التالي</v-btn>
        <v-btn v-else color="primary" prepend-icon="mdi-check-circle-outline" data-form-flow-primary :loading="saving" :disabled="walletInsufficient" @click="save">تسجيل وإغلاق الجلسة</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
  <AddPatientDialog v-model="patientDialog" @saved="patientCreated" />
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { getPatient } from '@/api/patients'
import { getDoctorOptions } from '@/api/doctors'
import { getServiceOptions } from '@/api/services'
import { getSponsorOptions } from '@/api/sponsors'
import { createSession } from '@/api/sessions'
import AddPatientDialog from '@/components/patients/AddPatientDialog.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PatientAutocomplete from '@/components/common/PatientAutocomplete.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import { paymentMethods } from '@/constants/labels'
import { decimal, formatMoney, numeric } from '@/composables/useMoney'
import { useIdempotency } from '@/composables/useIdempotency'
import { useDialogOptions } from '@/composables/useDialogOptions'
import { useUiStore } from '@/stores/ui'
import { clinicDate } from '@/utils/date'

const props = defineProps({ modelValue: Boolean, initialPatientId: { type: [Number, String], default: null }, initialPatient: { type: Object, default: null } })
const emit = defineEmits(['update:modelValue', 'saved'])
const ui = useUiStore()
const { key, reset } = useIdempotency()
const steps = [{ value: 1, label: 'المريض' }, { value: 2, label: 'الجلسة' }, { value: 3, label: 'التسوية' }]
const step = ref(1)
const saving = ref(false)
const error = ref('')
const errors = reactive({})
const extraUnits = reactive({})
const patientDetails = ref(null)
const patientSelection = ref(null)
const loadingPatient = ref(false)
const patientError = ref('')
const dialogBody = ref(null)
let patientRequest
const { data: options, loading: loadingOptions, error: optionsError, load: loadOptions } = useDialogOptions(() => props.modelValue, async () => {
  const [doctors, services, sponsors] = await Promise.all([getDoctorOptions(), getServiceOptions(), getSponsorOptions()])
  return { doctors, services, sponsors }
})
const doctors = computed(() => options.value?.doctors || [])
const services = computed(() => options.value?.services || [])
const sponsors = computed(() => options.value?.sponsors || [])
const patientDialog = ref(false)
const initialForm = () => ({ patient_id: props.initialPatientId ? Number(props.initialPatientId) : null, type: 'individual', doctor_id: null, session_date: clinicDate(), subscription_id: null, service_ids: [], discount: 0, wallet_amount: 0, paid_amount: 0, method: 'cash', sponsor_id: null })
const form = reactive(initialForm())

const activeDoctors = computed(() => doctors.value.filter((item) => item.is_active))
const activeServices = computed(() => services.value.filter((item) => item.is_active))
const activeSubscriptions = computed(() => patientDetails.value?.subscriptions?.filter((item) => item.status === 'active' && item.is_usable !== false && item.remaining > 0) || [])
const selectedSubscription = computed(() => activeSubscriptions.value.find((item) => item.id === Number(form.subscription_id)) || null)
const canChooseServices = computed(() => true)
const availableServices = computed(() => {
  if (form.type !== 'subscription') return activeServices.value
  const coveredIds = new Set((selectedSubscription.value?.services || []).map((service) => Number(service.service_id)))
  return activeServices.value.filter((service) => coveredIds.has(Number(service.id)))
})
const selectedServices = computed(() => availableServices.value.filter((item) => form.service_ids.map(Number).includes(Number(item.id))))
const servicesTotal = computed(() => {
  if (form.type === 'subscription') return 0
  return selectedServices.value.reduce((total, service) => total + numeric(serviceTotal(service)), 0)
})
const discount = computed(() => form.type === 'subscription' ? 0 : Math.max(0, Math.min(numeric(form.discount), servicesTotal.value)))
const netTotal = computed(() => Math.max(0, servicesTotal.value - discount.value))
const availableWallet = computed(() => numeric(patientDetails.value?.wallet_balance))
const debtAmount = computed(() => Math.max(0, netTotal.value - numeric(form.wallet_amount) - numeric(form.paid_amount)))
const walletInsufficient = computed(() => form.type === 'individual' && numeric(form.wallet_amount) > availableWallet.value)
const servicesTotalValue = computed(() => decimal(servicesTotal.value))
const discountValue = computed(() => decimal(discount.value))
const netTotalValue = computed(() => decimal(netTotal.value))
const debtAmountValue = computed(() => decimal(debtAmount.value))
const coverage = computed(() => {
  if (form.type === 'subscription') return 'subscription'
  const wallet = numeric(form.wallet_amount)
  const paid = numeric(form.paid_amount)
  const debt = debtAmount.value
  const used = [wallet > 0, paid > 0, debt > 0].filter(Boolean).length
  if (used > 1) return 'mixed'
  if (wallet > 0) return 'wallet'
  if (debt > 0) return 'debt'
  return form.method === 'transfer' ? 'transfer' : 'cash'
})
const coverageExplanation = computed(() => ({
  subscription: 'ستُغلق الجلسة مباشرة وتُسجل كمغطاة بالاشتراك دون قبض جديد.',
  wallet: 'سيُخصم المبلغ من الرصيد عند تسجيل الجلسة.',
  debt: 'سيُنشأ الدين عند تسجيل الجلسة.',
  mixed: 'سيُوزع الصافي بين الرصيد والمقبوض والدين مباشرة.',
  transfer: 'سيُسجل المقبوض كتحويل مصرفي.',
  cash: 'سيُسجل المقبوض نقدًا.',
})[coverage.value])

watch(() => props.modelValue, (open) => {
  patientRequest?.abort()
  loadingPatient.value = false
  if (!open) return
  Object.assign(form, initialForm())
  patientDetails.value = null
  patientSelection.value = props.initialPatient
  patientError.value = ''
  Object.keys(extraUnits).forEach((id) => delete extraUnits[id])
  step.value = 1
  error.value = ''
  clearErrors()
  reset()
  if (form.patient_id) loadPatient(form.patient_id)
})
onBeforeUnmount(() => patientRequest?.abort())
watch(step, async () => {
  await nextTick()
  dialogBody.value?.$el.scrollTo({ top: 0 })
})

watch(() => form.type, () => {
  form.subscription_id = null
  form.service_ids = []
  form.wallet_amount = 0
  form.paid_amount = 0
})
watch(() => form.subscription_id, () => { form.service_ids = [] })
watch(netTotal, (value, previous) => {
  if (form.type === 'individual' && (!previous || numeric(form.paid_amount) === numeric(previous))) form.paid_amount = value.toFixed(3)
})

function clearErrors() { Object.keys(errors).forEach((name) => delete errors[name]) }
function subscriptionTitle(item) { return item.name + ' — متبقي ' + item.remaining }
function subscriptionSummary(item) { return item.name + ' · ' + item.services.map((service) => service.name).join(' + ') }
function subscriptionValidity(item) { return 'متبقي ' + item.remaining + ' من ' + item.sessions_count }
function serviceTotal(service) { return decimal(numeric(service.price) + numeric(service.extra_unit_price) * Math.max(0, Number(extraUnits[service.id] || 0))) }
function extraHint(service) { return 'سعر الواحدة ' + formatMoney(service.extra_unit_price) }
function servicePayload() { return selectedServices.value.map((service) => ({ service_id: service.id, item_type: 'service', extra_units: form.type === 'subscription' ? 0 : Math.max(0, Number(extraUnits[service.id] || 0)) })) }
async function loadPatient(id) {
  patientRequest?.abort()
  patientDetails.value = null
  patientError.value = ''
  loadingPatient.value = false
  if (!id) return
  const controller = new AbortController()
  patientRequest = controller
  loadingPatient.value = true
  try {
    const response = await getPatient(id, controller.signal)
    if (!controller.signal.aborted) {
      patientDetails.value = response.data
      patientSelection.value = response.data
    }
  } catch (cause) {
    if (!controller.signal.aborted) patientError.value = cause.message || 'تعذر تحميل حساب المريض.'
  } finally {
    if (!controller.signal.aborted) loadingPatient.value = false
  }
}
function selectPatient(patient) { patientSelection.value = patient; loadPatient(patient?.id) }
function patientCreated(patient) { form.patient_id = patient.id; selectPatient(patient) }
function close() { emit('update:modelValue', false) }
function goTo(target) { if (target < step.value) step.value = target }
function next() {
  if (loadingPatient.value || (step.value === 2 && (loadingOptions.value || optionsError.value))) return
  clearErrors()
  error.value = ''
  if (step.value === 1 && !form.patient_id) { errors.patient_id = ['اختر المريض أولًا.']; return }
  if (step.value === 2) {
    if (!form.doctor_id) errors.doctor_id = ['اختر الطبيب المسؤول.']
    if (form.type === 'subscription' && !form.subscription_id) errors.subscription_id = ['اختر الاشتراك.']
    if (canChooseServices.value && !form.service_ids.length) errors.service_ids = ['اختر خدمة واحدة على الأقل.']
    if (Object.keys(errors).length) return
  }
  step.value += 1
}

async function save() {
  saving.value = true
  error.value = ''
  clearErrors()
  try {
    const payload = {
      patient_id: form.patient_id,
      doctor_id: form.doctor_id,
      session_date: form.session_date,
      type: form.type,
      subscription_id: form.type === 'subscription' ? form.subscription_id : null,
      ...(canChooseServices.value ? { services: servicePayload() } : {}),
      discount: discountValue.value,
      ...(form.type === 'individual' ? {
        settlement: {
          coverage: coverage.value,
          wallet_amount: decimal(form.wallet_amount),
          paid_amount: decimal(form.paid_amount),
          debt_amount: debtAmountValue.value,
          method: numeric(form.paid_amount) > 0 ? form.method : null,
          sponsor_id: debtAmount.value > 0 ? form.sponsor_id : null,
        },
      } : {}),
    }
    const response = await createSession(payload, key.value)
    const savedSession = response.data.session
    ui.showSnackbar('تم تسجيل وإغلاق الجلسة ' + savedSession.code + ' وتثبيت آثارها.')
    emit('saved', savedSession)
    close()
  } catch (apiError) {
    error.value = apiError.message
    Object.assign(errors, apiError.errors)
  } finally {
    saving.value = false
  }
}

</script>

<style scoped>
.session-dialog { height: min(800px, calc(100dvh - 48px)); max-height: calc(100dvh - 48px); }
.dialog-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 20px; padding: 22px 24px 14px; border-bottom: 1px solid var(--color-hairline); }
.dialog-heading span, .dialog-heading small { display: block; }.dialog-heading span { font-size: var(--font-section-size); font-weight: var(--font-section-weight); }.dialog-heading small { margin-top: 4px; color: var(--color-ink-muted); font-size: 13px; font-weight: 400; }
.step-indicator { display: grid; grid-template-columns: repeat(3, 1fr); padding: 16px 24px; border-bottom: 1px solid var(--color-hairline); background: var(--color-pearl); }
.step-indicator button { display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 44px; color: var(--color-ink-muted); border: 0; background: transparent; cursor: pointer; }.step-indicator button span { width: 26px; height: 26px; display: grid; place-items: center; border: 1px solid var(--color-hairline); border-radius: 50%; background: var(--color-canvas); }.step-indicator button.active { color: var(--color-primary); font-weight: 600; }.step-indicator button.active span, .step-indicator button.complete span { color: white; border-color: var(--color-primary); background: var(--color-primary); }
.dialog-body { min-height: 0; scrollbar-gutter: stable; padding: 24px !important; }.step-section { display: grid; gap: 20px; }.step-section h3 { margin: 0; font-size: var(--font-section-size); font-weight: var(--font-section-weight); }.step-section p { margin: 4px 0 0; color: var(--color-ink-muted); }
.account-summary, .locked-services, .financial-summary { border: 1px solid var(--color-hairline); border-radius: 14px; overflow: hidden; }.summary-head { display: flex; justify-content: space-between; gap: 12px; padding: 16px; text-align: right; background: var(--color-pearl); }.summary-head h4 { margin: 0; font-weight: 600; }.summary-head span { color: var(--color-ink-muted); }.summary-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); direction: rtl; }.summary-item { min-width: 0; display: flex; flex-direction: column; align-items: flex-start; padding: 16px; direction: rtl; text-align: right; border-inline-start: 1px solid var(--color-hairline); }.summary-item:first-child { border-inline-start: 0; }.summary-item span, .summary-item strong { display: block; width: 100%; text-align: right; }.summary-item span { color: var(--color-ink-muted); font-size: 13px; }.summary-item strong { margin-top: 4px; font-weight: 600; }.summary-item :deep(.money-text) { display: block; width: 100%; margin-top: 4px; text-align: right; }.subscription-preview { display: grid; gap: 1px; direction: rtl; background: var(--color-hairline); }.subscription-preview div { display: flex; justify-content: space-between; gap: 12px; padding: 12px 16px; text-align: right; background: var(--color-canvas); font-size: 14px; }.subscription-preview strong { color: var(--color-primary); font-weight: 600; }
.type-toggle { width: fit-content; }.locked-title, .locked-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 14px 16px; }.locked-title { justify-content: flex-start; color: var(--color-primary); background: var(--color-info-bg); }.locked-row { border-top: 1px solid var(--color-hairline); }
.service-lines { display: grid; gap: 1px; overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-field); background: var(--color-hairline); }.service-line { display: grid; grid-template-columns: minmax(220px, 1fr) minmax(180px, .7fr) auto; align-items: center; gap: 16px; padding: 14px 16px; background: var(--color-canvas); }.service-copy, .service-copy span { display: block; }.service-copy span { margin-top: 3px; color: var(--color-ink-muted); font-size: 13px; }
.settlement-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }.wallet-warning { display: flex; align-items: center; gap: 8px; padding: 12px 14px; color: var(--color-danger); border: 1px solid var(--color-danger-border); border-radius: var(--radius-field); background: var(--color-danger-bg); }
.covered-label { color: var(--color-primary); font-weight: 600; }
.financial-summary { display: grid; grid-template-columns: repeat(3, 1fr); }.financial-summary div { display: flex; flex-direction: column; gap: 4px; padding: 16px; border-inline-start: 1px solid var(--color-hairline); border-top: 1px solid var(--color-hairline); }.financial-summary span { color: var(--color-ink-muted); font-size: 13px; }.financial-summary :deep(.money-text) { font-size: 19px; font-weight: 600; }.summary-net { background: var(--color-info-bg); }.coverage-note { display: flex; align-items: center; gap: 10px; color: var(--color-ink-muted); }.dialog-actions { gap: 8px; padding: 14px 24px 22px; border-top: 1px solid var(--color-hairline); }
@media (max-width: 760px) { .summary-grid, .financial-summary { grid-template-columns: repeat(2, 1fr); }.service-line { grid-template-columns: 1fr; }.settlement-grid { grid-template-columns: 1fr; } } @media (max-width: 560px) { .step-indicator { padding-inline: 8px; }.step-indicator button { font-size: 12px; }.summary-grid, .financial-summary { grid-template-columns: 1fr; } }
</style>
