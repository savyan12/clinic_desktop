<template>
  <div class="page-container page-stack">
    <PageHeader
      title="الخدمات"
      subtitle="الأسعار الحالية؛ تعديل السعر لا يغير الجلسات السابقة"
      action-label="إضافة خدمة"
      action-icon="mdi-plus"
      @action="openCreate"
    />
    <div class="surface-card service-filter" data-form-flow-filter>
      <v-text-field
        v-model="search"
        label="بحث باسم الخدمة"
        prepend-inner-icon="mdi-magnify"
        clearable
        @update:model-value="searchChanged"
      />
    </div>
    <DataTableCard
      :headers="headers"
      :items="services"
      :loading="loading"
      :error="error"
      :meta="meta"
      @retry="load"
      @page="changePage"
    ><template #rows="{ items }"><tr v-for="service in items" :key="service.id">
      <td>
        <strong>{{ service.name }}</strong>
      </td>
      <td><MoneyText :value="service.price" /></td>
      <td><span v-if="service.included_units">يشمل {{ service.included_units }} {{ service.unit_name }}<span class="addon-price">الإضافي <MoneyText :value="service.extra_unit_price" /> / {{ service.unit_name }}</span></span><span v-else class="caption-text">لا توجد ملحقات</span></td>
      <td><StatusChip type="active" :value="service.is_active" /></td>
      <td>{{ formatDate(service.updated_at) }}</td>
      <td>
        <v-menu><template #activator="{ props }"><v-btn
          v-bind="props"
          icon="mdi-dots-vertical"
          variant="text"
          aria-label="إجراءات الخدمة"
        /></template><v-list density="compact"><v-list-item
          prepend-icon="mdi-pencil-outline"
          title="تعديل"
          @click="openEdit(service)"
        /><v-list-item
          :prepend-icon="
            service.is_active
              ? 'mdi-eye-off-outline'
              : 'mdi-eye-outline'
          "
          :title="
            service.is_active ? 'تعطيل الخدمة' : 'إعادة تفعيل الخدمة'
          "
          @click="askToggle(service)"
        /></v-list></v-menu>
      </td></tr></template></DataTableCard>
  </div>
  <v-dialog v-model="dialog" persistent max-width="560"><v-card><v-card-title class="dialog-title">{{
    editing ? "تعديل الخدمة" : "إضافة خدمة"
  }}</v-card-title><v-card-text class="dialog-content"><InlineError v-if="dialogError" :message="dialogError" /><v-text-field
    v-model="form.name"
    label="اسم الخدمة"
    :error-messages="fieldErrors.name"
  /><v-text-field
    v-model="form.price"
    label="السعر الحالي"
    type="text"
    inputmode="decimal"
    suffix="د.ل"
    :error-messages="fieldErrors.price"
  /><div class="addon-fields"><v-text-field v-model.number="form.included_units" label="الوحدات المشمولة (اختياري)" type="number" min="1" :error-messages="fieldErrors.included_units" /><v-text-field v-model="form.unit_name" label="اسم الوحدة (مثال: كوب)" /><v-text-field v-model="form.extra_unit_price" label="سعر الوحدة الإضافية" type="text" inputmode="decimal" suffix="د.ل" /></div><p class="addon-help">اترك الحقول الثلاثة فارغة للخدمات التي لا تحتوي على ملحقات.</p><v-switch
    v-model="form.is_active"
    label="الخدمة نشطة"
    color="primary"
    hide-details
  /><v-textarea
    v-if="editing"
    v-model="form.reason"
    label="سبب تعديل السعر أو البيانات"
    rows="2"
  /></v-card-text><v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="saving" @click="dialog = false">إلغاء</v-btn><v-btn color="primary" data-form-flow-primary :loading="saving" @click="save">حفظ</v-btn></v-card-actions></v-card></v-dialog>
  <ConfirmDialog
    v-model="toggleDialog"
    :title="selected?.is_active ? 'تعطيل الخدمة' : 'إعادة تفعيل الخدمة'"
    :message="
      selected?.is_active
        ? 'لن تظهر الخدمة في الجلسات والاشتراكات الجديدة، وستبقى في السجلات السابقة.'
        : 'ستصبح الخدمة متاحة للاستخدام الجديد.'
    "
    :confirm-label="selected?.is_active ? 'تعطيل' : 'تفعيل'"
    :danger="selected?.is_active"
    :loading="saving"
    @confirm="toggle"
  />
</template>

<script setup>
import { onMounted, reactive, ref } from "vue";
import { createService, listServices, updateService } from "@/api/services";
import ConfirmDialog from "@/components/common/ConfirmDialog.vue";
import DataTableCard from "@/components/common/DataTableCard.vue";
import InlineError from "@/components/common/InlineError.vue";
import MoneyText from "@/components/common/MoneyText.vue";
import PageHeader from "@/components/common/PageHeader.vue";
import StatusChip from "@/components/common/StatusChip.vue";
import { decimal } from "@/composables/useMoney";
import { useDebounce } from "@/composables/useDebounce";
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();
const debounce = useDebounce();
const services = ref([]);
const meta = ref(null);
const search = ref("");
const page = ref(1);
const loading = ref(true);
const error = ref("");
const dialog = ref(false);
const toggleDialog = ref(false);
const editing = ref(false);
const selected = ref(null);
const saving = ref(false);
const dialogError = ref("");
const fieldErrors = reactive({});
const form = reactive({ name: "", price: null, included_units: null, unit_name: "", extra_unit_price: null, is_active: true, reason: "" });
const headers = [
  { title: "الخدمة", key: "name" },
  { title: "السعر الحالي", key: "price" },
  { title: "الملحقات", key: "addons" },
  { title: "الحالة", key: "status" },
  { title: "آخر تعديل", key: "updated" },
  { title: "الإجراءات", key: "actions" },
];
onMounted(load);
async function load() {
  loading.value = true;
  error.value = "";
  try {
    const response = await listServices({
      search: search.value,
      page: page.value,
    });
    services.value = response.data;
    meta.value = response.meta;
  } catch (apiError) {
    error.value = apiError.message;
  } finally {
    loading.value = false;
  }
}
function openCreate() {
  editing.value = false;
  selected.value = null;
  Object.keys(fieldErrors).forEach((key) => delete fieldErrors[key]);
  Object.assign(form, { name: "", price: null, included_units: null, unit_name: "", extra_unit_price: null, is_active: true, reason: "" });
  dialogError.value = "";
  dialog.value = true;
}
function openEdit(service) {
  editing.value = true;
  selected.value = service;
  Object.keys(fieldErrors).forEach((key) => delete fieldErrors[key]);
  Object.assign(form, {
    name: service.name,
    price: Number(service.price),
    included_units: service.included_units,
    unit_name: service.unit_name || "",
    extra_unit_price: service.extra_unit_price === null ? null : Number(service.extra_unit_price),
    is_active: service.is_active,
    reason: "",
  });
  dialogError.value = "";
  dialog.value = true;
}
function askToggle(service) {
  selected.value = service;
  toggleDialog.value = true;
}
async function save() {
  saving.value = true;
  dialogError.value = "";
  Object.keys(fieldErrors).forEach((key) => delete fieldErrors[key]);
  try {
    const hasAddons = form.included_units !== null && form.included_units !== "";
    const payload = { ...form, price: decimal(form.price), included_units: hasAddons ? Number(form.included_units) : null, unit_name: hasAddons ? form.unit_name : null, extra_unit_price: hasAddons ? decimal(form.extra_unit_price) : null };
    if (editing.value) await updateService(selected.value.id, payload);
    else await createService(payload);
    ui.showSnackbar(
      editing.value
        ? "تم تحديث الخدمة دون التأثير على الأسعار التاريخية."
        : "تمت إضافة الخدمة.",
    );
    dialog.value = false;
    await load();
  } catch (apiError) {
    dialogError.value = apiError.message;
    Object.assign(fieldErrors, apiError.errors);
  } finally {
    saving.value = false;
  }
}
async function toggle() {
  saving.value = true;
  try {
    await updateService(selected.value.id, {
      name: selected.value.name,
      price: selected.value.price,
      included_units: selected.value.included_units,
      unit_name: selected.value.unit_name,
      extra_unit_price: selected.value.extra_unit_price,
      is_active: !selected.value.is_active,
      reason: selected.value.is_active
        ? "تعطيل من شاشة الخدمات"
        : "إعادة تفعيل من شاشة الخدمات",
    });
    ui.showSnackbar(
      selected.value.is_active
        ? "تم تعطيل الخدمة للجلسات الجديدة."
        : "تمت إعادة تفعيل الخدمة.",
    );
    toggleDialog.value = false;
    await load();
  } catch (apiError) {
    ui.showSnackbar(apiError.message, "error");
  } finally {
    saving.value = false;
  }
}
function searchChanged() {
  page.value = 1;
  debounce(load);
}
function changePage(value) {
  page.value = value;
  load();
}
const formatDate = (value) =>
  new Intl.DateTimeFormat("ar-LY-u-nu-latn", { dateStyle: "medium" }).format(
    new Date(value),
  );
</script>

<style scoped>
.service-filter {
  max-width: 560px;
  padding: 18px 20px;
}
.dialog-title {
  padding: 24px 24px 4px;
  font-size: var(--font-section-size);
  font-weight: 600;
}
.dialog-content {
  display: grid;
  gap: 16px;
  padding: 12px 24px !important;
}
.dialog-actions {
  gap: 8px;
  justify-content: flex-end;
  padding: 12px 24px 24px;
}
.addon-price { display: block; margin-top: 3px; color: var(--color-ink-muted); font-size: 13px; }.addon-fields { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }.addon-help { margin: -8px 0 0; color: var(--color-ink-muted); font-size: 13px; }
@media (max-width: 640px) { .addon-fields { grid-template-columns: 1fr; } }
</style>
