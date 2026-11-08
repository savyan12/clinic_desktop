<template>
  <div class="page-container page-stack">
    <template v-if="loading"><v-skeleton-loader type="heading, image, article" /></template>
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="patient">
      <PageHeader :title="patient.name">
        <template #subtitle>ملف المريض · <bdi class="phone-value" dir="ltr">{{ patient.phone || '—' }}</bdi></template>
        <template #actions>
          <v-btn color="primary" prepend-icon="mdi-plus" @click="sessionDialog = true">فتح جلسة</v-btn>
          <v-btn variant="outlined" prepend-icon="mdi-wallet-membership" @click="subscriptionDialog = true">اشتراك جديد</v-btn>
          <v-btn :to="`/print/statement/patient/${patient.id}`" variant="outlined" prepend-icon="mdi-printer-outline">كشف الحساب</v-btn>
        </template>
      </PageHeader>

      <section class="patient-identity surface-card">
        <div class="patient-avatar">{{ patient.name.charAt(0) }}</div>
        <div class="identity-main"><h2>{{ patient.name }}</h2><span class="phone-value">{{ patient.phone }}</span></div>
        <div><span>العمر</span><strong>{{ patient.age }} سنة</strong></div>
        <div><span>الجنس</span><strong>{{ labelFor('gender', patient.gender) }}</strong></div>
        <div><span>المنطقة</span><strong>{{ patient.area || '—' }}</strong></div>
        <div><span>الجهة الراعية</span><strong>{{ patient.sponsor?.name || 'لا توجد' }}</strong></div>
      </section>

      <v-tabs v-model="tab" color="primary" class="patient-tabs">
        <v-tab value="details">بيانات المريض</v-tab>
        <v-tab value="sessions">سجل الجلسات</v-tab>
        <v-tab value="subscriptions">الاشتراكات</v-tab>
        <v-tab value="financial">الحساب المالي</v-tab>
      </v-tabs>

      <v-window v-model="tab">
        <v-window-item value="details">
          <section class="surface-card tab-panel">
            <h2 class="section-heading">البيانات الأساسية</h2>
            <div class="detail-grid">
              <div class="detail-item"><span class="detail-label">الاسم الكامل</span><strong>{{ patient.name }}</strong></div>
              <div class="detail-item"><span class="detail-label">رقم الهاتف</span><strong class="phone-value">{{ patient.phone }}</strong></div>
              <div class="detail-item"><span class="detail-label">العمر</span><strong>{{ patient.age }} سنة</strong></div>
              <div class="detail-item"><span class="detail-label">الجنس</span><strong>{{ labelFor('gender', patient.gender) }}</strong></div>
              <div class="detail-item"><span class="detail-label">العنوان</span><strong>{{ patient.area || '—' }}</strong></div>
              <div class="detail-item"><span class="detail-label">هاتف الطوارئ</span><strong class="phone-value">{{ patient.emergency_phone || '—' }}</strong></div>
              <div class="detail-item"><span class="detail-label">الجهة الراعية</span><strong>{{ patient.sponsor?.name || 'لا توجد' }}</strong></div>
              <div class="detail-item"><span class="detail-label">تاريخ إنشاء السجل</span><strong>{{ formatDate(patient.created_at) }}</strong></div>
            </div>
          </section>
        </v-window-item>

        <v-window-item value="sessions">
          <section class="surface-card tab-panel">
            <div class="section-row"><h2 class="section-heading">سجل الجلسات</h2><v-btn color="primary" variant="text" prepend-icon="mdi-plus" @click="sessionDialog = true">فتح جلسة</v-btn></div>
            <div v-if="patient.sessions.length" class="table-scroll"><v-table><thead><tr><th>التاريخ</th><th>الطبيب</th><th>الخدمات</th><th>القيمة</th><th>التسوية</th><th>الحالة</th><th></th></tr></thead><tbody><tr v-for="item in patient.sessions" :key="item.id"><td>{{ formatDateTime(item.opened_at) }}</td><td>{{ item.doctor.name }}</td><td>{{ item.services.map((service) => service.name).join('، ') }}</td><td><MoneyText :value="item.net_total" /></td><td><StatusChip type="coverage" :value="item.settlement.coverage" /></td><td><StatusChip type="session" :value="item.status" /></td><td><v-btn :to="`/sessions/${item.id}`" variant="text">فتح</v-btn></td></tr></tbody></v-table></div>
            <EmptyState v-else icon="mdi-clipboard-outline" title="لا توجد جلسات" description="لم تُسجل جلسات لهذا المريض بعد." />
          </section>
        </v-window-item>

        <v-window-item value="subscriptions">
          <section class="surface-card tab-panel">
            <div class="section-row"><h2 class="section-heading">الاشتراكات</h2><v-btn color="primary" variant="text" prepend-icon="mdi-plus" @click="subscriptionDialog = true">اشتراك جديد</v-btn></div>
            <div v-if="patient.subscriptions.length" class="subscription-cards">
              <router-link v-for="item in patient.subscriptions" :key="item.id" :to="`/subscriptions/${item.id}`" class="subscription-card">
                <div class="section-row"><strong>{{ item.name }} — {{ item.services.map((service) => service.name).join(' + ') }}</strong><StatusChip type="subscription" :value="item.status" /></div>
                <div class="usage-track"><span :style="{ width: `${item.sessions_count ? (item.sessions_used / item.sessions_count) * 100 : 0}%` }"></span></div>
                <div class="subscription-meta"><span>{{ item.sessions_used }} مستخدمة</span><span>{{ item.remaining }} متبقية</span><MoneyText :value="item.total_price" /></div>
              </router-link>
            </div>
            <EmptyState v-else icon="mdi-wallet-membership" title="لا توجد اشتراكات" description="يمكن إنشاء أكثر من اشتراك للمريض." />
          </section>
        </v-window-item>

        <v-window-item value="financial">
          <div class="page-stack tab-panel">
            <div class="financial-separation">
              <section class="account-card wallet-card">
                <div><span>الرصيد المدفوع مسبقًا</span><MoneyText :value="patient.wallet_balance" /><small>رصيد متاح للاستخدام — ليس دينًا</small></div>
                <v-btn color="primary" prepend-icon="mdi-wallet-plus-outline" @click="topUpDialog = true">شحن الرصيد</v-btn>
              </section>
              <section class="account-card debt-card">
                <div><span>الديون القائمة</span><MoneyText :value="patient.debt_total" /><small>مبالغ مستحقة — مستقلة عن الرصيد</small></div>
                <div class="account-actions"><v-btn variant="outlined" color="primary" :disabled="!openDebts.length" @click="settlementDialog = true">تسجيل تسديد</v-btn><v-btn v-if="isAdmin" variant="text" @click="openingDebtDialog = true">دين افتتاحي</v-btn></div>
              </section>
            </div>

            <section class="surface-card">
              <div class="section-row"><h2 class="section-heading">حركات الرصيد</h2><span class="caption-text">الشحن والخصم والإعادات</span></div>
              <div v-if="patient.wallet_movements.length" class="table-scroll"><v-table><thead><tr><th>التاريخ</th><th>النوع</th><th>المبلغ</th><th>الرصيد بعد الحركة</th></tr></thead><tbody><tr v-for="item in patient.wallet_movements" :key="item.id"><td>{{ formatDateTime(item.created_at) }}</td><td>{{ walletLabel(item) }}</td><td><MoneyText :value="item.amount" /></td><td><MoneyText :value="item.balance_after" /></td></tr></tbody></v-table></div>
              <EmptyState v-else title="لا توجد حركات رصيد" description="لم يُشحن أو يُستخدم الرصيد بعد." />
            </section>

            <section class="surface-card">
              <div class="section-row"><h2 class="section-heading">الديون والتسويات</h2><v-btn v-if="isAdmin" color="error" variant="text" prepend-icon="mdi-cash-refund" :disabled="!refundablePayments.length" @click="refundDialog = true">تسجيل استرجاع</v-btn></div>
              <div v-if="patient.debts.length" class="table-scroll"><v-table><thead><tr><th>المرجع</th><th>النوع</th><th>الأصل</th><th>المسدد</th><th>المتبقي</th><th>الحالة</th></tr></thead><tbody><tr v-for="item in patient.debts" :key="item.id"><td>#{{ item.id }}</td><td>{{ debtKind(item.kind) }}</td><td><MoneyText :value="item.amount" /></td><td><MoneyText :value="item.paid_amount" /></td><td><MoneyText :value="item.remaining" /></td><td><StatusChip type="debt" :value="item.status" /></td></tr></tbody></v-table></div>
              <EmptyState v-else icon="mdi-check-circle-outline" title="لا توجد ديون" description="حساب المريض لا يحتوي على مبالغ مستحقة." />
            </section>

            <section class="surface-card">
              <h2 class="section-heading">المدفوعات والاسترجاعات</h2>
              <div v-if="patient.payments.length" class="movement-list"><div v-for="item in patient.payments" :key="item.id"><div><strong>{{ paymentLabel(item.type) }}</strong><span>{{ formatDateTime(item.created_at) }} · {{ labelFor('paymentMethod', item.method) }}</span></div><MoneyText :value="item.amount" /></div></div>
              <EmptyState v-else title="لا توجد مدفوعات" description="لا توجد حركات دفع أو استرجاع مرتبطة بالمريض." />
            </section>
          </div>
        </v-window-item>
      </v-window>
    </template>
  </div>

  <NewSessionDialog v-if="patient" v-model="sessionDialog" :initial-patient-id="patient.id" :initial-patient="patient" @saved="load" />
  <CreateSubscriptionDialog v-if="patient" v-model="subscriptionDialog" :initial-patient-id="patient.id" :initial-patient="patient" @saved="load" />
  <TopUpWalletDialog v-if="patient" v-model="topUpDialog" :patient-id="patient.id" :current-balance="patient.wallet_balance" @saved="load" />
  <DebtSettlementDialog v-if="patient" v-model="settlementDialog" :debts="patient.debts" @saved="load" />
  <RefundDialog v-if="patient" v-model="refundDialog" :patient-id="patient.id" :payments="patient.payments" @saved="load" />
  <OpeningDebtDialog v-if="patient" v-model="openingDebtDialog" :patient-id="patient.id" @saved="load" />
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { getPatient } from '@/api/patients'
import EmptyState from '@/components/common/EmptyState.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import DebtSettlementDialog from '@/components/financial/DebtSettlementDialog.vue'
import OpeningDebtDialog from '@/components/financial/OpeningDebtDialog.vue'
import RefundDialog from '@/components/financial/RefundDialog.vue'
import TopUpWalletDialog from '@/components/financial/TopUpWalletDialog.vue'
import NewSessionDialog from '@/components/sessions/NewSessionDialog.vue'
import CreateSubscriptionDialog from '@/components/subscriptions/CreateSubscriptionDialog.vue'
import { labelFor } from '@/constants/labels'
import { usePermissions } from '@/composables/usePermissions'

const route = useRoute(); const { isAdmin } = usePermissions()
const patient = ref(null); const loading = ref(true); const error = ref(''); const tab = ref('details')
const sessionDialog = ref(false); const subscriptionDialog = ref(false); const topUpDialog = ref(false); const settlementDialog = ref(false); const refundDialog = ref(false); const openingDebtDialog = ref(false)
const openDebts = computed(() => patient.value?.debts.filter((item) => ['open', 'partial'].includes(item.status)) || [])
const refundablePayments = computed(() => {
  const payments = patient.value?.payments || []
  const refunded = payments.filter((payment) => payment.type === 'refund' && payment.reverses_payment_id).reduce((totals, payment) => {
    totals[payment.reverses_payment_id] = (totals[payment.reverses_payment_id] || 0) + Math.abs(Number(payment.amount || 0))
    return totals
  }, {})
  return payments.filter((payment) => payment.type !== 'refund' && Number(payment.amount) - (refunded[payment.id] || 0) > 0.0005)
})
onMounted(load)
async function load() { loading.value = true; error.value = ''; try { patient.value = (await getPatient(route.params.id)).data } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
const formatDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(value))
const formatDateTime = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
const walletLabel = (movement) => movement.type === 'debit' && movement.payment_id
  ? 'استرجاع'
  : ({ topup: 'شحن', debit: 'خصم جلسة', reversal: 'إعادة رصيد' }[movement.type] || movement.type)
const debtKind = (kind) => ({ session: 'جلسة', subscription: 'اشتراك', opening: 'افتتاحي' }[kind] || kind)
const paymentLabel = (type) => ({ wallet_topup: 'شحن رصيد', session_payment: 'دفعة جلسة', subscription_payment: 'دفعة اشتراك', debt_payment: 'تسديد دين', refund: 'استرجاع' }[type] || type)
</script>

<style scoped>
.patient-identity { display: grid; grid-template-columns: auto 1.5fr repeat(4, 1fr); align-items: center; gap: 20px; }.patient-avatar { width: 64px; height: 64px; display: grid; place-items: center; color: var(--color-primary); border-radius: var(--radius-lg); background: var(--color-primary-subtle); border: 1px solid var(--color-primary-border); font-size: 28px; font-weight: 600; }.identity-main h2 { margin: 0; font-size: 20px; }.identity-main span, .patient-identity > div:not(.patient-avatar):not(.identity-main) span { display: block; color: var(--color-ink-muted); font-size: 13px; }.patient-identity > div strong { font-weight: 600; }
.patient-tabs { border-bottom: 1px solid var(--color-hairline); }.tab-panel { margin-top: 20px; }.section-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; }.table-scroll { overflow-x: auto; margin-top: 16px; }.table-scroll :deep(table) { min-width: 850px; }
.subscription-cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 16px; margin-top: 18px; }.subscription-card { display: grid; gap: 14px; padding: 18px; color: var(--color-ink); border: 1px solid var(--color-hairline); border-radius: var(--radius-lg); }.subscription-card:hover { border-color: var(--color-primary); background: var(--color-primary-subtle); }.usage-track { height: 8px; overflow: hidden; border-radius: var(--radius-pill); background: var(--color-parchment); }.usage-track span { display: block; height: 100%; background: var(--color-primary); }.subscription-meta { display: flex; justify-content: space-between; gap: 10px; color: var(--color-ink-muted); font-size: 13px; }
.account-card { display: flex; align-items: center; justify-content: space-between; gap: 20px; padding: 24px; border: 1px solid var(--color-hairline); border-radius: var(--radius-card); }.account-card > div:first-child { display: grid; }.account-card > div:first-child > span { color: var(--color-ink-muted); }.account-card :deep(.money-text) { margin: 4px 0; font-size: 32px; font-weight: 600; }.account-card small { color: var(--color-ink-muted); }.wallet-card { background: var(--color-primary-subtle); border-color: var(--color-primary-border); }.wallet-card :deep(.money-text) { color: var(--color-primary); }.debt-card { background: var(--color-warning-bg); border-color: var(--color-warning-border); }.debt-card :deep(.money-text) { color: var(--color-warning); }.account-actions { display: flex; flex-wrap: wrap; gap: 8px; justify-content: flex-end; }
.movement-list { display: grid; margin-top: 16px; }.movement-list > div { min-height: 64px; display: flex; align-items: center; justify-content: space-between; gap: 16px; border-top: 1px solid var(--color-hairline); }.movement-list > div:first-child { border-top: 0; }.movement-list strong, .movement-list span { display: block; }.movement-list strong { font-weight: 600; }.movement-list span { color: var(--color-ink-muted); font-size: 13px; }
@media (max-width: 1100px) { .patient-identity { grid-template-columns: auto 1fr repeat(2, 1fr); } }
@media (max-width: 700px) { .patient-identity { grid-template-columns: auto 1fr; } .account-card { align-items: flex-start; flex-direction: column; } }
</style>
