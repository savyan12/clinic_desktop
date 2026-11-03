<template>
  <v-layout class="app-layout">
    <v-navigation-drawer
      v-model="drawer"
      location="right"
      :permanent="mdAndUp"
      :temporary="!mdAndUp"
      :width="280"
      class="app-sidebar"
    >
      <div class="brand-block">
        <div class="brand-mark"><ClinicLogo :size="56" /></div>
        <div class="brand-copy">
          <strong>{{ settings.clinic_name }}</strong>
          <span>{{ settings.clinic_subtitle }}</span>
        </div>
      </div>

      <v-list nav class="nav-list" aria-label="التنقل الرئيسي">
        <v-list-item
          v-for="item in visibleNavigation"
          :key="item.to"
          :to="item.to"
          :prepend-icon="item.icon"
          :title="item.label"
          rounded="lg"
          color="primary"
          exact
        />
      </v-list>

      <template #append>
        <div v-if="isMock" class="mock-switcher">
          <span class="caption-text">معاينة الصلاحيات</span>
          <v-btn-toggle
            v-model="mockRole"
            mandatory
            density="compact"
            color="primary"
            variant="outlined"
            @update:model-value="switchRole"
          >
            <v-btn value="receptionist">استقبال</v-btn>
            <v-btn value="admin">مدير</v-btn>
          </v-btn-toggle>
        </div>
      </template>
    </v-navigation-drawer>

    <v-main class="app-main">
      <header class="topbar">
        <div class="topbar-start">
          <v-btn
            v-if="!mdAndUp"
            icon="mdi-menu"
            variant="text"
            aria-label="فتح القائمة"
            @click="drawer = true"
          />
          <nav class="breadcrumbs" aria-label="مسار الصفحة">
            <span>{{ auth.isAdmin ? "الإدارة" : "الاستقبال" }}</span>
            <v-icon icon="mdi-chevron-left" size="18" />
            <strong>{{ route.meta.title || "الرئيسية" }}</strong>
          </nav>
        </div>
        <div class="user-cluster">
          <div class="user-copy">
            <strong>{{ auth.user?.name }}</strong>
            <span>{{ labelFor("roles", auth.role) }}</span>
          </div>
          <v-btn
            prepend-icon="mdi-logout-variant"
            variant="text"
            @click="handleLogout"
          >خروج</v-btn>
        </div>
      </header>
      <div class="route-stage">
        <router-view v-slot="{ Component, route: viewRoute }">
          <div :key="viewRoute.path" class="route-page" data-form-flow-page>
            <component :is="Component" />
          </div>
        </router-view>
      </div>
    </v-main>
  </v-layout>
</template>

<script setup>
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useDisplay } from "vuetify";
import { useRoute, useRouter } from "vue-router";
import { getSettings } from "@/api/settings";
import ClinicLogo from "@/components/common/ClinicLogo.vue";
import { labelFor } from "@/constants/labels";
import { useAuthStore } from "@/stores/auth";
import { useUiStore } from "@/stores/ui";
import { applyTypographySettings } from "@/utils/typography";

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const ui = useUiStore();
const { mdAndUp } = useDisplay();
const drawer = ref(mdAndUp.value);
const settings = reactive({
  clinic_name: "عيادة عين الحياة",
  clinic_subtitle: "للعلاج الطبيعي والتأهيل",
});
const mockRole = ref(auth.role);
const isMock = import.meta.env.VITE_USE_MOCK === "true";

const navigation = [
  {
    label: "الرئيسية",
    icon: "mdi-view-dashboard-outline",
    to: "/",
    roles: ["admin", "receptionist"],
  },
  {
    label: "الجلسات",
    icon: "mdi-clipboard-pulse-outline",
    to: "/sessions",
    roles: ["admin", "receptionist"],
  },
  {
    label: "المرضى",
    icon: "mdi-account-group-outline",
    to: "/patients",
    roles: ["admin", "receptionist"],
  },
  {
    label: "الاشتراكات",
    icon: "mdi-wallet-membership",
    to: "/subscriptions",
    roles: ["admin", "receptionist"],
  },
  {
    label: "الأطباء ومستحقاتهم",
    icon: "mdi-doctor",
    to: "/doctors",
    roles: ["admin"],
  },
  {
    label: "الخدمات",
    icon: "mdi-hand-heart-outline",
    to: "/services",
    roles: ["admin"],
  },
  {
    label: "الجهات الراعية",
    icon: "mdi-domain",
    to: "/sponsors",
    roles: ["admin"],
  },
  {
    label: "المستخدمون",
    icon: "mdi-account-cog-outline",
    to: "/users",
    roles: ["admin"],
  },
  {
    label: "التقارير",
    icon: "mdi-chart-box-outline",
    to: "/reports/sessions",
    roles: ["admin"],
  },
  {
    label: "سجل التدقيق",
    icon: "mdi-history",
    to: "/audit-log",
    roles: ["admin"],
  },
  {
    label: "إعدادات النظام",
    icon: "mdi-cog-outline",
    to: "/settings",
    roles: ["admin"],
  },
];

const visibleNavigation = computed(() =>
  navigation.filter((item) => item.roles.includes(auth.role)),
);

watch(mdAndUp, (value) => {
  drawer.value = value;
});
watch(
  () => auth.role,
  (value) => {
    mockRole.value = value;
  },
);

onMounted(async () => {
  try {
    const loadedSettings = (await getSettings()).data;
    Object.assign(settings, loadedSettings);
    applyTypographySettings(loadedSettings);
  } catch {
    /* Keep settings defaults if unavailable. */
  }
});

async function handleLogout() {
  await auth.logout();
  ui.showSnackbar("تم تسجيل الخروج بنجاح.", "info");
  router.push("/login");
}

async function switchRole(role) {
  await auth.switchMockRole(role);
  ui.showSnackbar(
    role === "admin" ? "تم تفعيل معاينة المدير." : "تم تفعيل معاينة الاستقبال.",
    "info",
  );
  if (route.meta.roles && !route.meta.roles.includes(role)) router.push("/");
}
</script>

<style scoped>
.app-layout {
  min-height: 100vh;
  background: var(--color-canvas);
}
.app-sidebar {
  background: var(--color-pearl) !important;
  border-left: 1px solid var(--color-hairline) !important;
  border-right: 0 !important;
}
.brand-block {
  display: flex;
  align-items: center;
  gap: 13px;
  padding: 28px 24px 22px;
}
.brand-mark {
  width: 56px;
  height: 56px;
  flex: 0 0 56px;
  border-radius: 16px;
  overflow: hidden;
  border: 1px solid var(--color-hairline);
  background: var(--color-canvas);
}
.brand-copy strong {
  display: block;
  font-size: 19px;
  font-weight: 600;
}
.brand-copy span {
  color: var(--color-ink-muted);
  font-size: 13px;
}
.nav-list {
  padding: 10px 14px;
}
.nav-list :deep(.v-list-item) {
  min-height: 52px;
  margin-bottom: 6px;
  border-radius: 13px !important;
  color: var(--color-ink-muted);
}
.nav-list :deep(.v-list-item--active) {
  color: var(--color-primary) !important;
  background: var(--color-info-bg) !important;
}
.nav-list :deep(.v-list-item-title) {
  font-size: 15px;
  font-weight: 400;
}
.mock-switcher {
  display: grid;
  gap: 10px;
  padding: 18px 20px 24px;
  border-top: 1px solid var(--color-hairline);
}
.app-main {
  min-width: 0;
  background: var(--color-canvas);
}
.route-stage {
  min-height: calc(100vh - 82px);
}
.route-page {
  min-height: inherit;
}
.topbar {
  height: 82px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding: 0 32px;
  border-bottom: 1px solid var(--color-hairline);
  background: var(--color-canvas);
}
.topbar-start,
.breadcrumbs,
.user-cluster {
  display: flex;
  align-items: center;
  gap: 10px;
}
.breadcrumbs {
  color: var(--color-ink-muted);
  font-size: 14px;
}
.breadcrumbs strong {
  color: var(--color-ink);
  font-weight: 400;
}
.user-cluster {
  gap: 16px;
}
.user-copy {
  text-align: left;
}
.user-copy strong,
.user-copy span {
  display: block;
}
.user-copy strong {
  font-size: 14px;
  font-weight: 600;
}
.user-copy span {
  color: var(--color-ink-muted);
  font-size: 13px;
}
@media (max-width: 640px) {
  .topbar {
    height: 70px;
    padding: 0 14px;
  }
  .route-stage {
    min-height: calc(100vh - 70px);
  }
  .breadcrumbs span,
  .breadcrumbs .v-icon,
  .user-copy {
    display: none;
  }
  .user-cluster {
    gap: 4px;
  }
}
</style>
