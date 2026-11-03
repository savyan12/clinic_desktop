<template>
  <v-dialog :model-value="modelValue" persistent max-width="820" scrollable @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">إضافة خدمات إلى الجلسة المغلقة</v-card-title>
      <v-card-text class="dialog-body">
        <p>تُضاف بنود جديدة دون تعديل السجل الأصلي. يمكن إضافة خدمة كاملة أو وحدات إضافية فقط مثل أكواب الحجامة.</p>
        <InlineError v-if="error" :message="error" />
        <InlineError v-if="optionsError" :message="optionsError" retry @retry="loadOptions" />
        <v-select v-model="selectedIds" label="الخدمات أو الملحقات" :items="services" item-title="name" item-value="id" multiple chips closable-chips :loading="loadingOptions" :disabled="loadingOptions || Boolean(optionsError)" />
        <div v-if="selectedServices.length" class="service-list">
          <div v-for="service in selectedServices" :key="service.id" class="service-row">
            <div class="service-copy"><strong>{{ service.name }}</strong><span>السعر الأساسي <MoneyText :value="service.price" /></span><span v-if="service.included_units">تشمل {{ service.included_units }} {{ service.unit_name || 'وحدة' }}</span></div>
            <v-select v-if="service.extra_unit_price !== null" v-model="items[service.id].item_type" label="نوع الإضافة" :items="itemTypes" />
            <v-text-field v-if="service.extra_unit_price !== null" v-model.number="items[service.id].extra_units" :label="(service.unit_name || 'وحدة') + ' إضافية'" type="number" :min="items[service.id].item_type === 'extra_units' ? 1 : 0" max="10000" :hint="'سعر الواحدة ' + formatMoney(service.extra_unit_price)" persistent-hint />
            <MoneyText :value="itemTotal(service)" />
          </div>
        </div>
        <InlineError v-if="invalidExtraOnly" message="أدخل وحدة إضافية واحدة على الأقل لكل بند ملحقات." />

        <template v-if="session.type === 'individual' && selectedServices.length">
          <div class="settlement-grid">
            <v-text-field v-model="form.wallet_amount" label="من الرصيد" type="text" inputmode="decimal" suffix="د.ل" :hint="'المتاح ' + formatMoney(session.patient.wallet_balance)" persistent-hint />
            <v-text-field v-model="form.paid_amount" label="مدفوع الآن" type="text" inputmode="decimal" suffix="د.ل" />
            <v-select v-model="form.method" label="طريقة الدفع" :items="paymentMethods" :disabled="numeric(form.paid_amount) <= 0" />
            <v-select v-model="form.sponsor_id" label="الجهة الراعية للدين (اختياري)" :items="sponsors" item-title="name" item-value="id" clearable :disabled="numeric(debtAmount) <= 0" />
          </div>
          <InlineError v-if="walletInsufficient" message="الرصيد المتاح أقل من المبلغ المطلوب. اشحن الرصيد أولاً." />
        </template>

        <div class="summary">
          <span>قيمة الإضافات <MoneyText :value="totalValue" /></span>
          <span v-if="session.type === 'subscription'">مغطاة بالاشتراك دون قبض جديد</span>
          <template v-else><span>مدفوع <MoneyText :value="form.paid_amount" /></span><span>رصيد <MoneyText :value="form.wallet_amount" /></span><span>دين <MoneyText :value="debtAmount" /></span></template>
        </div>
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="saving" @click="close">تراجع</v-btn>
        <v-btn color="primary" prepend-icon="mdi-plus-circle-outline" data-form-flow-primary :loading="saving" :disabled="loadingOptions || Boolean(optionsError) || !selectedServices.length || invalidExtraOnly || walletInsufficient" @click="save">إضافة وتثبيت الخدمات</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { getServiceOptions } from '@/api/services'
import { getSponsorOptions } from '@/api/sponsors'
import { useDialogOptions } from '@/composables/useDialogOptions'
import { addSessionServices } from '@/api/sessions'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import { paymentMethods } from '@/constants/labels'
import { decimal, formatMoney, numeric } from '@/composables/useMoney'
import { useIdempotency } from '@/composables/useIdempotency'

const props = defineProps({ modelValue: Boolean, session: { type: Object, required: true } })
const emit = defineEmits(['update:modelValue', 'saved'])
const { key, reset } = useIdempotency()
const { data: options, loading: loadingOptions, error: optionsError, load: loadOptions } = useDialogOptions(() => props.modelValue, async () => {
  const [services, sponsors] = await Promise.all([getServiceOptions(), getSponsorOptions()])
  return { services, sponsors }
})
const services = computed(() => (options.value?.services || []).filter((item) => item.is_active))
const sponsors = computed(() => (options.value?.sponsors || []).filter((item) => item.is_active))
const selectedIds = ref([])
const items = reactive({})
const saving = ref(false)
const error = ref('')
const form = reactive({ wallet_amount: 0, paid_amount: 0, method: 'cash', sponsor_id: null })
const itemTypes = [{ title: 'خدمة كاملة', value: 'service' }, { title: 'ملحقات / وحدات إضافية فقط', value: 'extra_units' }]
const selectedServices = computed(() => services.value.filter((service) => selectedIds.value.map(Number).includes(service.id)))
const total = computed(() => selectedServices.value.reduce((sum, service) => sum + numeric(itemTotal(service)), 0))
const totalValue = computed(() => decimal(total.value))
const debtAmount = computed(() => decimal(Math.max(0, total.value - numeric(form.wallet_amount) - numeric(form.paid_amount))))
const walletInsufficient = computed(() => props.session.type === 'individual' && numeric(form.wallet_amount) > numeric(props.session.patient.wallet_balance))
const invalidExtraOnly = computed(() => selectedServices.value.some((service) => {
  const item = items[service.id]
  return item?.item_type === 'extra_units' && Number(item.extra_units || 0) < 1
}))

watch(() => props.modelValue, (open) => {
  if (!open) return
  selectedIds.value = []
  Object.keys(items).forEach((id) => delete items[id])
  Object.assign(form, { wallet_amount: 0, paid_amount: 0, method: 'cash', sponsor_id: null })
  error.value = ''
  reset()
})

watch(selectedIds, (ids) => {
  ids.forEach((id) => {
    if (!items[id]) items[id] = { item_type: 'service', extra_units: 0 }
  })
}, { deep: true })
watch(total, (value, previous) => {
  if (props.session.type === 'individual' && (!previous || numeric(form.paid_amount) === numeric(previous))) form.paid_amount = decimal(value)
})

function itemTotal(service) {
  const item = items[service.id] || { item_type: 'service', extra_units: 0 }
  const base = item.item_type === 'service' ? numeric(service.price) : 0
  return decimal(base + numeric(service.extra_unit_price) * Math.max(0, Number(item.extra_units || 0)))
}
function close() { emit('update:modelValue', false) }
async function save() {
  if (invalidExtraOnly.value) return
  saving.value = true
  error.value = ''
  try {
    const payload = {
      services: selectedServices.value.map((service) => ({
        service_id: service.id,
        item_type: items[service.id]?.item_type || 'service',
        extra_units: Math.max(0, Number(items[service.id]?.extra_units || 0)),
      })),
      ...(props.session.type === 'individual' ? {
        settlement: {
          wallet_amount: decimal(form.wallet_amount),
          paid_amount: decimal(form.paid_amount),
          debt_amount: debtAmount.value,
          method: numeric(form.paid_amount) > 0 ? form.method : null,
          sponsor_id: numeric(debtAmount.value) > 0 ? form.sponsor_id : null,
        },
      } : {}),
    }
    const response = await addSessionServices(props.session.id, payload, key.value)
    emit('saved', response.data)
    close()
  } catch (apiError) {
    error.value = apiError.message
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: var(--font-section-size); font-weight: 600; }.dialog-body { display: grid; gap: 18px; padding: 12px 24px !important; }.dialog-body > p { margin: 0; color: var(--color-ink-muted); }
.service-list { display: grid; gap: 1px; overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-field); background: var(--color-hairline); }.service-row { display: grid; grid-template-columns: minmax(180px, 1fr) 180px 180px auto; gap: 12px; align-items: center; padding: 14px; background: var(--color-canvas); }.service-copy, .service-copy span { display: block; }.service-copy span { margin-top: 3px; color: var(--color-ink-muted); font-size: 13px; }
.settlement-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }.summary { display: flex; flex-wrap: wrap; gap: 12px 24px; padding: 16px; border-radius: var(--radius-field); background: var(--color-pearl); }.summary span { display: flex; gap: 6px; }.dialog-actions { justify-content: flex-end; gap: 8px; padding: 12px 24px 24px; }
@media (max-width: 760px) { .service-row, .settlement-grid { grid-template-columns: 1fr; } }
</style>
