<template>
  <div class="page-stack">
    <PageHeader v-if="subscription" :title="subscription.name" :subtitle="`${subscription.patient.name} · بدأ في ${formatDate(subscription.start_date)}`">
      <template #actions>
        <v-btn variant="outlined" prepend-icon="mdi-account-outline" :to="`/patients/${subscription.patient_id}`">ملف المريض</v-btn>
        <template v-if="isAdmin && subscription.status !== 'cancelled'">
          <v-btn variant="outlined" prepend-icon="mdi-pencil-outline" @click="openCorrection">تعديل العقد</v-btn>
          <v-btn color="error" variant="outlined" prepend-icon="mdi-cancel" @click="cancelDialog = true">إلغاء الاشتراك</v-btn>
        </template>
      </template>
    </PageHeader>

    <v-skeleton-loader v-if="loading" type="article, table" />
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="subscription">
      <div class="subscription-status-banner">
        <StatusChip type="subscription" :value="subscription.status" />
        <span>{{ statusDescription }}</span>
      </div>

      <section class="usage-overview surface-card">
        <div class="usage-number"><strong>{{ subscription.remaining }}</strong><span>جلسة متبقية</span></div>
        <div class="usage-main">
          <div class="section-row"><span>{{ subscription.sessions_used }} مستخدمة</span><span>{{ subscription.sessions_count }} إجمالي</span></div>
          <div class="usage-track"><span :style="{ width: usagePercent + '%' }"></span></div>
          <div class="services-chips"><v-chip v-for="service in subscription.services" :key="service.service_id" variant="outlined">{{ service.name }}</v-chip></div>
          <p>كل جلسة مكتملة تخصم زيارة واحدة من الرصيد المشترك، مهما كان عدد الخدمات المستخدمة فيها.</p>
        </div>
      </section>

      <section class="detail-grid">
        <div class="detail-item"><span class="detail-label">قيمة الاشتراك</span><MoneyText :value="subscription.total_price" /></div>
        <div class="detail-item"><span class="detail-label">إجمالي المحصل</span><MoneyText :value="subscription.received_amount" /></div>
        <div class="detail-item"><span class="detail-label">إجمالي المسترجع</span><MoneyText :value="subscription.refunded_amount" /></div>
        <div class="detail-item"><span class="detail-label">صافي المحتفظ به</span><MoneyText :value="subscription.retained_amount" /></div>
        <div class="detail-item"><span class="detail-label">الرصيد المستحق</span><MoneyText :value="subscription.outstanding_amount" /></div>
        <div class="detail-item"><span class="detail-label">تاريخ البداية</span><strong>{{ formatDate(subscription.start_date) }}</strong></div>
      </section>

      <section class="surface-card">
        <h2 class="section-heading">سجل جلسات الاشتراك</h2>
        <div v-if="subscription.sessions.length" class="session-list">
          <div v-for="(session, index) in subscription.sessions" :key="session.id" class="session-row">
            <div class="session-number">{{ subscription.sessions.length - index }}</div>
            <div class="session-main">
              <strong>{{ session.services.map((service) => service.name).join('، ') }}</strong>
              <span>{{ formatDateTime(session.closed_at || session.opened_at) }} · {{ session.doctor?.name || 'طبيب غير محدد' }}</span>
              <span v-if="session.status === 'cancelled' && session.cancel_reason" class="cancel-reason">سبب الإلغاء: {{ session.cancel_reason }}</span>
            </div>
            <StatusChip type="session" :value="session.status" />
            <router-link :to="`/sessions/${session.id}`">التفاصيل</router-link>
          </div>
        </div>
        <EmptyState v-else icon="mdi-history" title="لا توجد جلسات بعد" description="ستظهر هنا كل جلسة مع خدماتها والطبيب المشرف وحالتها." />
      </section>

      <section class="two-column">
        <div class="surface-card">
          <h2 class="section-heading">الدفعات والاسترجاعات</h2>
          <div v-if="subscription.payments.length" class="record-list">
            <div v-for="payment in subscription.payments" :key="`p-${payment.id}-${payment.source || 'direct'}`">
              <div><strong>{{ paymentLabel(payment) }}</strong><span>{{ formatDateTime(payment.created_at) }} · {{ methodLabel(payment.method) }}</span></div>
              <MoneyText :value="payment.amount" />
            </div>
          </div>
          <EmptyState v-else title="لا توجد دفعات" description="لم تُسجل حركات مالية على هذا الاشتراك." />
        </div>
        <div class="surface-card">
          <h2 class="section-heading">سجل الدين</h2>
          <div v-if="subscription.debts.length" class="record-list">
            <div v-for="debt in subscription.debts" :key="`d-${debt.id}`"><div><strong>دين الاشتراك</strong><span><StatusChip type="debt" :value="debt.status" /></span></div><MoneyText :value="debt.remaining" /></div>
          </div>
          <EmptyState v-else title="لا يوجد دين" description="تم دفع قيمة الاشتراك بالكامل عند الإنشاء." />
        </div>
      </section>
    </template>
  </div>

  <CancelSubscriptionDialog v-if="subscription" v-model="cancelDialog" :subscription-id="subscription.id" @saved="cancelled" />
  <v-dialog v-model="correctionDialog" persistent max-width="580">
    <v-card>
      <v-card-title class="dialog-title">تعديل عقد الاشتراك</v-card-title>
      <v-card-text class="correction-body">
        <p>لن تتغير الدفعات السابقة. سيعاد حساب الرصيد المستحق من السعر الجديد والحركات المالية الفعلية.</p>
        <InlineError v-if="correctionError" :message="correctionError" />
        <v-text-field v-model="correction.sessions_count" label="إجمالي عدد الجلسات" type="number" :min="subscription?.sessions_used || 1" />
        <v-text-field v-model="correction.total_price" label="سعر الاشتراك الإجمالي" type="text" inputmode="decimal" suffix="د.ل" />
        <v-textarea v-model="correction.reason" label="سبب التعديل" rows="3" />
      </v-card-text>
      <v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="correcting" @click="correctionDialog = false">تراجع</v-btn><v-btn color="primary" data-form-flow-primary :loading="correcting" @click="applyCorrection">حفظ التعديل</v-btn></v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'
import { correctSubscription, getSubscription } from '@/api/subscriptions'
import EmptyState from '@/components/common/EmptyState.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import CancelSubscriptionDialog from '@/components/subscriptions/CancelSubscriptionDialog.vue'
import { useIdempotency } from '@/composables/useIdempotency'
import { usePermissions } from '@/composables/usePermissions'
import { useUiStore } from '@/stores/ui'
import { numeric } from '@/composables/useMoney'

const route = useRoute()
const ui = useUiStore()
const { isAdmin } = usePermissions()
const { key, reset } = useIdempotency()
const subscription = ref(null)
const loading = ref(true)
const error = ref('')
const cancelDialog = ref(false)
const correctionDialog = ref(false)
const correctionError = ref('')
const correcting = ref(false)
const correction = reactive({ sessions_count: '', total_price: '', reason: '' })
const usagePercent = computed(() => subscription.value?.sessions_count ? Math.min(100, (subscription.value.sessions_used / subscription.value.sessions_count) * 100) : 0)
const statusDescription = computed(() => {
  if (subscription.value.status === 'active') return 'الاشتراك متاح ما دام يحتوي على جلسات متبقية.'
  if (subscription.value.status === 'completed') return 'اكتمل الاشتراك باستهلاك جميع الجلسات.'
  return 'أُلغي الاشتراك وحُفظت جلساته وحركاته المالية السابقة.'
})

onMounted(load)
async function load() {
  loading.value = true; error.value = ''
  try { subscription.value = (await getSubscription(route.params.id)).data } catch (apiError) { error.value = apiError.message } finally { loading.value = false }
}
async function cancelled() { ui.showSnackbar('تم إلغاء الاشتراك وتسجيل الاسترجاع المحدد.'); await load() }
function openCorrection() {
  reset()
  Object.assign(correction, { sessions_count: subscription.value.sessions_count, total_price: subscription.value.total_price, reason: '' })
  correctionError.value = ''
  correctionDialog.value = true
}
async function applyCorrection() {
  correcting.value = true; correctionError.value = ''
  try {
    await correctSubscription(subscription.value.id, { changes: { sessions_count: Number(correction.sessions_count), total_price: numeric(correction.total_price) }, reason: correction.reason.trim() }, key.value)
    ui.showSnackbar('تم تعديل عقد الاشتراك وتوثيق القيم السابقة والجديدة.')
    correctionDialog.value = false
    await load()
  } catch (apiError) { correctionError.value = apiError.message } finally { correcting.value = false }
}
function paymentLabel(payment) {
  if (payment.type === 'refund') return 'استرجاع اشتراك'
  if (payment.source === 'debt_allocation' || payment.type === 'debt_payment') return 'دفعة مخصصة لدين الاشتراك'
  return 'دفعة اشتراك'
}
const methodLabel = (method) => method === 'transfer' ? 'تحويل' : 'نقدي'
const formatDate = (value) => value ? new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(`${value}T00:00:00`)) : 'غير محدد'
const formatDateTime = (value) => value ? new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value)) : '—'
</script>

<style scoped>
.subscription-status-banner { display: flex; align-items: center; gap: 12px; padding: 16px 20px; border: 1px solid var(--color-hairline); border-radius: 14px; background: var(--color-pearl); }.usage-overview { display: grid; grid-template-columns: 180px 1fr; align-items: center; gap: 28px; }.usage-number { text-align: center; }.usage-number strong, .usage-number span { display: block; }.usage-number strong { font-size: 56px; line-height: 1; color: var(--color-primary); }.usage-number span { margin-top: 8px; color: var(--color-ink-muted); }.usage-main { display: grid; gap: 14px; }.usage-main p { margin: 0; color: var(--color-ink-muted); }.section-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; }.usage-track { height: 12px; overflow: hidden; border-radius: 99px; background: var(--color-parchment); }.usage-track span { display: block; height: 100%; background: var(--color-primary); }.services-chips { display: flex; flex-wrap: wrap; gap: 8px; }
.session-list, .record-list { display: grid; margin-top: 16px; }.session-row { display: grid; grid-template-columns: 40px minmax(0, 1fr) auto auto; align-items: center; gap: 14px; min-height: 78px; border-top: 1px solid var(--color-hairline); }.session-row:first-child, .record-list > div:first-child { border-top: 0; }.session-number { display: grid; place-items: center; width: 34px; height: 34px; border-radius: 50%; color: var(--color-primary); background: var(--color-info-bg); font-weight: 700; }.session-main { display: grid; gap: 4px; }.session-main span, .record-list span { color: var(--color-ink-muted); font-size: 13px; }.session-main .cancel-reason { color: var(--color-danger); }.two-column { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 24px; }.record-list > div { min-height: 64px; display: flex; align-items: center; justify-content: space-between; gap: 12px; border-top: 1px solid var(--color-hairline); }.record-list strong, .record-list span { display: block; }.record-list strong { font-weight: 600; }
.dialog-title { padding: 24px 24px 4px; font-size: var(--font-section-size); font-weight: 600; }.correction-body { display: grid; gap: 16px; padding: 12px 24px !important; }.correction-body p { margin: 0; color: var(--color-ink-muted); }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
@media (max-width: 800px) { .two-column, .usage-overview { grid-template-columns: 1fr; }.session-row { grid-template-columns: 36px minmax(0, 1fr) auto; }.session-row > a { grid-column: 2; } }
</style>
