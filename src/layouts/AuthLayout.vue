<template>
  <v-main class="auth-layout">
    <div class="auth-brand">
      <div class="brand-mark" aria-hidden="true">
        <ClinicLogo :size="84" alt="" />
      </div>
      <div>
        <strong>{{ settings.clinic_name }}</strong>
        <span>{{ settings.clinic_subtitle }}</span>
      </div>
    </div>
    <router-view />
  </v-main>
</template>

<script setup>
import { onMounted, reactive } from "vue";
import { getSettings } from "@/api/settings";
import ClinicLogo from "@/components/common/ClinicLogo.vue";
import { applyTypographySettings } from "@/utils/typography";

const settings = reactive({
  clinic_name: "عيادة عين الحياة",
  clinic_subtitle: "للعلاج الطبيعي والتأهيل",
});

onMounted(async () => {
  try {
    const loadedSettings = (await getSettings()).data;
    Object.assign(settings, loadedSettings);
    applyTypographySettings(loadedSettings);
  } catch {
    /* Login stays available with defaults. */
  }
});
</script>

<style scoped>
.auth-layout {
  min-height: 100vh;
  display: grid;
  place-items: center;
  align-content: center;
  gap: 32px;
  padding: 32px 16px;
  background: var(--color-pearl);
}
.auth-brand {
  display: flex;
  align-items: center;
  gap: 14px;
}
.auth-brand strong {
  display: block;
  font-size: 22px;
  font-weight: 600;
}
.auth-brand span {
  color: var(--color-ink-muted);
  font-size: 14px;
}
.brand-mark {
  width: 84px;
  height: 84px;
  flex: 0 0 84px;
  border-radius: 20px;
  overflow: hidden;
  border: 1px solid var(--color-hairline);
  background: var(--color-canvas);
}
</style>
