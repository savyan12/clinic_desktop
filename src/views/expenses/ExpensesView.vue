<template>
  <div class="page-container page-stack">
    <PageHeader
      title="المشتريات والإيجار"
      subtitle="تسجيل المصروفات المدفوعة فعليًا مع حفظ التصحيحات والإلغاءات"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openCreate">
          إضافة مصروف
        </v-btn>
        <v-btn to="/reports/expenses" variant="outlined" prepend-icon="mdi-chart-box-outline">
          عرض التقرير
        </v-btn>
      </template>
    </PageHeader>

    <section class="expense-summary" aria-label="ملخص المصروفات النشطة">
      <div>
        <span>إجمالي المشتريات</span>
        <MoneyText :value="summary.total_purchases" />
      </div>
      <div>
        <span>إجمالي الإيجار</span>
        <MoneyText :value="summary.total_rent" />
      </div>
      <div class="summary-total">
        <span>إجمالي المصروفات</span>
        <MoneyText :value="summary.total_expenses" />
      </div>
    </section>

    <section class="surface-card expense-filters" data-form-flow-filter>
      <v-text-field
        v-model="filters.search"
        label="بحث في الوصف أو المستلم أو المرجع"
        prepend-inner-icon="mdi-magnify"
        clearable
        @update:model-value="searchChanged"
      />
      <v-text-field v-model="filters.from" label="من تاريخ" type="date" />
      <v-text-field v-model="filters.to" label="إلى تاريخ" type="date" />
      <v-select v-model="filters.type" label="نوع المصروف" :items="expenseTypes" clearable />
      <v-select v-model="filters.method" label="طريقة الدفع" :items="paymentMethods" clearable />
      <v-select v-model="filters.status" label="الحالة" :items="expenseStatuses" clearable />
      <v-btn color="primary" prepend-icon="mdi-filter-outline" @click="applyFilters">تطبيق</v-btn>
      <v-btn variant="text" @click="clearFilters">مسح</v-btn>
    </section>

    <DataTableCard
      :headers="headers"
      :items="expenses"
      :loading="loading"
      :error="error"
      :meta="meta"
      :empty="{
        icon: 'mdi-receipt-text-minus-outline',
        title: 'لا توجد مصروفات مسجلة',
        description: 'أضف أول عملية شراء أو دفعة إيجار مدفوعة.',
        actionLabel: 'إضافة مصروف',
      }"
      @retry="load"
      @page="changePage"
      @empty-action="openCreate"
    >
      <template #rows="{ items }">
        <tr v-for="expense in items" :key="expense.id">
          <td>{{ formatDate(expense.payment_date) }}</td>
          <td><v-chip size="small" variant="tonal">{{ labelFor('expenseType', expense.type) }}</v-chip></td>
          <td class="expense-description-cell">
            <strong :title="expense.description">{{ expense.description }}</strong>
          </td>
          <td>{{ expense.recipient }}</td>
          <td><bdi dir="auto">{{ expense.reference_number || '—' }}</bdi></td>
          <td>{{ labelFor('paymentMethod', expense.payment_method) }}</td>
          <td><MoneyText :value="expense.amount" /></td>
          <td><StatusChip type="expense" :value="expense.status" /></td>
          <td>
            <v-menu>
              <template #activator="{ props }">
                <v-btn v-bind="props" icon="mdi-dots-vertical" variant="text" aria-label="إجراءات المصروف" />
              </template>
              <v-list density="compact">
                <v-list-item prepend-icon="mdi-eye-outline" title="عرض التفاصيل" @click="openDetails(expense)" />
                <template v-if="expense.status === 'active'">
                  <v-divider />
                  <v-list-item prepend-icon="mdi-pencil-outline" title="تصحيح السجل" @click="openEdit(expense)" />
                  <v-list-item prepend-icon="mdi-cancel" title="إلغاء المصروف" class="text-error" @click="openCancel(expense)" />
                </template>
              </v-list>
            </v-menu>
          </td>
        </tr>
      </template>
    </DataTableCard>
  </div>

  <v-dialog v-model="detailsDialog" max-width="760">
    <v-card v-if="detailsExpense" class="expense-details-card">
      <v-card-title class="expense-details-title">
        <div class="expense-details-heading">
          <span class="expense-details-icon">
            <v-icon :icon="detailsExpense.type === 'rent' ? 'mdi-home-city-outline' : 'mdi-cart-outline'" />
          </span>
          <div>
            <h2>تفاصيل المصروف</h2>
            <span><bdi dir="ltr">{{ detailsExpense.reference_number || '—' }}</bdi></span>
          </div>
        </div>
        <StatusChip type="expense" :value="detailsExpense.status" />
      </v-card-title>

      <v-card-text class="expense-details-content">
        <section class="expense-details-summary">
          <div>
            <span>وصف المصروف</span>
            <strong>{{ detailsExpense.description }}</strong>
          </div>
          <div class="expense-details-amount">
            <span>المبلغ المدفوع</span>
            <MoneyText :value="detailsExpense.amount" />
          </div>
        </section>

        <section class="expense-details-grid" aria-label="بيانات المصروف">
          <div>
            <span>نوع المصروف</span>
            <strong>{{ labelFor('expenseType', detailsExpense.type) }}</strong>
          </div>
          <div>
            <span>تاريخ الدفع</span>
            <strong>{{ formatDate(detailsExpense.payment_date) }}</strong>
          </div>
          <div>
            <span>{{ detailsExpense.type === 'purchase' ? 'اسم المورد' : 'المستلم' }}</span>
            <strong>{{ detailsExpense.recipient || '—' }}</strong>
          </div>
          <div>
            <span>طريقة الدفع</span>
            <strong>{{ labelFor('paymentMethod', detailsExpense.payment_method) }}</strong>
          </div>
          <div>
            <span>رقم المرجع</span>
            <strong><bdi dir="ltr">{{ detailsExpense.reference_number || '—' }}</bdi></strong>
          </div>
          <div>
            <span>الحالة</span>
            <strong>{{ labelFor('expenseStatus', detailsExpense.status) }}</strong>
          </div>
        </section>

        <section v-if="detailsExpense.type === 'rent'" class="rent-details-panel">
          <h3><v-icon icon="mdi-calendar-month-outline" /> بيانات الإيجار</h3>
          <div class="rent-details-grid">
            <div>
              <span>شهر الإيجار</span>
              <strong>{{ formatMonth(detailsExpense.rent_month) }}</strong>
            </div>
            <div>
              <span>بداية فترة الإيجار</span>
              <strong>{{ formatDate(detailsExpense.rent_start_date) }}</strong>
            </div>
            <div>
              <span>نهاية فترة الإيجار</span>
              <strong>{{ formatDate(detailsExpense.rent_end_date) }}</strong>
            </div>
            <div v-if="!detailsExpense.rent_start_date && detailsExpense.rent_period">
              <span>فترة الإيجار</span>
              <strong>{{ detailsExpense.rent_period }}</strong>
            </div>
          </div>
        </section>

        <section v-if="detailsExpense.notes" class="expense-details-note">
          <span>الملاحظات</span>
          <p>{{ detailsExpense.notes }}</p>
        </section>

        <section v-if="detailsExpense.status === 'cancelled'" class="expense-cancellation-details">
          <div>
            <v-icon icon="mdi-cancel" />
            <strong>بيانات الإلغاء</strong>
          </div>
          <p>{{ detailsExpense.cancel_reason || 'لم يُسجل سبب للإلغاء.' }}</p>
          <span v-if="detailsExpense.cancelled_at">أُلغي في {{ formatDateTime(detailsExpense.cancelled_at) }}</span>
        </section>

        <p class="expense-details-meta">
          سُجل في {{ formatDateTime(detailsExpense.created_at) }}
          <template v-if="detailsExpense.updated_at"> · آخر تحديث {{ formatDateTime(detailsExpense.updated_at) }}</template>
        </p>
      </v-card-text>

      <v-card-actions class="dialog-actions">
        <v-btn variant="text" @click="detailsDialog = false">إغلاق</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>

  <v-dialog v-model="editorDialog" persistent max-width="760">
    <v-card>
      <v-form @submit.prevent="saveExpense">
        <v-card-title class="dialog-title">
          {{ editing ? 'تصحيح المصروف' : 'إضافة مصروف جديد' }}
        </v-card-title>
        <v-card-text class="dialog-content">
          <InlineError v-if="dialogError" :message="dialogError" />
          <div class="form-grid">
            <v-select
              v-model="form.type"
              label="نوع المصروف"
              :items="expenseTypes"
              :error-messages="fieldErrors.type"
              @update:model-value="expenseTypeChanged"
            />
            <v-text-field
              v-model="form.payment_date"
              label="تاريخ الدفع"
              type="date"
              :max="today"
              :error-messages="fieldErrors.payment_date"
            />
            <v-text-field
              v-model="form.description"
              label="وصف المصروف"
              :placeholder="form.type === 'rent' ? rentDescriptionPlaceholder : undefined"
              :persistent-placeholder="form.type === 'rent'"
              :error-messages="fieldErrors.description"
            />
            <v-text-field
              v-model="form.amount"
              label="المبلغ المدفوع"
              inputmode="decimal"
              suffix="د.ل"
              :error-messages="fieldErrors.amount"
            />
            <v-text-field
              v-if="form.type === 'purchase'"
              v-model="form.recipient"
              label="اسم المورد"
              :error-messages="fieldErrors.recipient"
            />
            <v-select
              v-model="form.payment_method"
              label="طريقة الدفع"
              :items="paymentMethods"
              :error-messages="fieldErrors.payment_method"
            />
            <v-text-field
              v-if="form.type === 'rent'"
              v-model="form.rent_month"
              label="شهر الإيجار"
              type="month"
              :error-messages="fieldErrors.rent_month"
            />
            <v-text-field
              v-if="form.type === 'rent'"
              v-model="form.rent_start_date"
              label="بداية فترة الإيجار"
              type="date"
              :max="form.rent_end_date || undefined"
              :error-messages="fieldErrors.rent_start_date"
            />
            <v-text-field
              v-if="form.type === 'rent'"
              v-model="form.rent_end_date"
              label="نهاية فترة الإيجار"
              type="date"
              :min="form.rent_start_date || undefined"
              :error-messages="fieldErrors.rent_end_date"
            />
          </div>
          <v-textarea
            v-model="form.notes"
            label="ملاحظات (اختياري)"
            rows="2"
            :error-messages="fieldErrors.notes"
          />
          <v-textarea
            v-if="editing"
            v-model="form.reason"
            label="سبب التصحيح"
            rows="2"
            :error-messages="fieldErrors.reason"
          />
          <p v-if="editing" class="audit-note">
            سيُحفظ سبب التصحيح والقيم السابقة والجديدة في سجل التدقيق.
          </p>
        </v-card-text>
        <v-card-actions class="dialog-actions">
          <v-btn variant="text" :disabled="saving" @click="editorDialog = false">إلغاء</v-btn>
          <v-btn color="primary" type="submit" :loading="saving">حفظ</v-btn>
        </v-card-actions>
      </v-form>
    </v-card>
  </v-dialog>

  <v-dialog v-model="cancelDialog" persistent max-width="560">
    <v-card>
      <v-card-title class="dialog-title">إلغاء المصروف</v-card-title>
      <v-card-text class="dialog-content">
        <InlineError v-if="cancelError" :message="cancelError" />
        <p class="cancel-copy">
          سيبقى السجل محفوظًا، وسيُستبعد من جميع إجماليات المصروفات النشطة.
        </p>
        <v-textarea
          v-model="cancelReason"
          label="سبب الإلغاء"
          rows="3"
          :error-messages="cancelReasonError"
        />
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="cancelling" @click="cancelDialog = false">رجوع</v-btn>
        <v-btn color="error" :loading="cancelling" @click="confirmCancel">تأكيد الإلغاء</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { cancelExpense, correctExpense, createExpense, listExpenses } from '@/api/expenses'
import DataTableCard from '@/components/common/DataTableCard.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import { decimal } from '@/composables/useMoney'
import { useDebounce } from '@/composables/useDebounce'
import { expenseTypes, labelFor, paymentMethods } from '@/constants/labels'
import { useUiStore } from '@/stores/ui'
import { clinicDate, CLINIC_TIME_ZONE } from '@/utils/date'

const ui = useUiStore()
const debounce = useDebounce()
const today = clinicDate()
const currentMonth = today.slice(0, 7)
const currentMonthName = new Intl.DateTimeFormat('ar-LY', {
  month: 'long',
  timeZone: CLINIC_TIME_ZONE,
}).format(new Date())
const rentDescriptionPlaceholder = `إيجار شهر ${currentMonthName}`
const expenses = ref([])
const meta = ref(null)
const summary = reactive({ total_purchases: '0.000', total_rent: '0.000', total_expenses: '0.000' })
const loading = ref(true)
const error = ref('')
const detailsDialog = ref(false)
const detailsExpense = ref(null)
const editorDialog = ref(false)
const editing = ref(false)
const selected = ref(null)
const saving = ref(false)
const dialogError = ref('')
const fieldErrors = reactive({})
const cancelDialog = ref(false)
const cancelReason = ref('')
const cancelReasonError = ref('')
const cancelError = ref('')
const cancelling = ref(false)
const filters = reactive({ search: '', from: '', to: '', type: null, method: null, status: null, page: 1 })
const form = reactive({
  type: 'purchase',
  description: '',
  recipient: '',
  payment_date: today,
  amount: '',
  payment_method: 'cash',
  rent_month: '',
  rent_start_date: '',
  rent_end_date: '',
  notes: '',
  reason: '',
})
const expenseStatuses = [
  { title: 'نشط', value: 'active' },
  { title: 'ملغي', value: 'cancelled' },
]
const headers = [
  { title: 'تاريخ الدفع', key: 'payment_date' },
  { title: 'النوع', key: 'type' },
  { title: 'الوصف', key: 'description' },
  { title: 'المستلم', key: 'recipient' },
  { title: 'المرجع', key: 'reference' },
  { title: 'طريقة الدفع', key: 'method' },
  { title: 'المبلغ', key: 'amount' },
  { title: 'الحالة', key: 'status' },
  { title: 'الإجراءات', key: 'actions' },
]

onMounted(load)

async function load() {
  loading.value = true
  error.value = ''
  try {
    const response = await listExpenses(cleanParams({ ...filters }))
    expenses.value = response.data
    meta.value = response.meta
    Object.assign(summary, response.summary)
  } catch (apiError) {
    error.value = apiError.message
  } finally {
    loading.value = false
  }
}

function openCreate() {
  editing.value = false
  selected.value = null
  Object.assign(form, {
    type: 'purchase', description: '', recipient: '', payment_date: today, amount: '',
    payment_method: 'cash', rent_month: '', rent_start_date: '', rent_end_date: '', notes: '', reason: '',
  })
  resetDialogErrors()
  editorDialog.value = true
}

function openDetails(expense) {
  detailsExpense.value = expense
  detailsDialog.value = true
}

function openEdit(expense) {
  editing.value = true
  selected.value = expense
  Object.assign(form, {
    type: expense.type,
    description: expense.description,
    recipient: expense.type === 'purchase' ? expense.recipient : '',
    payment_date: expense.payment_date,
    amount: expense.amount,
    payment_method: expense.payment_method,
    rent_month: expense.rent_month || '',
    rent_start_date: expense.rent_start_date || '',
    rent_end_date: expense.rent_end_date || '',
    notes: expense.notes || '',
    reason: '',
  })
  resetDialogErrors()
  editorDialog.value = true
}

async function saveExpense() {
  resetDialogErrors()
  if (form.type === 'rent') {
    if (!form.rent_month) fieldErrors.rent_month = ['شهر الإيجار مطلوب.']
    if (!form.rent_start_date) fieldErrors.rent_start_date = ['تاريخ بداية الإيجار مطلوب.']
    if (!form.rent_end_date) fieldErrors.rent_end_date = ['تاريخ نهاية الإيجار مطلوب.']
    if (form.rent_start_date && form.rent_end_date && form.rent_end_date < form.rent_start_date) {
      fieldErrors.rent_end_date = ['تاريخ نهاية الإيجار يجب ألا يسبق تاريخ البداية.']
    }
  }
  if (editing.value && !form.reason.trim()) fieldErrors.reason = ['سبب التصحيح مطلوب.']
  if (Object.keys(fieldErrors).length) return

  saving.value = true
  try {
    const payload = {
      type: form.type,
      description: form.description,
      recipient: form.type === 'rent' ? 'صاحب العقار' : form.recipient,
      payment_date: form.payment_date,
      amount: decimal(form.amount),
      payment_method: form.payment_method,
      rent_month: form.type === 'rent' ? form.rent_month : null,
      rent_start_date: form.type === 'rent' ? form.rent_start_date : null,
      rent_end_date: form.type === 'rent' ? form.rent_end_date : null,
      notes: form.notes || null,
      ...(editing.value ? { reason: form.reason } : {}),
    }
    if (editing.value) await correctExpense(selected.value.id, payload)
    else await createExpense(payload)
    ui.showSnackbar(editing.value ? 'تم تصحيح المصروف وحفظ التغيير في سجل التدقيق.' : 'تم تسجيل المصروف.')
    editorDialog.value = false
    await load()
  } catch (apiError) {
    dialogError.value = apiError.message
    Object.assign(fieldErrors, apiError.errors)
  } finally {
    saving.value = false
  }
}

function openCancel(expense) {
  selected.value = expense
  cancelReason.value = ''
  cancelReasonError.value = ''
  cancelError.value = ''
  cancelDialog.value = true
}

async function confirmCancel() {
  cancelReasonError.value = ''
  cancelError.value = ''
  if (!cancelReason.value.trim()) {
    cancelReasonError.value = 'سبب الإلغاء مطلوب.'
    return
  }
  cancelling.value = true
  try {
    await cancelExpense(selected.value.id, { reason: cancelReason.value })
    ui.showSnackbar('تم إلغاء المصروف واستبعاده من الإجماليات.')
    cancelDialog.value = false
    await load()
  } catch (apiError) {
    cancelError.value = apiError.message
    cancelReasonError.value = apiError.errors?.reason?.[0] || ''
  } finally {
    cancelling.value = false
  }
}

function expenseTypeChanged(value) {
  form.recipient = ''
  form.rent_month = value === 'rent' ? currentMonth : ''
  form.rent_start_date = ''
  form.rent_end_date = ''
  for (const key of ['recipient', 'rent_month', 'rent_start_date', 'rent_end_date']) delete fieldErrors[key]
}

function searchChanged() {
  filters.page = 1
  debounce(load)
}

function applyFilters() {
  filters.page = 1
  load()
}

function clearFilters() {
  Object.assign(filters, { search: '', from: '', to: '', type: null, method: null, status: null, page: 1 })
  load()
}

function changePage(page) {
  filters.page = page
  load()
}

function resetDialogErrors() {
  dialogError.value = ''
  Object.keys(fieldErrors).forEach((key) => delete fieldErrors[key])
}

function cleanParams(params) {
  return Object.fromEntries(Object.entries(params).filter(([, value]) => value !== null && value !== ''))
}

function formatDate(value) {
  if (!value) return '—'
  return new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(`${value}T00:00:00`))
}

function formatMonth(value) {
  if (!/^\d{4}-\d{2}$/.test(value || '')) return value || '—'
  return new Intl.DateTimeFormat('ar-LY-u-nu-latn', {
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  }).format(new Date(`${value}-01T00:00:00Z`))
}

function formatDateTime(value) {
  if (!value) return '—'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return String(value)
  return new Intl.DateTimeFormat('ar-LY-u-nu-latn', {
    dateStyle: 'medium',
    timeStyle: 'short',
    timeZone: CLINIC_TIME_ZONE,
  }).format(date)
}
</script>

<style scoped>
.expense-summary { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-card); background: var(--color-canvas); }
.expense-summary > div { display: grid; gap: 6px; padding: 22px; border-inline-start: 1px solid var(--color-hairline); }
.expense-summary > div:first-child { border-inline-start: 0; }
.expense-summary span { color: var(--color-ink-muted); font-size: 13px; }
.expense-summary :deep(.money-text) { font-size: 25px; font-weight: 600; }
.expense-summary .summary-total { background: var(--color-primary-subtle); }
.expense-filters { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 16px; align-items: end; }
.cell-caption { display: block; margin-top: 4px; color: var(--color-ink-muted); font-size: 12px; }
.expense-description-cell { max-width: 260px; }
.expense-description-cell strong { display: -webkit-box; overflow: hidden; line-height: 1.45; overflow-wrap: anywhere; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.expense-details-card { overflow: hidden; }
.expense-details-title { display: flex; align-items: center; justify-content: space-between; gap: 18px; padding: 22px 24px 18px; border-bottom: 1px solid var(--color-hairline); white-space: normal; }
.expense-details-heading { display: flex; align-items: center; gap: 12px; min-width: 0; }
.expense-details-heading h2 { margin: 0; font-size: 21px; }
.expense-details-heading span { color: var(--color-ink-muted); font-size: 12px; }
.expense-details-icon { width: 44px; height: 44px; display: grid; flex: 0 0 auto; place-items: center; border-radius: var(--radius-md); background: var(--color-primary-subtle); border: 1px solid var(--color-primary-border); color: var(--color-primary); }
.expense-details-icon :deep(.v-icon) { font-size: 24px; }
.expense-details-content { display: grid; gap: 18px; padding: 22px 24px 8px !important; }
.expense-details-summary { display: grid; grid-template-columns: minmax(0, 1.6fr) minmax(180px, .8fr); overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-field); background: var(--color-canvas); }
.expense-details-summary > div { min-width: 0; padding: 17px 18px; }
.expense-details-summary > div + div { border-inline-start: 1px solid var(--color-hairline); }
.expense-details-summary span, .expense-details-grid span, .rent-details-grid span, .expense-details-note > span { display: block; margin-bottom: 5px; color: var(--color-ink-muted); font-size: 12px; }
.expense-details-summary strong { display: block; overflow-wrap: anywhere; font-size: 16px; }
.expense-details-amount { background: var(--color-primary-subtle); }
.expense-details-amount :deep(.money-text) { color: var(--color-primary); font-size: 23px; font-weight: 700; }
.expense-details-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
.expense-details-grid > div, .rent-details-grid > div { min-width: 0; padding: 13px 14px; border: 1px solid var(--color-hairline); border-radius: var(--radius-field); background: var(--color-pearl); }
.expense-details-grid strong, .rent-details-grid strong { display: block; overflow-wrap: anywhere; font-size: 14px; }
.rent-details-panel { display: grid; gap: 12px; padding: 16px; border: 1px solid var(--color-primary-border); border-radius: var(--radius-field); background: var(--color-primary-subtle); }
.rent-details-panel h3 { display: flex; align-items: center; gap: 7px; margin: 0; color: var(--color-primary); font-size: 15px; }
.rent-details-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
.rent-details-grid > div { background: var(--color-canvas); }
.expense-details-note { padding: 14px 16px; border: 1px solid var(--color-hairline); border-radius: var(--radius-field); background: var(--color-pearl); }
.expense-details-note p { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
.expense-cancellation-details { display: grid; gap: 6px; padding: 14px 16px; border: 1px solid var(--color-danger-border); border-radius: var(--radius-field); background: var(--color-danger-bg); }
.expense-cancellation-details > div { display: flex; align-items: center; gap: 7px; color: var(--color-danger); }
.expense-cancellation-details p { margin: 0; overflow-wrap: anywhere; }
.expense-cancellation-details > span { color: var(--color-ink-muted); font-size: 12px; }
.expense-details-meta { margin: 0; color: var(--color-ink-muted); font-size: 13px; }
.dialog-title { padding: 24px 24px 4px; font-size: var(--font-section-size); font-weight: 600; }
.dialog-content { display: grid; gap: 16px; padding: 16px 24px !important; }
.form-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
.audit-note, .cancel-copy { margin: 0; color: var(--color-ink-muted); font-size: 13px; }
@media (max-width: 760px) { .expense-summary, .form-grid, .expense-details-summary, .expense-details-grid, .rent-details-grid { grid-template-columns: 1fr; } .expense-summary > div, .expense-details-summary > div + div { border-inline-start: 0; border-top: 1px solid var(--color-hairline); } .expense-summary > div:first-child, .expense-details-summary > div:first-child { border-top: 0; } .expense-details-title { align-items: flex-start; } }
</style>
