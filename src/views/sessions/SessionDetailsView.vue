<template>
  <div class="page-container page-stack">
    <template v-if="loading"><v-skeleton-loader type="heading, image, article" /></template>
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <template v-else-if="session">
      <PageHeader
        :title="'الجلسة ' + session.code"
        :subtitle="session.patient.name + ' · ' + formatDate(session.opened_at)"
      >
        <template #actions>
          <v-btn
            v-if="session.status === 'closed' && session.type === 'individual'"
            color="primary"
            prepend-icon="mdi-plus-circle-outline"
            @click="additionDialog = true"
          >إضافة خدمات</v-btn>
          <v-btn
            :to="'/print/session/' + session.id"
            variant="outlined"
            prepend-icon="mdi-printer-outline"
          >طباعة</v-btn>
          <v-btn
            v-if="session.status === 'closed' && isAdmin"
            variant="outlined"
            prepend-icon="mdi-file-edit-outline"
            @click="openCorrection"
          >تصحيح الجلسة</v-btn>
          <v-btn
            v-if="session.status === 'closed' && isAdmin"
            color="error"
            variant="outlined"
            prepend-icon="mdi-cancel"
            @click="askCancel"
          >إلغاء جلسة مغلقة</v-btn>
        </template>
      </PageHeader>

      <div class="session-banner" :class="'banner-' + session.status">
        <StatusChip type="session" :value="session.status" />
        <span
          v-if="session.status === 'closed' && session.type === 'subscription'"
        >سُجل حضور جلسة الاشتراك والطبيب المشرف دون إنشاء عمولة أو حركة مالية
          مستقلة.</span>
        <span v-else-if="session.status === 'closed'">سُجلت الجلسة في النظام.</span>
        <span v-else-if="session.status === 'cancelled'">أُلغيت الجلسة: {{ session.cancel_reason }}</span>
        <span v-else>هذه جلسة قديمة مفتوحة؛ لا يستطيع موظف الاستقبال تعديلها.</span>
      </div>

      <section class="detail-grid">
        <div class="detail-item">
          <span class="detail-label">المريض</span><router-link :to="'/patients/' + session.patient.id"><strong>{{ session.patient.name }}</strong></router-link><span class="sub-value phone-value">{{
            session.patient.phone
          }}</span>
        </div>
        <div class="detail-item">
          <span class="detail-label">{{
            session.type === "subscription" ? "الطبيب المشرف" : "الطبيب المسؤول"
          }}</span><strong>{{ session.doctor.name }}</strong><span class="sub-value">{{
            session.type === "subscription"
              ? "إشراف تشغيلي دون عمولة"
              : `نسبة محفوظة ${session.commission_rate || 0}%`
          }}</span>
        </div>
        <div class="detail-item">
          <span class="detail-label">نوع الجلسة</span><strong>{{ labelFor("sessionType", session.type) }}</strong><router-link
            v-if="session.subscription_id"
            :to="'/subscriptions/' + session.subscription_id"
          >عرض الاشتراك</router-link>
        </div>
        <div class="detail-item">
          <span class="detail-label">سجلها</span><strong>{{ session.created_by_user?.name }}</strong><span class="sub-value">{{ formatDate(session.opened_at) }}</span>
        </div>
      </section>

      <section class="two-column">
        <div class="surface-card">
          <div class="section-row">
            <h2 class="section-heading">الخدمات والملحقات</h2>
            <span class="caption-text">{{ session.services.length }} بند</span>
          </div>
          <div class="line-list">
            <div v-for="service in session.services" :key="service.id">
              <div class="service-description">
                <strong>{{ service.name }}</strong>
                <span v-if="service.item_type === 'extra_units'">ملحقات فقط: {{ service.extra_units }}
                  {{ service.unit_name || "وحدة" }}</span>
                <span v-else-if="service.included_units || service.extra_units">يشمل {{ service.included_units }} + إضافي
                  {{ service.extra_units }}
                  {{ service.unit_name || "وحدة" }}</span>
                <span v-if="service.is_addition" class="addition-label">مضافة بعد إغلاق الجلسة</span>
              </div>
              <MoneyText :value="service.price" />
            </div>
          </div>
          <div class="totals-list">
            <div>
              <span>إجمالي الخدمات</span><MoneyText :value="session.services_total" />
            </div>
            <div><span>الخصم</span><MoneyText :value="session.discount" /></div>
            <div class="net">
              <strong>الصافي</strong><MoneyText :value="session.net_total" />
            </div>
          </div>
        </div>
        <div class="surface-card">
          <h2 class="section-heading">التسوية المالية</h2>
          <div class="coverage-heading">
            <StatusChip
              type="coverage"
              :value="session.settlement.coverage"
            /><span>{{ coverageDescription }}</span>
          </div>
          <div class="totals-list settlement-list">
            <div>
              <span>من الرصيد</span><MoneyText :value="session.settlement.wallet_amount" />
            </div>
            <div>
              <span>مدفوع</span><MoneyText :value="session.settlement.paid_amount" />
            </div>
            <div>
              <span>دين</span><MoneyText :value="session.settlement.debt_amount" />
            </div>
          </div>
          <div
            v-if="session.status === 'closed' && session.type === 'individual'"
            class="commission-box"
          >
            <span>مستحق الطبيب من هذه الجلسة</span><MoneyText :value="session.commission_amount" />
          </div>
          <div
            v-else-if="
              session.status === 'closed' &&
                session.type === 'subscription' &&
                session.commission_amount != null
            "
            class="commission-box historical"
          >
            <span>عمولة تاريخية محفوظة قبل تغيير السياسة</span><MoneyText :value="session.commission_amount" />
          </div>
          <div v-if="session.additions?.length" class="addition-summary">
            <strong>إضافات مثبتة: {{ session.additions.length }}</strong><span>كل إضافة محفوظة كسجل مستقل مع تسويتها.</span>
          </div>
        </div>
      </section>

      <section class="surface-card lifecycle-card">
        <h2 class="section-heading">سجل الحالة</h2>
        <div class="timeline">
          <div>
            <span class="timeline-dot"></span>
            <div>
              <strong>سُجلت الجلسة</strong><small>{{ formatDate(session.opened_at) }} ·
                {{ session.created_by_user?.name }}</small>
            </div>
          </div>
          <div v-if="session.closed_at">
            <span class="timeline-dot success"></span>
            <div>
              <strong>أُغلقت مباشرة</strong><small>{{ formatDate(session.closed_at) }} ·
                {{
                  session.closed_by_user?.name || session.created_by_user?.name
                }}</small>
            </div>
          </div>
          <div v-for="addition in session.additions" :key="addition.id">
            <span class="timeline-dot addition"></span>
            <div>
              <strong>أضيفت خدمات بقيمة
                <MoneyText :value="addition.net_total" /></strong><small>{{ formatDate(addition.created_at) }}</small>
            </div>
          </div>
          <div v-if="session.cancelled_at">
            <span class="timeline-dot danger"></span>
            <div>
              <strong>أُلغيت الجلسة</strong><small>{{ session.cancel_reason }}</small>
            </div>
          </div>
        </div>
      </section>
    </template>
  </div>

  <ConfirmDialog
    v-model="confirm.open"
    :title="confirm.title"
    :message="confirm.message"
    :confirm-label="confirm.label"
    require-reason
    danger
    :loading="confirm.loading"
    @confirm="executeCancel"
  />
  <AddSessionServicesDialog
    v-if="session?.type === 'individual'"
    v-model="additionDialog"
    :session="session"
    @saved="servicesAdded"
  />

  <v-dialog v-model="correctionDialog" persistent max-width="620">
    <v-card>
      <v-card-title class="dialog-title">تصحيح جلسة مغلقة</v-card-title>
      <v-card-text class="correction-body">
        <p>
          سيُحفظ القديم والجديد، وتُطبق الفروقات بحركات تصحيحية دون محو السجل
          الأصلي.
        </p>
        <InlineError v-if="correctionError" :message="correctionError" />
        <InlineError
          v-if="optionsError"
          :message="optionsError"
          retry
          @retry="loadOptions"
        />
        <v-select
          v-if="session?.type === 'subscription'"
          v-model="correction.doctor_id"
          label="الطبيب المصحح"
          :items="doctors"
          item-title="name"
          item-value="id"
          :loading="loadingOptions"
          :disabled="loadingOptions"
        />
        <template v-else>
          <v-text-field
            v-model="correction.discount"
            label="الخصم المصحح"
            type="text"
            inputmode="decimal"
            suffix="د.ل"
          />
          <div class="form-grid">
            <v-text-field
              v-model="correction.wallet_amount"
              label="من الرصيد"
              type="text"
              inputmode="decimal"
              suffix="د.ل"
            />
            <v-text-field
              v-model="correction.paid_amount"
              label="مدفوع"
              type="text"
              inputmode="decimal"
              suffix="د.ل"
            />
            <v-select
              v-model="correction.method"
              label="طريقة الدفع"
              :items="paymentMethods"
              :disabled="numeric(correction.paid_amount) <= 0"
            />
            <v-select
              v-model="correction.sponsor_id"
              label="جهة الدين"
              :items="sponsors"
              item-title="name"
              item-value="id"
              clearable
              :loading="loadingOptions"
              :disabled="numeric(correctionDebtAmount) <= 0 || loadingOptions"
            />
          </div>
          <InlineError
            v-if="correctionOverAllocated"
            message="مجموع الرصيد والمدفوع يتجاوز الصافي المصحح."
          />
        </template>
        <v-textarea
          v-model="correction.reason"
          label="سبب التصحيح (مطلوب)"
          rows="3"
        />
        <div v-if="session?.type === 'individual'" class="impact-preview">
          <span>صافي قديم <MoneyText :value="session?.net_total" /></span><span>صافي جديد <MoneyText :value="correctedNet" /></span><span>دين جديد <MoneyText :value="correctionDebtAmount" /></span>
        </div>
        <div v-else class="subscription-correction-note">
          تصحيح جلسة الاشتراك يغيّر الطبيب المشرف فقط، ولا ينشئ أو يعدّل أي
          عمولة.
        </div>
      </v-card-text>
      <v-card-actions class="dialog-actions"><v-btn
        variant="text"
        :disabled="correcting"
        @click="correctionDialog = false"
      >تراجع</v-btn><v-btn
        color="primary"
        data-form-flow-primary
        :loading="correcting"
        :disabled="
          correctionOverAllocated || loadingOptions || Boolean(optionsError)
        "
        @click="applyCorrection"
      >تأكيد التصحيح</v-btn></v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { getDoctorOptions } from "@/api/doctors";
import { getSponsorOptions } from "@/api/sponsors";
import { useDialogOptions } from "@/composables/useDialogOptions";
import { cancelSession, correctSession, getSession } from "@/api/sessions";
import AddSessionServicesDialog from "@/components/sessions/AddSessionServicesDialog.vue";
import ConfirmDialog from "@/components/common/ConfirmDialog.vue";
import InlineError from "@/components/common/InlineError.vue";
import MoneyText from "@/components/common/MoneyText.vue";
import PageHeader from "@/components/common/PageHeader.vue";
import StatusChip from "@/components/common/StatusChip.vue";
import { labelFor, paymentMethods } from "@/constants/labels";
import { decimal, formatMoney, numeric } from "@/composables/useMoney";
import { useIdempotency } from "@/composables/useIdempotency";
import { usePermissions } from "@/composables/usePermissions";
import { useUiStore } from "@/stores/ui";

const route = useRoute();
const router = useRouter();
const ui = useUiStore();
const { isAdmin } = usePermissions();
const { key, reset } = useIdempotency();
const session = ref(null);
const loading = ref(true);
const error = ref("");
const additionDialog = ref(false);
const confirm = reactive({
  open: false,
  title: "",
  message: "",
  label: "",
  loading: false,
});
const correctionDialog = ref(false);
const {
  data: options,
  loading: loadingOptions,
  error: optionsError,
  load: loadOptions,
} = useDialogOptions(
  () => correctionDialog.value,
  async () => {
    if (session.value.type === "subscription")
      return { doctors: await getDoctorOptions(), sponsors: [] };
    return { doctors: [], sponsors: await getSponsorOptions() };
  },
);
const doctors = computed(() => options.value?.doctors || []);
const sponsors = computed(() => options.value?.sponsors || []);
const correcting = ref(false);
const correctionError = ref("");
const correction = reactive({
  doctor_id: null,
  discount: 0,
  wallet_amount: 0,
  paid_amount: 0,
  method: "cash",
  sponsor_id: null,
  reason: "",
});
const correctedNet = computed(() =>
  decimal(
    Math.max(
      0,
      numeric(session.value?.services_total) - numeric(correction.discount),
    ),
  ),
);
const correctionOverAllocated = computed(
  () =>
    numeric(correction.wallet_amount) + numeric(correction.paid_amount) >
    numeric(correctedNet.value) + 0.0005,
);
const correctionDebtAmount = computed(() =>
  decimal(
    Math.max(
      0,
      numeric(correctedNet.value) -
        numeric(correction.wallet_amount) -
        numeric(correction.paid_amount),
    ),
  ),
);
const coverageDescription = computed(
  () =>
    ({
      subscription: "التغطية من الاشتراك لا تمثل قبضًا جديدًا.",
      wallet: "الخصم من الرصيد لا يمثل قبضًا جديدًا.",
      debt: "القيمة مسجلة كدين مستحق.",
      mixed: "القيمة موزعة على أكثر من جزء.",
      cash: "دفعة نقدية مثبتة.",
      transfer: "تحويل مصرفي مثبت.",
    })[session.value?.settlement.coverage],
);

onMounted(load);
async function load() {
  loading.value = true;
  error.value = "";
  try {
    const sessionResponse = await getSession(route.params.id);
    session.value = sessionResponse.data;
  } catch (apiError) {
    error.value = apiError.message;
  } finally {
    loading.value = false;
  }
}
function askCancel() {
  reset();
  Object.assign(confirm, {
    open: true,
    title: "إلغاء جلسة مغلقة",
    message: "سيتم إنشاء الحركات العكسية اللازمة دون حذف الحركات الأصلية.",
    label: "تأكيد الإلغاء",
  });
}
async function executeCancel(reason) {
  confirm.loading = true;
  try {
    await cancelSession(session.value.id, { reason }, key.value);
    ui.showSnackbar("تم إلغاء الجلسة وإنشاء الحركات العكسية وتوثيق السبب.");
    confirm.open = false;
    await router.replace('/sessions');
  } catch (apiError) {
    ui.showSnackbar(apiError.message, "error");
  } finally {
    confirm.loading = false;
  }
}
async function servicesAdded() {
  ui.showSnackbar("تمت إضافة الخدمات إلى الجلسة وتثبيت تسويتها ومستحق الطبيب.");
  await load();
}
function openCorrection() {
  reset();
  Object.assign(correction, {
    doctor_id: session.value.doctor_id,
    discount: Number(session.value.discount),
    wallet_amount: Number(session.value.settlement.wallet_amount),
    paid_amount: Number(session.value.settlement.paid_amount),
    method: session.value.settlement.method || "cash",
    sponsor_id: session.value.settlement.sponsor_id,
    reason: "",
  });
  correctionError.value = "";
  correctionDialog.value = true;
}
async function applyCorrection() {
  correcting.value = true;
  correctionError.value = "";
  try {
    const changes =
      session.value.type === "subscription"
        ? { doctor_id: correction.doctor_id }
        : {
            discount: decimal(correction.discount),
            settlement: {
              wallet_amount: decimal(correction.wallet_amount),
              paid_amount: decimal(correction.paid_amount),
              debt_amount: correctionDebtAmount.value,
              method:
                numeric(correction.paid_amount) > 0 ? correction.method : null,
              sponsor_id:
                numeric(correctionDebtAmount.value) > 0
                  ? correction.sponsor_id
                  : null,
            },
          };
    const response = await correctSession(
      session.value.id,
      { ...changes, reason: correction.reason },
      key.value,
    );
    ui.showSnackbar(
      "تم التصحيح. فرق الصافي " +
        formatMoney(response.data.financial_impact.net_delta) +
        ".",
    );
    correctionDialog.value = false;
    await load();
  } catch (apiError) {
    correctionError.value = apiError.message;
  } finally {
    correcting.value = false;
  }
}
const formatDate = (value) =>
  value
    ? new Intl.DateTimeFormat("ar-LY-u-nu-latn", {
        dateStyle: "medium",
        timeStyle: "short",
      }).format(new Date(value))
    : "—";
</script>

<style scoped>
.session-banner {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px 20px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-lg);
}
.banner-open {
  background: var(--color-info-bg);
}
.banner-closed {
  background: var(--color-success-bg);
}
.banner-cancelled {
  background: var(--color-danger-bg);
}
.section-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
.sub-value {
  display: block;
  margin-top: 4px;
  color: var(--color-ink-muted);
  font-size: 13px;
}
.two-column {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 24px;
}
.line-list,
.totals-list {
  display: grid;
  margin-top: 14px;
}
.line-list > div,
.totals-list > div {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  min-height: 54px;
  align-items: center;
  border-top: 1px solid var(--color-hairline);
}
.line-list > div:first-child {
  border-top: 0;
}
.service-description,
.service-description span {
  display: block;
}
.service-description span {
  margin-top: 3px;
  color: var(--color-ink-muted);
  font-size: 13px;
}
.service-description .addition-label {
  width: fit-content;
  padding: 2px 8px;
  color: var(--color-primary);
  border-radius: var(--radius-pill);
  background: var(--color-info-bg);
}
.totals-list .net {
  font-size: 18px;
}
.coverage-heading {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 16px 0;
  color: var(--color-ink-muted);
}
.commission-box {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-top: 18px;
  padding: 14px;
  color: var(--color-success);
  border-radius: 12px;
  background: var(--color-success-bg);
}
.commission-box :deep(.money-text) {
  font-size: 19px;
  font-weight: 600;
}
.addition-summary {
  display: grid;
  gap: 3px;
  margin-top: 14px;
  padding: 14px;
  border-radius: var(--radius-field);
  background: var(--color-info-bg);
}
.addition-summary span {
  color: var(--color-ink-muted);
  font-size: 13px;
}
.commission-box.historical {
  color: var(--color-warning);
  background: var(--color-warning-bg);
}
.lifecycle-card {
  display: grid;
  gap: 18px;
}
.timeline {
  display: grid;
  gap: 18px;
}
.timeline > div {
  display: flex;
  gap: 12px;
}
.timeline-dot {
  width: 12px;
  height: 12px;
  margin-top: 6px;
  flex: 0 0 12px;
  border-radius: 50%;
  background: var(--color-primary);
}
.timeline-dot.success {
  background: var(--color-success);
}
.timeline-dot.danger {
  background: var(--color-danger);
}
.timeline-dot.addition {
  background: var(--color-warning);
}
.timeline strong,
.timeline small {
  display: block;
}
.timeline strong {
  font-weight: 600;
}
.timeline small {
  margin-top: 2px;
  color: var(--color-ink-muted);
}
.dialog-title {
  padding: 24px 24px 4px;
  font-size: var(--font-section-size);
  font-weight: 600;
}
.correction-body {
  display: grid;
  gap: 16px;
  padding: 12px 24px !important;
}
.correction-body > p {
  margin: 0;
  color: var(--color-ink-muted);
}
.form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}
.impact-preview {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}
.impact-preview span {
  display: grid;
  gap: 4px;
  padding: 12px;
  border-radius: 10px;
  background: var(--color-pearl);
}
.dialog-actions {
  gap: 8px;
  justify-content: flex-end;
  padding: 12px 24px 24px;
}
.subscription-correction-note {
  padding: 14px;
  color: var(--color-ink-muted);
  border-radius: 12px;
  background: var(--color-info-bg);
}
@media (max-width: 800px) {
  .two-column,
  .form-grid {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 640px) {
  .impact-preview {
    grid-template-columns: 1fr;
  }
}
</style>
