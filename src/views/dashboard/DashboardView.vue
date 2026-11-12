<template>
  <div class="page-container page-stack dashboard-page">
    <PageHeader :title="greeting" :subtitle="todayLabel">
      <template #actions>
        <v-btn
          v-if="auth.isAdmin"
          to="/reports/sessions"
          variant="text"
          prepend-icon="mdi-file-chart-outline"
        >التقارير</v-btn>
        <v-btn
          color="primary"
          prepend-icon="mdi-plus"
          @click="sessionDialog = true"
        >تسجيل جلسة</v-btn>
      </template>
    </PageHeader>

    <template v-if="loading">
      <section
        class="dashboard-skeleton"
        aria-busy="true"
        aria-label="جارٍ تحميل لوحة التحكم"
      >
        <v-skeleton-loader v-for="index in 4" :key="index" type="article" />
      </section>
      <v-skeleton-loader type="image, list-item-three-line@4" />
    </template>
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="dashboard">
      <section
        class="dashboard-metrics"
        aria-label="إحصائيات اليوم"
        :style="{
          '--dashboard-stat-columns': Math.max(
            1,
            Math.min(primaryStats.length, 4),
          ),
        }"
      >
        <StatCell
          v-for="stat in primaryStats"
          :key="stat.key"
          v-bind="stat"
          :stat-key="stat.key"
          variant="tinted"
        />
      </section>

      <!-- <section
        class="doctor-snapshot surface-card"
        aria-labelledby="doctor-snapshot-title"
      >
        <div class="section-row">
          <div>
            <h2 id="doctor-snapshot-title" class="section-heading">
              حالة الأطباء اليوم
            </h2>
            <p class="section-support">
              توزيع الجلسات المسجلة اليوم حسب الطبيب المشرف.
            </p>
          </div>
          <router-link to="/doctors">عرض الأطباء</router-link>
        </div>
        <div v-if="doctorSnapshots.length" class="snapshot-strip">
          <article
            v-for="doctor in doctorSnapshots"
            :key="doctor.id"
            class="snapshot-card"
          >
            <div class="snapshot-avatar" aria-hidden="true">
              {{ doctor.name.charAt(0) }}
            </div>
            <div class="snapshot-copy">
              <div>
                <strong>{{ doctor.name }}</strong
                ><span
                  >{{ doctor.count }}
                  {{ doctor.count === 1 ? "جلسة" : "جلسات" }}</span
                >
              </div>
              <div class="snapshot-track" aria-hidden="true">
                <span :style="{ width: `${doctor.progress}%` }" />
              </div>
            </div>
            <StatusChip type="active" :value="doctor.is_active !== false" />
          </article>
        </div>
        <EmptyState
          v-else
          size="section"
          icon="mdi-doctor"
          title="لا توجد جلسات موزعة اليوم"
          description="ستظهر حالة الأطباء بعد تسجيل أول جلسة."
        />
      </section> -->

      <div class="dashboard-content-grid">
        <section
          class="activity-card surface-card"
          aria-labelledby="today-activity-title"
        >
          <div class="section-row activity-heading">
            <div>
              <h2 id="today-activity-title" class="section-heading">
                جلسات اليوم
              </h2>
            </div>
            <router-link to="/sessions">عرض الكل</router-link>
          </div>

          <div class="activity-filters">
            <v-text-field
              v-model="filters.search"
              label="بحث"
              placeholder="المريض أو رقم الهاتف"
              prepend-inner-icon="mdi-magnify"
              clearable
            />
            <v-select
              v-model="filters.doctor_id"
              label="الطبيب"
              :items="doctors"
              item-title="name"
              item-value="id"
              clearable
              :loading="loadingDoctors"
            />
            <v-select
              v-model="filters.status"
              label="الحالة"
              :items="statusItems"
              clearable
            />
            <v-btn
              v-if="hasActiveFilters"
              variant="text"
              size="small"
              @click="clearFilters"
            >مسح</v-btn>
          </div>
          <InlineError
            v-if="doctorError"
            :message="doctorError"
            retry
            @retry="loadDoctors"
          />

          <div v-if="filteredSessions.length" class="activity-feed">
            <article
              v-for="session in filteredSessions"
              :key="session.id"
              class="activity-item"
            >
              <div class="activity-icon" aria-hidden="true">
                <v-icon icon="mdi-clipboard-pulse-outline" size="21" />
              </div>
              <div class="activity-copy">
                <p>
                  سُجلت جلسة لـ
                  <router-link :to="`/patients/${session.patient.id}`">{{
                    session.patient.name
                  }}</router-link>
                  مع <strong>{{ session.doctor.name }}</strong>
                </p>
                <span>{{
                  session.services.map((item) => item.name).join("، ") ||
                    "بدون خدمات"
                }}
                  · <bdi dir="ltr">{{ session.patient.phone }}</bdi></span>
              </div>
              <div class="activity-meta">
                <time :datetime="session.opened_at">{{
                  time(session.opened_at)
                }}</time>
                <div>
                  <StatusChip
                    type="coverage"
                    :value="session.settlement.coverage"
                  /><StatusChip type="session" :value="session.status" />
                </div>
              </div>
              <v-menu>
                <template #activator="{ props }"><v-btn
                  v-bind="props"
                  icon="mdi-dots-vertical"
                  variant="text"
                  size="small"
                  aria-label="إجراءات الجلسة"
                /></template>
                <v-list density="compact">
                  <v-list-item
                    prepend-icon="mdi-eye-outline"
                    title="فتح التفاصيل"
                    :to="`/sessions/${session.id}`"
                  />
                  <v-list-item
                    prepend-icon="mdi-printer-outline"
                    title="طباعة"
                    :to="`/print/session/${session.id}`"
                  />
                </v-list>
              </v-menu>
            </article>
          </div>
          <EmptyState
            v-else
            size="section"
            icon="mdi-clipboard-check-outline"
            title="لا توجد جلسات مطابقة"
            description="لم تُسجل جلسات اليوم أو لا توجد نتائج مطابقة للفلاتر."
          />
        </section>

        <section
          class="follow-up-card surface-card"
          aria-labelledby="follow-up-title"
        >
          <div class="section-row">
            <div>
              <h2 id="follow-up-title" class="section-heading">تحتاج متابعة</h2>
              <p class="section-support">اشتراكات متبقي منها جلستان أو أقل.</p>
            </div>
            <router-link to="/subscriptions">الكل</router-link>
          </div>
          <div
            v-if="dashboard.near_subscriptions.length"
            class="follow-up-list"
          >
            <router-link
              v-for="subscription in dashboard.near_subscriptions"
              :key="subscription.id"
              :to="`/subscriptions/${subscription.id}`"
            >
              <span class="follow-up-avatar" aria-hidden="true">{{
                subscription.patient.name.charAt(0)
              }}</span>
              <span><strong>{{ subscription.patient.name }}</strong><small>{{ subscription.name }} ·
                {{
                  subscription.services.map((item) => item.name).join(" + ")
                }}</small></span>
              <em>متبقي {{ subscription.remaining }}</em>
            </router-link>
          </div>
          <EmptyState
            v-else
            class="mx-auto"
            size="section"
            icon="mdi-check"
            title="لا توجد حالات قريبة"
            description=""
          />
        </section>

        <section
          v-if="auth.isAdmin"
          class="financial-card"
          aria-labelledby="financial-title"
        >
          <div class="financial-card-heading">
            <div>
              <span>الملخص المالي</span>
              <h2 id="financial-title">حركة اليوم</h2>
            </div>
            <v-icon icon="mdi-finance" size="24" aria-hidden="true" />
          </div>
          <router-link to="/reports/collections"><span>المقبوضات اليوم</span><MoneyText :value="dashboard.financial_summary.collections_today" /></router-link>
          <router-link to="/reports/debts"><span>الديون المتبقية</span><MoneyText :value="dashboard.financial_summary.debts_remaining" /></router-link>
          <router-link to="/reports/doctor-dues"><span>مستحقات الأطباء</span><MoneyText
            :value="dashboard.financial_summary.doctor_dues_remaining"
          /></router-link>
        </section>

        <section
          v-if="auth.isAdmin"
          class="surface-card recent-card"
          aria-labelledby="recent-title"
        >
          <div class="section-row">
            <h2 id="recent-title" class="section-heading">أحدث الحركات</h2>
            <router-link to="/reports/collections">الكل</router-link>
          </div>
          <div v-if="dashboard.recent_movements.length" class="movement-feed">
            <div
              v-for="movement in dashboard.recent_movements"
              :key="movement.id"
            >
              <span class="movement-dot" aria-hidden="true" />
              <span><strong>{{ paymentLabel(movement.type) }}</strong><small>{{ movement.patient?.name || "—" }}</small></span>
              <MoneyText :value="movement.amount" />
            </div>
          </div>
          <EmptyState
            v-else
            size="inline"
            icon="mdi-cash-clock"
            title="لا توجد حركات مالية"
          />
        </section>
      </div>
    </template>
  </div>

  <NewSessionDialog v-model="sessionDialog" @saved="load" />
</template>

<script setup>
import { computed, onMounted, reactive, ref } from "vue";
import { useRoute } from "vue-router";
import { getDashboard } from "@/api/dashboard";
import { getDoctorOptions } from "@/api/doctors";
import EmptyState from "@/components/common/EmptyState.vue";
import InlineError from "@/components/common/InlineError.vue";
import MoneyText from "@/components/common/MoneyText.vue";
import PageHeader from "@/components/common/PageHeader.vue";
import StatCell from "@/components/common/StatCell.vue";
import StatusChip from "@/components/common/StatusChip.vue";
import NewSessionDialog from "@/components/sessions/NewSessionDialog.vue";
import { useAuthStore } from "@/stores/auth";
import { useUiStore } from "@/stores/ui";
import { CLINIC_TIME_ZONE, clinicDate, clinicHour } from "@/utils/date";

const auth = useAuthStore();
const ui = useUiStore();
const route = useRoute();
const loading = ref(true);
const error = ref("");
const dashboard = ref(null);
const doctors = ref([]);
const loadingDoctors = ref(false);
const doctorError = ref("");
const sessionDialog = ref(false);
const filters = reactive({
  search: "",
  doctor_id: null,
  status: null,
  date: clinicDate(),
});
const statusItems = [
  { title: "مغلقة", value: "closed" },
  { title: "مفتوحة قديمة", value: "open" },
];

const firstName = computed(() => auth.user?.name?.split(" ")[0] || "");
const greeting = computed(
  () =>
    `${clinicHour() < 12 ? "صباح الخير" : "مساء الخير"}، ${firstName.value}`,
);
const todayLabel = new Intl.DateTimeFormat("ar-LY-u-nu-latn", {
  timeZone: CLINIC_TIME_ZONE,
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
}).format(new Date());
const primaryStats = computed(() => {
  const stats = dashboard.value?.stats || [];
  const order = [
    ["sessions_today"],
    ["patients_today"],
    ["collections_today"],
    ["service_value_today", "service_value"],
  ];
  const ordered = order
    .map((keys) => stats.find((item) => keys.includes(item.key)))
    .filter(Boolean);
  const orderedKeys = new Set(ordered.map((item) => item.key));
  const remaining = stats.filter((item) => !orderedKeys.has(item.key));
  return [...ordered, ...remaining].slice(0, 4);
});
const visibleTodaySessions = computed(() =>
  (dashboard.value?.today_sessions || []).filter(
    (session) => session.status !== "cancelled",
  ),
);
const filteredSessions = computed(() =>
  visibleTodaySessions.value.filter((session) => {
    const query = filters.search.trim();
    return (
      (!query ||
        session.patient.name.includes(query) ||
        session.patient.phone.includes(query)) &&
      (!filters.doctor_id || session.doctor_id === Number(filters.doctor_id)) &&
      (!filters.status || session.status === filters.status) &&
      (!filters.date || session.opened_at.startsWith(filters.date))
    );
  }),
);
const hasActiveFilters = computed(() =>
  Boolean(filters.search || filters.doctor_id || filters.status),
);

onMounted(() => {
  if (route.query.forbidden)
    ui.showSnackbar("هذه الصفحة متاحة لمدير النظام فقط.", "warning");
  load();
  loadDoctors();
});

async function load() {
  loading.value = true;
  error.value = "";
  try {
    dashboard.value = (await getDashboard()).data;
  } catch (apiError) {
    error.value = apiError.message;
  } finally {
    loading.value = false;
  }
}

async function loadDoctors() {
  loadingDoctors.value = true;
  doctorError.value = "";
  try {
    doctors.value = await getDoctorOptions();
  } catch (cause) {
    doctorError.value = cause.message;
  } finally {
    loadingDoctors.value = false;
  }
}

function clearFilters() {
  filters.search = "";
  filters.doctor_id = null;
  filters.status = null;
}

const time = (value) =>
  new Intl.DateTimeFormat("ar-LY-u-nu-latn", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
const paymentLabel = (type) =>
  ({
    wallet_topup: "شحن رصيد",
    session_payment: "دفعة جلسة",
    subscription_payment: "دفعة اشتراك",
    debt_payment: "تسديد دين",
    refund: "استرجاع",
  })[type] || type;
</script>

<style scoped>
.dashboard-page {
  padding-bottom: 48px;
}
.dashboard-skeleton,
.dashboard-metrics {
  display: grid;
  width: 100%;
  grid-template-columns: repeat(
    var(--dashboard-stat-columns, 4),
    minmax(0, 1fr)
  );
  gap: var(--grid-gap);
}
.section-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}
.section-support {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: 13px;
}
.doctor-snapshot {
  display: grid;
  gap: 18px;
}
.snapshot-strip {
  display: grid;
  grid-auto-flow: column;
  grid-auto-columns: minmax(250px, 1fr);
  gap: 12px;
  overflow-x: auto;
  padding-bottom: 3px;
}
.snapshot-card {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px;
  background: var(--surface-subtle);
  border: 1px solid var(--border);
  border-radius: 14px;
}
.snapshot-avatar,
.follow-up-avatar {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  color: var(--accent);
  background: var(--accent-tint);
  border-radius: 50%;
  font-weight: 600;
}
.snapshot-avatar {
  width: 40px;
  height: 40px;
}
.snapshot-copy {
  min-width: 0;
  flex: 1;
  display: grid;
  gap: 8px;
}
.snapshot-copy > div:first-child {
  display: flex;
  justify-content: space-between;
  gap: 10px;
}
.snapshot-copy strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
}
.snapshot-copy span {
  color: var(--text-muted);
  font-size: 12px;
  white-space: nowrap;
}
.snapshot-track {
  height: 5px;
  overflow: hidden;
  background: var(--border);
  border-radius: var(--r-chip);
}
.snapshot-track span {
  display: block;
  height: 100%;
  background: var(--accent);
  border-radius: inherit;
}
.dashboard-content-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.75fr) minmax(320px, 0.75fr);
  grid-template-rows: repeat(2, minmax(0, 1fr));
  grid-template-areas: "activity follow-up" "financial recent";
  gap: var(--grid-gap);
  align-items: stretch;
}
.activity-card,
.follow-up-card,
.financial-card,
.recent-card {
  height: 100%;
  min-height: 0;
}
.activity-card {
  grid-area: activity;
  padding: 0;
  overflow: hidden;
}
.activity-heading {
  padding: 22px 24px 18px;
}
.activity-filters {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) 180px 150px auto;
  gap: 10px;
  align-items: start;
  padding: 12px 24px;
  background: var(--surface-subtle);
  border-block: 1px solid var(--border);
}
.activity-filters :deep(.v-field) {
  min-height: var(--control-h-sm);
}
.activity-feed {
  display: grid;
}
.activity-item {
  min-width: 0;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 14px;
  min-height: 78px;
  padding: 14px 18px;
  border-bottom: 1px solid var(--border);
  transition: background-color var(--duration-fast) ease;
}
.activity-item:last-child {
  border-bottom: 0;
}
.activity-item:hover {
  background: var(--surface-subtle);
}
.activity-icon {
  width: 42px;
  height: 42px;
  display: grid;
  place-items: center;
  color: var(--accent);
  background: var(--accent-tint);
  border-radius: 13px;
}
.activity-copy {
  min-width: 0;
}
.activity-copy p {
  margin: 0;
  color: var(--text);
  font-size: 14px;
}
.activity-copy p a,
.activity-copy p strong {
  color: var(--ink);
  font-weight: 600;
}
.activity-copy > span {
  display: block;
  max-width: 100%;
  margin-top: 3px;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.activity-meta {
  display: grid;
  justify-items: end;
  gap: 7px;
}
.activity-meta time {
  color: var(--text-muted);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.activity-meta > div {
  display: flex;
  gap: 5px;
}
.follow-up-card {
  grid-area: follow-up;
}
.follow-up-card,
.recent-card {
  display: grid;
  grid-template-rows: auto 1fr;
  gap: 16px;
}
.follow-up-list {
  display: grid;
}
.follow-up-list a {
  min-width: 0;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  min-height: 64px;
  padding: 10px 0;
  color: var(--text);
  border-top: 1px solid var(--border);
}
.follow-up-list a:first-child {
  border-top: 0;
}
.follow-up-avatar {
  width: 36px;
  height: 36px;
}
.follow-up-list strong,
.follow-up-list small {
  display: block;
}
.follow-up-list strong {
  color: var(--ink);
  font-size: 14px;
}
.follow-up-list small {
  max-width: 190px;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.follow-up-list em {
  padding: 4px 8px;
  color: var(--warning-fg);
  background: var(--warning-bg);
  border-radius: var(--r-chip);
  font-size: 12px;
  font-style: normal;
  font-weight: 600;
  white-space: nowrap;
}
.financial-card {
  grid-area: financial;
  display: grid;
  grid-template-rows: auto repeat(3, minmax(54px, 1fr));
  gap: 2px;
  padding: 22px;
  color: var(--text-on-dark);
  background: var(--ink);
  border-radius: var(--r-card);
  box-shadow: var(--shadow-card-hover);
}
.financial-card-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}
.financial-card-heading span {
  color: var(--text-on-dark-muted);
  font-size: 12px;
}
.financial-card-heading h2 {
  margin: 2px 0 0;
  font-size: var(--fs-section);
  font-weight: var(--fw-section);
}
.financial-card a {
  min-height: 54px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  color: var(--text-on-dark);
  border-top: 1px solid rgba(255, 255, 255, 0.1);
}
.financial-card a > span {
  color: var(--text-on-dark-muted);
  font-size: 13px;
}
.financial-card :deep(.money-text) {
  color: var(--text-on-dark);
  font-weight: 600;
}
.recent-card {
  grid-area: recent;
}
.movement-feed {
  display: grid;
  align-content: start;
}
.movement-feed > div {
  min-height: 56px;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  border-top: 1px solid var(--border);
}
.movement-feed > div:first-child {
  border-top: 0;
}
.movement-dot {
  width: 8px;
  height: 8px;
  background: var(--accent);
  border-radius: 50%;
}
.movement-feed strong,
.movement-feed small {
  display: block;
}
.movement-feed strong {
  font-size: 13px;
}
.movement-feed small {
  color: var(--text-muted);
  font-size: 12px;
}
.movement-feed :deep(.money-text) {
  font-size: 13px;
  font-weight: 600;
}
@media (max-width: 1180px) {
  .dashboard-skeleton,
  .dashboard-metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .dashboard-content-grid {
    grid-template-columns: minmax(0, 1.4fr) minmax(300px, 0.8fr);
  }
  .activity-filters {
    grid-template-columns: 1fr 1fr;
  }
}
@media (max-width: 900px) {
  .dashboard-content-grid {
    grid-template-columns: 1fr;
    grid-template-rows: auto;
    grid-template-areas: "activity" "follow-up" "financial" "recent";
  }
  .activity-card,
  .follow-up-card,
  .financial-card,
  .recent-card {
    height: auto;
  }
}
@media (max-width: 700px) {
  .dashboard-skeleton,
  .dashboard-metrics,
  .activity-filters {
    grid-template-columns: 1fr;
  }
  .activity-item {
    grid-template-columns: auto minmax(0, 1fr) auto;
  }
  .activity-meta {
    grid-column: 2 / -1;
    grid-row: 2;
    justify-items: start;
  }
  .snapshot-strip {
    grid-auto-columns: minmax(230px, 82vw);
  }
}
</style>
