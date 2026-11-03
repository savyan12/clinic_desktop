<template>
  <div class="page-container page-stack">
    <template v-if="loading"><v-skeleton-loader type="heading, image, article" /></template>
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="subscription">
      <PageHeader :title="'اشتراك ' + subscription.patient.name" :subtitle="subscriptionSubtitle">
        <template #actions>
          <v-btn :to="'/patients/' + subscription.patient_id" variant="outlined" prepend-icon="mdi-account-outline">ملف المريض</v-btn>
          <template v-if="isAdmin">
            <v-btn variant="outlined" prepend-icon="mdi-pencil-outline" @click="openCorrection">تصحيح</v-btn>
            <v-btn v-if="subscription.status === 'active'" color="warning" variant="outlined" @click="askStop">إيقاف</v-btn>
            <v-btn v-if="subscription.status === 'stopped'" color="primary" @click="askReactivate">إعادة تفعيل</v-btn>
            <v-btn v-if="canCancel" color="error" variant="outlined" prepend-icon="mdi-cancel" @click="cancelDialog = true">إلغاء الاشتراك</v-btn>
          </template>
        </template>
      </PageHeader>

      <div class="subscription-status-banner">
        <StatusChip type="subscription" :value="subscription.status" />
        <span>{{ statusDescription }}</span>
      </div>

      <section v-if="isMonthly" class="usage-overview surface-card">
        <div class="usage-number"><strong>{{ subscription.remaining_days }}</strong><span>يوم متبقٍ</span></div>
        <div class="usage-main">
          <div class="section-row"><span>{{ formatDate(subscription.start_date) }}</span><span>{{ formatDate(subscription.end_date) }}</span></div>
          <div class="usage-track"><span :style="{ width: durationPercent + '%' }"></span></div>
          <div class="schedule-chips"><v-chip v-for="day in scheduledDayLabels" :key="day" size="small" color="primary" variant="tonal">{{ day }}</v-chip></div>
          <p>يغطي الاشتراك جلسة واحدة في كل يوم حضور محدد، وتُختار الخدمات الفعلية داخل الجلسة.</p>
        </div>
      </section>

      <section v-else class="usage-overview surface-card">
        <div class="usage-number"><strong>{{ subscription.remaining }}</strong><span>جلسة متبقية</span></div>
        <div class="usage-main">
          <div class="section-row"><span>{{ subscription.sessions_used }} مستخدمة</span><span>{{ subscription.sessions_count }} إجمالي</span></div>
          <div class="usage-track"><span :style="{ width: legacyUsagePercent + '%' }"></span></div>
          <div class="services-chips"><v-chip v-for="service in subscription.services" :key="service.service_id" variant="outlined">{{ service.name }} · <MoneyText :value="service.price" /></v-chip></div>
        </div>
      </section>

      <section class="detail-grid">
        <div class="detail-item"><span class="detail-label">نوع الاشتراك</span><strong>{{ isMonthly ? 'اشتراك شهري' : 'باقة قديمة محفوظة' }}</strong></div>
        <div v-if="isMonthly" class="detail-item"><span class="detail-label">أيام الحضور</span><strong>{{ scheduleLabel }}</strong></div>
        <div class="detail-item"><span class="detail-label">القيمة</span><MoneyText :value="subscription.net_total" /></div>
        <div class="detail-item"><span class="detail-label">المدفوع</span><MoneyText :value="subscription.paid_amount" /></div>
        <div v-if="subscription.status !== 'cancelled'" class="detail-item"><span class="detail-label">المتبقي المالي</span><MoneyText :value="subscription.remaining_amount" /></div>
        <div v-else class="detail-item"><span class="detail-label">المبلغ المسترجع</span><MoneyText :value="subscription.refund_amount" /></div>
        <div class="detail-item"><span class="detail-label">الجهة الراعية</span><strong>{{ subscription.sponsor?.name || 'لا توجد' }}</strong></div>
      </section>

      <section class="two-column">
        <div class="surface-card">
          <h2 class="section-heading">{{ isMonthly ? 'الجلسات المسجلة خلال الاشتراك' : 'سجل الاستخدام' }}</h2>
          <div v-if="isMonthly && subscription.sessions.length" class="record-list">
            <div v-for="session in subscription.sessions" :key="session.id">
              <div><strong>{{ session.code }} · {{ session.services.map((service) => service.name).join('، ') }}</strong><span>{{ formatDateTime(session.closed_at || session.opened_at) }}</span></div>
              <router-link :to="'/sessions/' + session.id">التفاصيل</router-link>
            </div>
          </div>
          <div v-else-if="!isMonthly && subscription.usages.length" class="record-list">
            <div v-for="usage in subscription.usages" :key="usage.id"><div><strong>{{ usage.type === 'consume' ? 'استهلاك جلسة' : 'إعادة جلسة' }}</strong><span>{{ formatDateTime(usage.created_at) }}</span></div><router-link :to="'/sessions/' + usage.session_id">الجلسة #{{ usage.session_id }}</router-link></div>
          </div>
          <EmptyState v-else icon="mdi-history" title="لا توجد جلسات بعد" description="ستظهر هنا الجلسات المسجلة ضمن الاشتراك." />
        </div>
        <div class="surface-card">
          <h2 class="section-heading">الدفعات والمستحقات</h2>
          <div class="record-list">
            <div v-for="payment in subscription.payments" :key="'p-' + payment.id"><div><strong>{{ payment.type === 'refund' ? 'استرجاع إلغاء الاشتراك' : 'دفعة اشتراك' }}</strong><span>{{ formatDateTime(payment.created_at) }}</span></div><MoneyText :value="payment.amount" /></div>
            <div v-for="debt in subscription.debts" :key="'d-' + debt.id"><div><strong>دين الاشتراك</strong><span><StatusChip type="debt" :value="debt.status" /></span></div><MoneyText :value="debt.remaining" /></div>
          </div>
          <EmptyState v-if="!subscription.payments.length && !subscription.debts.length" title="لا توجد حركات مالية" description="لا توجد دفعات أو ديون مرتبطة بهذا الاشتراك." />
        </div>
      </section>
    </template>
  </div>

  <ConfirmDialog v-model="confirm.open" :title="confirm.title" :message="confirm.message" :confirm-label="confirm.label" :require-reason="confirm.requireReason" :loading="confirm.loading" :danger="confirm.action === 'stop'" @confirm="executeAction" />
  <CancelSubscriptionDialog v-if="subscription" v-model="cancelDialog" :subscription-id="subscription.id" @saved="cancelled" />
  <v-dialog v-model="correctionDialog" persistent max-width="580">
    <v-card>
      <v-card-title class="dialog-title">تصحيح بيانات الاشتراك</v-card-title>
      <v-card-text class="correction-body">
        <p>التصحيح لا يغيّر الحركات المالية أو الجلسات السابقة.</p>
        <InlineError v-if="correctionError" :message="correctionError" />
        <InlineError v-if="optionsError" :message="optionsError" retry @retry="loadOptions" />
        <v-text-field v-model="correction.end_date" :label="isMonthly ? 'تاريخ النهاية' : 'تاريخ النهاية المتوقع'" type="date" />
        <div v-if="isMonthly" class="correction-schedule">
          <span>أيام الحضور</span>
          <v-btn-toggle v-model="correction.scheduled_weekdays" multiple divided color="primary" variant="outlined" class="weekday-toggle" aria-label="تصحيح أيام حضور الاشتراك">
            <v-btn v-for="day in SUBSCRIPTION_WEEKDAYS" :key="day.value" :value="day.value">{{ day.title }}</v-btn>
          </v-btn-toggle>
        </div>
        <v-select v-model="correction.sponsor_id" label="الجهة الراعية" :items="sponsors" item-title="name" item-value="id" clearable :loading="loadingOptions" :disabled="!canCorrectSponsor || loadingOptions || Boolean(optionsError)" :hint="canCorrectSponsor ? 'يمكن التصحيح قبل إنشاء دين على الاشتراك.' : 'لا يمكن تغيير الجهة بعد إنشاء دين.'" persistent-hint />
        <v-textarea v-model="correction.reason" label="سبب التصحيح" rows="3" />
      </v-card-text>
      <v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="correcting" @click="correctionDialog = false">تراجع</v-btn><v-btn color="primary" data-form-flow-primary :loading="correcting" @click="applyCorrection">تأكيد التصحيح</v-btn></v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'
import { getSponsorOptions } from '@/api/sponsors'
import { useDialogOptions } from '@/composables/useDialogOptions'
import { correctSubscription, getSubscription, reactivateSubscription, stopSubscription } from '@/api/subscriptions'
import ConfirmDialog from '@/components/common/ConfirmDialog.vue'
import EmptyState from '@/components/common/EmptyState.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import CancelSubscriptionDialog from '@/components/subscriptions/CancelSubscriptionDialog.vue'
import { useIdempotency } from '@/composables/useIdempotency'
import { usePermissions } from '@/composables/usePermissions'
import { useUiStore } from '@/stores/ui'
import { formatSubscriptionSchedule, normalizeSubscriptionWeekdays, SUBSCRIPTION_WEEKDAYS } from '@/constants/subscriptions'

const route = useRoute()
const ui = useUiStore()
const { isAdmin } = usePermissions()
const { key, reset } = useIdempotency()
const subscription = ref(null)
const loading = ref(true)
const error = ref('')
const cancelDialog = ref(false)
const correctionDialog = ref(false)
const { data: sponsorOptions, loading: loadingOptions, error: optionsError, load: loadOptions } = useDialogOptions(() => correctionDialog.value, getSponsorOptions)
const sponsors = computed(() => (sponsorOptions.value || []).filter((item) => item.is_active))
const correctionError = ref('')
const correcting = ref(false)
const correction = reactive({ end_date: '', sponsor_id: null, scheduled_weekdays: [], reason: '' })
const confirm = reactive({ open: false, action: '', title: '', message: '', label: '', requireReason: false, loading: false })
const isMonthly = computed(() => subscription.value?.plan_type === 'monthly')
const legacyUsagePercent = computed(() => subscription.value?.sessions_count ? Math.min(100, (subscription.value.sessions_used / subscription.value.sessions_count) * 100) : 0)
const durationPercent = computed(() => {
  if (!isMonthly.value) return 0
  const start = new Date(subscription.value.start_date + 'T00:00:00')
  const end = new Date(subscription.value.end_date + 'T00:00:00')
  const today = new Date()
  const total = Math.max(1, end - start)
  return Math.max(0, Math.min(100, ((today - start) / total) * 100))
})
const scheduledDayLabels = computed(() => normalizeSubscriptionWeekdays(subscription.value?.scheduled_weekdays)
  .map((value) => SUBSCRIPTION_WEEKDAYS.find((day) => day.value === value)?.title)
  .filter(Boolean))
const scheduleLabel = computed(() => formatSubscriptionSchedule(subscription.value?.scheduled_weekdays))
const canCorrectSponsor = computed(() => !subscription.value?.debts?.length)
const canCancel = computed(() => ['active', 'stopped'].includes(subscription.value?.status) && (isMonthly.value || subscription.value?.remaining > 0))
const subscriptionSubtitle = computed(() => isMonthly.value
  ? 'من ' + formatDate(subscription.value.start_date) + ' إلى ' + formatDate(subscription.value.end_date)
  : 'بدأ في ' + formatDate(subscription.value.start_date) + ' · النهاية المتوقعة ' + formatDate(subscription.value.expected_end_date))
const statusDescription = computed(() => {
  if (subscription.value.status === 'active') return isMonthly.value ? 'الاشتراك صالح في أيام الحضور المحددة خلال مدته.' : 'الاشتراك صالح ما دام يحتوي على جلسات متبقية.'
  if (subscription.value.status === 'completed') return isMonthly.value ? 'انتهت مدة الاشتراك.' : 'اكتمل الاشتراك باستهلاك جميع الجلسات.'
  if (subscription.value.status === 'stopped') return 'الاشتراك موقوف يدويًا ولا يمكن استخدامه حتى إعادة التفعيل.'
  return 'أُلغي الاشتراك نهائيًا.'
})

onMounted(load)
async function load() {
  loading.value = true
  error.value = ''
  try {
    const subscriptionResponse = await getSubscription(route.params.id)
    subscription.value = subscriptionResponse.data
  } catch (apiError) {
    error.value = apiError.message
  } finally {
    loading.value = false
  }
}
function askStop() { reset(); Object.assign(confirm, { open: true, action: 'stop', title: 'إيقاف الاشتراك', message: 'لن يُعاد مبلغ تلقائيًا. سيبقى السجل المالي كما هو حتى إعادة التفعيل أو الإلغاء.', label: 'إيقاف الاشتراك', requireReason: true }) }
function askReactivate() { reset(); Object.assign(confirm, { open: true, action: 'reactivate', title: 'إعادة تفعيل الاشتراك', message: isMonthly.value ? 'سيصبح الاشتراك متاحًا إذا كانت مدته لم تنتهِ.' : 'سيصبح الاشتراك متاحًا ما دام يحتوي على جلسات.', label: 'إعادة التفعيل', requireReason: false }) }
async function executeAction(reason) {
  confirm.loading = true
  try {
    if (confirm.action === 'stop') await stopSubscription(subscription.value.id, { reason }, key.value)
    else await reactivateSubscription(subscription.value.id, {}, key.value)
    ui.showSnackbar(confirm.action === 'stop' ? 'تم إيقاف الاشتراك.' : 'تمت إعادة تفعيل الاشتراك.')
    confirm.open = false
    await load()
  } catch (apiError) {
    ui.showSnackbar(apiError.message, 'error')
  } finally {
    confirm.loading = false
  }
}
async function cancelled() { ui.showSnackbar('تم إلغاء الاشتراك وتسوية الاسترجاع أو الدين حسب المدة المستهلكة.'); await load() }
function openCorrection() {
  reset()
  Object.assign(correction, { end_date: isMonthly.value ? subscription.value.end_date : (subscription.value.expected_end_date || ''), sponsor_id: subscription.value.sponsor_id || null, scheduled_weekdays: normalizeSubscriptionWeekdays(subscription.value.scheduled_weekdays), reason: '' })
  correctionError.value = ''
  correctionDialog.value = true
}
async function applyCorrection() {
  correcting.value = true
  correctionError.value = ''
  if (isMonthly.value && !normalizeSubscriptionWeekdays(correction.scheduled_weekdays).length) {
    correctionError.value = 'اختر يوم حضور واحدًا على الأقل.'
    correcting.value = false
    return
  }
  try {
    const changes = { [isMonthly.value ? 'end_date' : 'expected_end_date']: correction.end_date || null }
    if (isMonthly.value) changes.scheduled_weekdays = normalizeSubscriptionWeekdays(correction.scheduled_weekdays)
    if (canCorrectSponsor.value) changes.sponsor_id = correction.sponsor_id || null
    await correctSubscription(subscription.value.id, { changes, reason: correction.reason }, key.value)
    ui.showSnackbar('تم تصحيح الاشتراك وتوثيق السبب.')
    correctionDialog.value = false
    await load()
  } catch (apiError) {
    correctionError.value = apiError.message
  } finally {
    correcting.value = false
  }
}
const formatDate = (value) => value ? new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(value + 'T00:00:00')) : 'غير محددة'
const formatDateTime = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
</script>

<style scoped>
.subscription-status-banner { display: flex; align-items: center; gap: 12px; padding: 16px 20px; border: 1px solid var(--color-hairline); border-radius: 14px; background: var(--color-pearl); }.usage-overview { display: grid; grid-template-columns: 180px 1fr; align-items: center; gap: 28px; }.usage-number { text-align: center; }.usage-number strong, .usage-number span { display: block; }.usage-number strong { font-size: 56px; line-height: 1; color: var(--color-primary); }.usage-number span { margin-top: 8px; color: var(--color-ink-muted); }.usage-main { display: grid; gap: 14px; }.usage-main p { margin: 0; color: var(--color-ink-muted); }.section-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; }.usage-track { height: 12px; overflow: hidden; border-radius: 99px; background: var(--color-parchment); }.usage-track span { display: block; height: 100%; background: var(--color-primary); }.services-chips { display: flex; flex-wrap: wrap; gap: 8px; }
.schedule-chips { display: flex; flex-wrap: wrap; gap: 8px; }
.two-column { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 24px; }.record-list { display: grid; margin-top: 16px; }.record-list > div { min-height: 64px; display: flex; align-items: center; justify-content: space-between; gap: 12px; border-top: 1px solid var(--color-hairline); }.record-list > div:first-child { border-top: 0; }.record-list strong, .record-list span { display: block; }.record-list strong { font-weight: 600; }.record-list span { color: var(--color-ink-muted); font-size: 13px; }
.dialog-title { padding: 24px 24px 4px; font-size: var(--font-section-size); font-weight: 600; }.correction-body { display: grid; gap: 16px; padding: 12px 24px !important; }.correction-body p { margin: 0; color: var(--color-ink-muted); }.correction-schedule { display: grid; gap: 8px; }.correction-schedule > span { color: var(--color-ink-muted); font-size: 13px; }.weekday-toggle { width: fit-content; max-width: 100%; flex-wrap: wrap; }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
@media (max-width: 800px) { .two-column { grid-template-columns: 1fr; }.usage-overview { grid-template-columns: 1fr; } }
</style>
