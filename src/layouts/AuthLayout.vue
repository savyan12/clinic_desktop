<template>
  <v-main class="auth-layout" data-form-flow-page>
    <div class="auth-shell">
      <header class="auth-brand">
        <ClinicLogo :size="64" />
        <div>
          <strong>{{ settings.clinic_name }}</strong>
          <span>{{ settings.clinic_subtitle }}</span>
        </div>
      </header>
      <router-view />
      <footer class="auth-footer">
        <v-icon icon="mdi-shield-check-outline" size="18" aria-hidden="true" />
        <span>منظومة داخلية لإدارة بيانات العيادة</span>
      </footer>
    </div>
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
  min-height: 100dvh;
  display: grid;
  place-items: center;
  padding: var(--space-8) var(--space-4);
  background: var(--color-background);
}
.auth-shell {
  width: min(100%, 480px);
  display: grid;
  gap: var(--space-5);
}
.auth-brand {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
}
.auth-brand strong {
  display: block;
  color: var(--color-foreground);
  font-size: 20px;
  font-weight: 600;
}
.auth-brand span {
  display: block;
  margin-top: 2px;
  color: var(--color-muted-foreground);
  font-size: 13px;
}
.auth-footer {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  color: var(--color-muted-foreground);
  font-size: 13px;
}
</style>
