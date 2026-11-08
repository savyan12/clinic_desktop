<template>
  <v-main class="print-layout" :class="{ 'print-layout--custom': route.meta.customPrintTemplate }">
    <div class="print-toolbar no-print">
      <v-btn
        prepend-icon="mdi-arrow-right"
        variant="outlined"
        @click="router.back()"
      >رجوع</v-btn>
      <v-btn prepend-icon="mdi-printer-outline" color="primary" @click="print">طباعة</v-btn>
    </div>
    <div v-if="!route.meta.customPrintTemplate" class="print-heading">
      <div class="print-brand">
        <ClinicLogo v-if="settings.show_logo" :size="56" />
      </div>
      <div>
        <h1>{{ settings.clinic_name }}</h1>
        <p>{{ settings.clinic_subtitle }}</p>
        <span><bdi class="phone-value" dir="ltr">{{ settings.phone || '—' }}</bdi> · {{ settings.address || '—' }}</span>
      </div>
    </div>
    <router-view />
  </v-main>
</template>

<script setup>
import { onMounted, provide, reactive } from "vue";
import { useRoute, useRouter } from "vue-router";
import { getSettings } from "@/api/settings";
import ClinicLogo from "@/components/common/ClinicLogo.vue";
import { applyTypographySettings } from "@/utils/typography";

const route = useRoute();
const router = useRouter();
const settings = reactive({
  clinic_name: "عيادة عين الحياة",
  clinic_subtitle: "للعلاج الطبيعي والتأهيل",
  phone: "",
  address: "",
  receipt_footer: "",
  show_logo: true,
});
provide('printSettings', settings);
const print = () => window.print();
onMounted(async () => {
  const loadedSettings = (await getSettings()).data;
  Object.assign(settings, loadedSettings);
  applyTypographySettings(loadedSettings);
});
</script>

<style scoped>
.print-layout {
  --color-primary: #0066cc;
  --color-primary-focus: #0071e3;
  --color-ink: #1d1d1f;
  --color-ink-muted: #4f4f54;
  --color-canvas: #ffffff;
  --color-parchment: #f5f5f7;
  --color-pearl: #fafafc;
  --color-hairline: #e0e0e0;
  --color-info-bg: #e8f1fc;
  --color-success: #1a7f37;
  --color-success-bg: #e6f4ea;
  --color-danger: #c5221f;
  --color-danger-bg: #fce8e6;
  --color-warning: #9a5b00;
  --color-warning-bg: #fef3d7;
  --color-print-brand: #00784f;
  --color-print-brand-dark: #00694b;
  --color-print-tint: #eaf4ef;
  --color-print-border: #c9d4e2;
  --color-print-muted: #566277;
  width: min(100%, 900px);
  margin: 0 auto;
  padding: 32px;
  color: var(--color-ink);
  background: var(--color-canvas);
  line-height: 1.47;
}
.print-layout--custom {
  width: 100%;
  max-width: none;
  padding: 24px;
  background: var(--color-parchment);
}
.print-toolbar {
  display: flex;
  justify-content: space-between;
  margin-bottom: 28px;
}
.print-toolbar :deep(.v-btn) {
  border-radius: var(--radius-pill) !important;
  font-weight: 400 !important;
}
.print-heading {
  display: flex;
  align-items: center;
  gap: 16px;
  padding-bottom: 22px;
  border-bottom: 2px solid var(--color-ink);
}
.print-brand {
  width: 56px;
  height: 56px;
  display: grid;
  place-items: center;
  border: 1px solid var(--color-hairline);
  border-radius: 14px;
  overflow: hidden;
}
.print-layout :deep(.clinic-logo) {
  background: var(--color-canvas);
}
.print-heading h1 {
  margin: 0;
  font-size: 26px;
}
.print-heading p {
  margin: 2px 0;
  color: var(--color-ink-muted);
}
.print-heading span {
  font-size: 13px;
  color: var(--color-ink-muted);
}
@media print {
  .print-layout,
  .print-layout--custom {
    width: auto;
    max-width: none;
    margin: 0;
    padding: 0;
    background: var(--color-canvas);
  }
}
</style>
