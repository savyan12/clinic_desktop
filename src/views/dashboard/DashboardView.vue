<template>
  <div class="page-container page-stack">
    <PageHeader :title="greeting" :subtitle="todayLabel" action-label="تسجيل جلسة جديدة" action-icon="mdi-plus" @action="sessionDialog = true">
      <template #actions>
        <div v-if="auth.isAdmin" class="shortcut-row">
          <v-btn v-for="shortcut in dashboard?.shortcuts" :key="shortcut.to" :to="shortcut.to" variant="outlined" :prepend-icon="shortcut.icon">{{ shortcut.label }}</v-btn>
          <v-btn color="primary" prepend-icon="mdi-plus" @click="sessionDialog = true">تسجيل جلسة جديدة</v-btn>
        </div>
        <v-btn v-else color="primary" prepend-icon="mdi-plus" @click="sessionDialog = true">تسجيل جلسة جديدة</v-btn>
      </template>
    </PageHeader>

    <template v-if="loading">
      <v-skeleton-loader type="heading, image, table" />
    </template>
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="dashboard">
      <section class="metric-grid" aria-label="الإحصائيات الرئيسية">
        <StatCell v-for="stat in dashboard.stats" :key="stat.key" v-bind="stat" />
      </section>

      <section class="page-stack">
        <div class="section-row">
          <div><h2 class="section-heading">جلسات اليوم</h2><span class="caption-text">كل جلسة تُغلق مباشرة عند تسجيلها</span></div>
          <router-link to="/sessions">عرض قسم الجلسات</router-link>
        </div>
        <div class="filter-grid surface-card dashboard-filters">
          <v-text-field v-model="filters.search" label="بحث سريع" placeholder="الاسم أو رقم الهاتف" prepend-inner-icon="mdi-magnify" clearable />
          <v-select v-model="filters.doctor_id" label="الطبيب" :items="doctors" item-title="name" item-value="id" clearable :loading="loadingDoctors" />
          <v-select v-model="filters.status" label="الحالة" :items="statusItems" clearable />
          <v-text-field v-model="filters.date" label="التاريخ" type="date" />
        </div>
        <InlineError v-if="doctorError" :message="doctorError" retry @retry="loadDoctors" />
        <DataTableCard
          :headers="sessionHeaders"
          :items="filteredSessions"
          :empty="{ icon: 'mdi-clipboard-check-outline', title: 'لا توجد جلسات اليوم', description: 'لم تُسجل جلسات اليوم أو لا توجد نتائج مطابقة للفلاتر.' }"
        >
          <template #rows="{ items }">
            <tr v-for="session in items" :key="session.id" class="clickable-row" @click="router.push(`/sessions/${session.id}`)">
              <td class="ltr-value">{{ time(session.opened_at) }}</td>
              <td><strong>{{ session.patient.name }}</strong></td>
              <td><span class="ltr-value">{{ session.patient.phone }}</span></td>
              <td>{{ session.doctor.name }}</td>
              <td><span class="services-cell">{{ session.services.map((item) => item.name).join('، ') }}</span></td>
              <td><StatusChip type="coverage" :value="session.settlement.coverage" /></td>
              <td><StatusChip type="session" :value="session.status" /></td>
              <td @click.stop>
                <v-menu>
                  <template #activator="{ props }"><v-btn v-bind="props" icon="mdi-dots-vertical" variant="text" size="small" aria-label="إجراءات الجلسة" /></template>
                  <v-list density="compact">
                    <v-list-item prepend-icon="mdi-eye-outline" title="فتح التفاصيل" :to="`/sessions/${session.id}`" />
                    <v-list-item prepend-icon="mdi-printer-outline" title="طباعة" :to="`/print/session/${session.id}`" />
                  </v-list>
                </v-menu>
              </td>
            </tr>
          </template>
        </DataTableCard>
      </section>

      <section class="follow-up-section">
        <div class="section-row">
          <div><h2 class="section-heading">تحتاج متابعة قريباً</h2><span class="caption-text">اشتراكات متبقي منها جلستان أو أقل أو يقترب تاريخها المتوقع</span></div>
          <router-link to="/subscriptions?near_end=1">عرض الاشتراكات</router-link>
        </div>
        <div v-if="dashboard.near_subscriptions.length" class="follow-up-strip">
          <router-link v-for="subscription in dashboard.near_subscriptions" :key="subscription.id" :to="`/subscriptions/${subscription.id}`">
            <strong>{{ subscription.patient.name }}</strong>
            <span>{{ subscription.plan_type === 'monthly' ? 'اشتراك شهري' : subscription.services.map((item) => item.name).join(' + ') }}</span>
            <em>{{ subscription.plan_type === 'monthly' ? `ينتهي ${formatSubscriptionDate(subscription.end_date)}` : `متبقي ${subscription.remaining}` }}</em>
          </router-link>
        </div>
        <EmptyState v-else icon="mdi-check-decagram-outline" title="لا توجد اشتراكات قريبة من الانتهاء" description="لا توجد اشتراكات تحتاج متابعة وفق الحد الحالي." />
      </section>

      <section v-if="auth.isAdmin" class="admin-summary-grid">
        <div class="surface-card">
          <div class="section-row"><h2 class="section-heading">الملخص المالي</h2><router-link to="/reports/collections">التقرير التفصيلي</router-link></div>
          <div class="finance-list">
            <router-link to="/reports/collections"><span>المقبوضات اليوم</span><MoneyText :value="dashboard.financial_summary.collections_today" /></router-link>
            <router-link to="/reports/debts"><span>الديون المتبقية</span><MoneyText :value="dashboard.financial_summary.debts_remaining" /></router-link>
            <router-link to="/reports/doctor-dues"><span>مستحقات الأطباء</span><MoneyText :value="dashboard.financial_summary.doctor_dues_remaining" /></router-link>
          </div>
        </div>
        <div class="surface-card">
          <div class="section-row"><h2 class="section-heading">أحدث الحركات</h2><router-link to="/reports/collections">عرض الكل</router-link></div>
          <div class="movement-list">
            <div v-for="movement in dashboard.recent_movements" :key="movement.id">
              <span>{{ paymentLabel(movement.type) }} · {{ movement.patient?.name || '—' }}</span>
              <MoneyText :value="movement.amount" />
            </div>
          </div>
        </div>
      </section>
    </template>
  </div>

  <NewSessionDialog v-model="sessionDialog" @saved="load" />
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getDashboard } from '@/api/dashboard'
import { getDoctorOptions } from '@/api/doctors'
import DataTableCard from '@/components/common/DataTableCard.vue'
import EmptyState from '@/components/common/EmptyState.vue'
import InlineError from '@/components/common/InlineError.vue'
import MoneyText from '@/components/common/MoneyText.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatCell from '@/components/common/StatCell.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import NewSessionDialog from '@/components/sessions/NewSessionDialog.vue'
import { useAuthStore } from '@/stores/auth'
import { useUiStore } from '@/stores/ui'
import { CLINIC_TIME_ZONE, clinicDate, clinicHour } from '@/utils/date'

const auth = useAuthStore()
const ui = useUiStore()
const router = useRouter()
const route = useRoute()
const loading = ref(true)
const error = ref('')
const dashboard = ref(null)
const doctors = ref([])
const loadingDoctors = ref(false)
const doctorError = ref('')
const sessionDialog = ref(false)
const filters = reactive({ search: '', doctor_id: null, status: null, date: clinicDate() })
const statusItems = [{ title: 'مغلقة', value: 'closed' }, { title: 'ملغاة', value: 'cancelled' }, { title: 'مفتوحة قديمة', value: 'open' }]
const sessionHeaders = [
  { title: 'الوقت', key: 'time' }, { title: 'المريض', key: 'patient' }, { title: 'رقم الهاتف', key: 'phone' },
  { title: 'الطبيب', key: 'doctor' }, { title: 'الخدمات', key: 'services' }, { title: 'التغطية / الدفع', key: 'coverage' },
  { title: 'الحالة', key: 'status' }, { title: 'الإجراءات', key: 'actions' },
]

const firstName = computed(() => auth.user?.name?.split(' ')[0] || '')
const greeting = computed(() => `${clinicHour() < 12 ? 'صباح الخير' : 'مساء الخير'}، ${firstName.value}`)
const todayLabel = new Intl.DateTimeFormat('ar-LY-u-nu-latn', { timeZone: CLINIC_TIME_ZONE, weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }).format(new Date())
const filteredSessions = computed(() => (dashboard.value?.today_sessions || []).filter((session) => {
  const query = filters.search.trim()
  return (!query || session.patient.name.includes(query) || session.patient.phone.includes(query))
    && (!filters.doctor_id || session.doctor_id === Number(filters.doctor_id))
    && (!filters.status || session.status === filters.status)
    && (!filters.date || session.opened_at.startsWith(filters.date))
}))

onMounted(() => {
  if (route.query.forbidden) ui.showSnackbar('هذه الصفحة متاحة لمدير النظام فقط.', 'warning')
  load()
  loadDoctors()
})

async function load() {
  loading.value = true; error.value = ''
  try {
    const dashboardResponse = await getDashboard()
    dashboard.value = dashboardResponse.data
  } catch (apiError) { error.value = apiError.message } finally { loading.value = false }
}

async function loadDoctors() {
  loadingDoctors.value = true
  doctorError.value = ''
  try { doctors.value = await getDoctorOptions() } catch (cause) { doctorError.value = cause.message } finally { loadingDoctors.value = false }
}

const time = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(value))
const paymentLabel = (type) => ({ wallet_topup: 'شحن رصيد', session_payment: 'دفعة جلسة', subscription_payment: 'دفعة اشتراك', debt_payment: 'تسديد دين', refund: 'استرجاع' }[type] || type)
const formatSubscriptionDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(`${value}T00:00:00`))
</script>

<style scoped>
.shortcut-row { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 10px; }
.section-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.dashboard-filters { padding: 18px 20px; }
.services-cell { display: inline-block; max-width: 240px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.follow-up-section { display: grid; gap: 14px; }
.follow-up-strip { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-card); }
.follow-up-strip a { display: grid; gap: 2px; padding: 18px 20px; color: var(--color-ink); border-inline-start: 1px solid var(--color-hairline); }
.follow-up-strip a:hover { background: var(--color-pearl); }
.follow-up-strip strong { font-weight: 600; }
.follow-up-strip span { color: var(--color-ink-muted); font-size: 13px; }
.follow-up-strip em { color: var(--color-warning); font-size: 14px; font-style: normal; font-weight: 600; }
.admin-summary-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 24px; }
.finance-list, .movement-list { display: grid; margin-top: 16px; }
.finance-list a, .movement-list div { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 52px; color: var(--color-ink); border-top: 1px solid var(--color-hairline); }
.finance-list a:first-child, .movement-list div:first-child { border-top: 0; }
@media (max-width: 900px) { .admin-summary-grid { grid-template-columns: 1fr; } }
@media (max-width: 640px) { .section-row { align-items: flex-start; } .shortcut-row { width: 100%; } .shortcut-row :deep(.v-btn) { flex: 1 1 140px; } }
</style>
