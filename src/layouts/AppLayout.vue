<template>
  <a class="skip-link" :class="{ 'skip-link-enabled': keyboardNavigation }" href="#main-content">تجاوز القائمة والانتقال إلى المحتوى</a>
  <v-layout class="app-layout">
    <v-navigation-drawer
      v-model="drawer"
      location="right"
      :permanent="lgAndUp"
      :temporary="!lgAndUp"
      :width="264"
      class="app-sidebar"
    >
      <div class="brand-block">
        <ClinicLogo :size="52" />
        <div class="brand-copy">
          <strong>{{ settings.clinic_name }}</strong>
          <span>{{ settings.clinic_subtitle }}</span>
        </div>
      </div>

      <nav class="sidebar-navigation" aria-label="التنقل الرئيسي">
        <section v-for="group in visibleNavigationGroups" :key="group.label" class="nav-group">
          <h2>{{ group.label }}</h2>
          <v-list nav density="compact" class="nav-list">
            <v-list-item
              v-for="item in group.items"
              :key="item.to"
              :to="item.to"
              :exact="item.to === '/'"
              :prepend-icon="item.icon"
              :title="item.label"
              rounded="md"
              color="primary"
            />
          </v-list>
        </section>
      </nav>

      <template #append>
        <div v-if="isMock" class="mock-switcher">
          <div>
            <strong>وضع المعاينة</strong>
            <span>بدّل الصلاحيات لاختبار الواجهة</span>
          </div>
          <v-btn-toggle
            v-model="mockRole"
            mandatory
            density="compact"
            color="primary"
            variant="outlined"
            class="segmented-control"
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
        <div class="topbar-context">
          <v-btn
            v-if="!lgAndUp"
            icon="mdi-menu"
            variant="text"
            aria-label="فتح القائمة الرئيسية"
            @click="drawer = true"
          />
          <button class="global-search-trigger" type="button" @click="openCommandPalette">
            <v-icon icon="mdi-magnify" size="20" aria-hidden="true" />
            <span>ابحث عن مريض أو انتقل إلى صفحة</span>
            <kbd>Ctrl K</kbd>
          </button>
        </div>

        <div class="user-cluster">
          <div class="user-avatar" aria-hidden="true">{{ userInitial }}</div>
          <div class="user-copy">
            <strong>{{ auth.user?.name }}</strong>
            <span>{{ labelFor('roles', auth.role) }}</span>
          </div>
          <v-btn
            icon="mdi-logout-variant"
            variant="text"
            aria-label="تسجيل الخروج"
            title="تسجيل الخروج"
            @click="handleLogout"
          />
        </div>
      </header>

      <div
        id="main-content"
        ref="mainContent"
        class="route-stage"
        tabindex="-1"
      >
        <router-view v-slot="{ Component, route: viewRoute }">
          <div :key="viewRoute.path" class="route-page" data-form-flow-page>
            <component :is="Component" />
          </div>
        </router-view>
      </div>
    </v-main>
  </v-layout>

  <v-dialog v-model="commandOpen" max-width="680" @after-enter="focusCommandInput">
    <v-card class="command-palette">
      <div class="command-search-row">
        <v-icon icon="mdi-magnify" size="22" aria-hidden="true" />
        <input
          ref="commandInput"
          v-model="commandQuery"
          type="search"
          placeholder="اسم المريض، رقم الهاتف، أو اسم الصفحة"
          aria-label="البحث العام"
          autocomplete="off"
          @input="searchPatients"
          @keydown.esc="commandOpen = false"
        >
        <kbd>Esc</kbd>
      </div>
      <div class="command-results" aria-live="polite">
        <section v-if="matchingNavigation.length">
          <h2>انتقال سريع</h2>
          <button v-for="item in matchingNavigation" :key="item.to" type="button" @click="goTo(item.to)">
            <v-icon :icon="item.icon" size="20" aria-hidden="true" />
            <span>{{ item.label }}</span>
            <v-icon class="command-arrow" icon="mdi-arrow-left" size="18" aria-hidden="true" />
          </button>
        </section>
        <section v-if="commandQuery.trim().length >= 2">
          <h2>المرضى</h2>
          <div v-if="commandLoading" class="command-loading"><v-progress-circular indeterminate size="20" width="2" /><span>جارٍ البحث…</span></div>
          <button v-for="patient in commandPatients" v-else :key="patient.id" type="button" @click="goTo(`/patients/${patient.id}`)">
            <span class="command-avatar" aria-hidden="true">{{ patient.name?.charAt(0) }}</span>
            <span><strong>{{ patient.name }}</strong><small dir="ltr">{{ patient.phone }}</small></span>
            <v-icon class="command-arrow" icon="mdi-arrow-left" size="18" aria-hidden="true" />
          </button>
          <p v-if="!commandLoading && !commandPatients.length" class="command-empty">لا توجد نتائج مطابقة.</p>
        </section>
        <p v-if="!matchingNavigation.length && commandQuery.trim().length < 2" class="command-hint">اكتب حرفين على الأقل للبحث في المرضى.</p>
      </div>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import { useRoute, useRouter } from 'vue-router'
import { getSettings } from '@/api/settings'
import { listPatients } from '@/api/patients'
import ClinicLogo from '@/components/common/ClinicLogo.vue'
import { labelFor } from '@/constants/labels'
import { useAuthStore } from '@/stores/auth'
import { useUiStore } from '@/stores/ui'
import { applyTypographySettings } from '@/utils/typography'
import { useDebounce } from '@/composables/useDebounce'

const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const ui = useUiStore()
const { lgAndUp } = useDisplay()
const drawer = ref(lgAndUp.value)
const mainContent = ref(null)
const settings = reactive({
  clinic_name: 'عيادة عين الحياة',
  clinic_subtitle: 'للعلاج الطبيعي والتأهيل',
})
const mockRole = ref(auth.role)
const isMock = import.meta.env.VITE_USE_MOCK === 'true'
const commandOpen = ref(false)
const commandInput = ref(null)
const commandQuery = ref('')
const commandPatients = ref([])
const commandLoading = ref(false)
const keyboardNavigation = ref(false)
const debounceCommand = useDebounce(250)

const navigationGroups = [
  {
    label: 'العمل اليومي',
    items: [
      { label: 'الرئيسية', icon: 'mdi-view-dashboard-outline', to: '/', roles: ['admin', 'receptionist'] },
      { label: 'الجلسات', icon: 'mdi-clipboard-pulse-outline', to: '/sessions', roles: ['admin', 'receptionist'] },
      { label: 'المرضى', icon: 'mdi-account-group-outline', to: '/patients', roles: ['admin', 'receptionist'] },
      { label: 'الاشتراكات', icon: 'mdi-wallet-membership', to: '/subscriptions', roles: ['admin', 'receptionist'] },
    ],
  },
  {
    label: 'الإدارة والمالية',
    items: [
      { label: 'الأطباء ومستحقاتهم', icon: 'mdi-doctor', to: '/doctors', roles: ['admin'] },
      { label: 'الخدمات', icon: 'mdi-hand-heart-outline', to: '/services', roles: ['admin'] },
      { label: 'الجهات الراعية', icon: 'mdi-domain', to: '/sponsors', roles: ['admin'] },
      { label: 'المشتريات والإيجار', icon: 'mdi-receipt-text-minus-outline', to: '/expenses', roles: ['admin'] },
      { label: 'التقارير', icon: 'mdi-chart-box-outline', to: '/reports/sessions', roles: ['admin'] },
    ],
  },
  {
    label: 'إدارة النظام',
    items: [
      { label: 'المستخدمون', icon: 'mdi-account-cog-outline', to: '/users', roles: ['admin'] },
      { label: 'سجل التدقيق', icon: 'mdi-history', to: '/audit-log', roles: ['admin'] },
      { label: 'إعدادات النظام', icon: 'mdi-cog-outline', to: '/settings', roles: ['admin'] },
    ],
  },
]

const visibleNavigationGroups = computed(() => navigationGroups
  .map((group) => ({ ...group, items: group.items.filter((item) => item.roles.includes(auth.role)) }))
  .filter((group) => group.items.length))
const flatNavigation = computed(() => visibleNavigationGroups.value.flatMap((group) => group.items))
const matchingNavigation = computed(() => {
  const query = commandQuery.value.trim()
  if (!query) return flatNavigation.value.slice(0, 6)
  return flatNavigation.value.filter((item) => item.label.includes(query)).slice(0, 6)
})

const userInitial = computed(() => auth.user?.name?.trim()?.charAt(0) || 'م')

watch(lgAndUp, (value) => {
  drawer.value = value
})

watch(
  () => auth.role,
  (value) => {
    mockRole.value = value
  },
)

watch(
  () => route.fullPath,
  async () => {
    if (!lgAndUp.value) drawer.value = false
    await nextTick()
    mainContent.value?.focus({ preventScroll: true })
  },
)

onMounted(async () => {
  window.addEventListener('keydown', handleGlobalShortcut)
  window.addEventListener('pointerdown', disableKeyboardNavigation)
  await nextTick()
  mainContent.value?.focus({ preventScroll: true })
  try {
    const loadedSettings = (await getSettings()).data
    Object.assign(settings, loadedSettings)
    applyTypographySettings(loadedSettings)
  } catch {
    /* Keep settings defaults if unavailable. */
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleGlobalShortcut)
  window.removeEventListener('pointerdown', disableKeyboardNavigation)
})

function handleGlobalShortcut(event) {
  if (event.key === 'Tab') keyboardNavigation.value = true
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault()
    openCommandPalette()
  }
}

function disableKeyboardNavigation() {
  keyboardNavigation.value = false
}

function openCommandPalette() {
  commandOpen.value = true
}

function focusCommandInput() {
  commandInput.value?.focus()
}

function searchPatients() {
  const query = commandQuery.value.trim()
  if (query.length < 2) {
    commandPatients.value = []
    commandLoading.value = false
    return
  }
  commandLoading.value = true
  debounceCommand(async () => {
    try {
      const response = await listPatients({ search: query, per_page: 6 })
      commandPatients.value = response.data
    } catch {
      commandPatients.value = []
    } finally {
      commandLoading.value = false
    }
  })
}

function goTo(path) {
  commandOpen.value = false
  commandQuery.value = ''
  commandPatients.value = []
  router.push(path)
}

async function handleLogout() {
  await auth.logout()
  ui.showSnackbar('تم تسجيل الخروج بنجاح.', 'info')
  router.push('/login')
}

async function switchRole(role) {
  await auth.switchMockRole(role)
  ui.showSnackbar(
    role === 'admin' ? 'تم تفعيل معاينة المدير.' : 'تم تفعيل معاينة الاستقبال.',
    'info',
  )
  if (route.meta.roles && !route.meta.roles.includes(role)) router.push('/')
}
</script>

<style scoped>
.app-layout {
  min-height: 100dvh;
  background: var(--color-background);
}

.app-sidebar {
  background: var(--color-surface) !important;
  border-inline-start: 1px solid var(--color-border) !important;
  border-inline-end: 0 !important;
  box-shadow: none !important;
}

.brand-block {
  min-height: 88px;
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-5) var(--space-5) var(--space-4);
  border-bottom: 1px solid var(--color-border);
}

.brand-copy {
  min-width: 0;
}

.brand-copy strong,
.brand-copy span {
  display: block;
}

.brand-copy strong {
  overflow: hidden;
  color: var(--color-foreground);
  font-size: 17px;
  font-weight: 600;
  line-height: 1.45;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.brand-copy span {
  margin-top: 2px;
  color: var(--color-muted-foreground);
  font-size: 12px;
  line-height: 1.45;
}

.sidebar-navigation {
  padding: var(--space-3) var(--space-3) var(--space-6);
}

.nav-group + .nav-group {
  margin-top: var(--space-4);
  padding-top: var(--space-4);
  border-top: 1px solid var(--color-border);
}

.nav-group h2 {
  margin: 0 10px var(--space-2);
  color: var(--color-muted-foreground);
  font-size: 12px;
  font-weight: 500 !important;
  line-height: 1.45;
}

.nav-list {
  padding: 0;
}

.nav-list :deep(.v-list-item) {
  min-height: 44px;
  margin-bottom: var(--space-1);
  padding-inline: 12px;
  color: var(--color-muted-foreground);
  border: 1px solid transparent;
  border-radius: var(--radius-md) !important;
}

.nav-list :deep(.v-list-item:hover) {
  color: var(--color-foreground);
  background: var(--color-surface-subtle);
}

.nav-list :deep(.v-list-item--active) {
  color: var(--color-primary) !important;
  background: var(--color-primary-subtle) !important;
  border-color: transparent;
  box-shadow: none;
}

.nav-list :deep(.v-list-item-title) {
  font-size: 14px;
  font-weight: 500;
}

.nav-list :deep(.v-icon) {
  font-size: 21px;
}

.mock-switcher {
  display: grid;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5) var(--space-5);
  background: var(--color-surface-raised);
  border-top: 1px solid var(--color-border);
}

.mock-switcher strong,
.mock-switcher span {
  display: block;
}

.mock-switcher strong {
  font-size: 13px;
  font-weight: 600;
}

.mock-switcher span {
  color: var(--color-muted-foreground);
  font-size: 12px;
}

.mock-switcher :deep(.v-btn-toggle) {
  width: 100%;
}

.mock-switcher :deep(.v-btn) {
  flex: 1;
}

.app-main {
  min-width: 0;
  background: var(--color-background);
}

.topbar {
  position: sticky;
  inset-block-start: 0;
  z-index: 20;
  height: var(--topbar-height);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-5);
  padding-inline: var(--space-8);
  background: color-mix(in srgb, var(--color-surface) 94%, transparent);
  backdrop-filter: blur(12px);
  border-bottom: 1px solid var(--color-border);
}

.topbar-context,
.user-cluster {
  min-width: 0;
  display: flex;
  align-items: center;
}

.topbar-context {
  flex: 1;
  gap: var(--space-3);
}

.global-search-trigger {
  width: min(460px, 100%);
  min-height: 40px;
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: 7px 12px;
  color: var(--color-muted-foreground);
  background: var(--color-surface-subtle);
  border: 1px solid var(--color-border);
  border-radius: 10px;
  cursor: pointer;
  text-align: start;
  font-size: 13px;
}

.global-search-trigger:hover {
  color: var(--color-foreground);
  border-color: var(--color-primary-border);
  background: var(--color-primary-subtle);
}

.global-search-trigger span {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

kbd {
  min-width: 42px;
  padding: 2px 7px;
  color: var(--color-muted-foreground);
  background: var(--color-surface);
  border: 1px solid var(--color-border-strong, var(--color-border));
  border-radius: 6px;
  box-shadow: 0 1px 1px rgba(16, 24, 40, 0.06);
  font: 500 11px/1.6 var(--font-sans);
  text-align: center;
}

.user-cluster {
  gap: var(--space-3);
}

.user-avatar {
  width: 36px;
  height: 36px;
  display: grid;
  flex: 0 0 36px;
  place-items: center;
  color: var(--color-primary);
  background: var(--color-primary-subtle);
  border: 1px solid var(--color-primary-border);
  border-radius: 50%;
  font-size: 15px;
  font-weight: 600;
}

.user-copy {
  min-width: 0;
  text-align: start;
}

.user-copy strong,
.user-copy span {
  display: block;
}

.user-copy strong {
  max-width: 180px;
  overflow: hidden;
  color: var(--color-foreground);
  font-size: 13px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.user-copy span {
  color: var(--color-muted-foreground);
  font-size: 12px;
}

.route-stage {
  min-height: calc(100dvh - var(--topbar-height));
}

.route-stage:focus {
  outline: none;
}

.route-page {
  min-height: inherit;
}

.command-palette {
  overflow: hidden;
  border-radius: 18px !important;
}

.command-search-row {
  min-height: 64px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 20px;
  border-bottom: 1px solid var(--color-border);
}

.command-search-row input {
  min-width: 0;
  flex: 1;
  color: var(--color-foreground);
  border: 0;
  outline: 0;
  background: transparent;
  font-size: 16px;
}

.command-search-row input::placeholder {
  color: var(--text-faint);
}

.command-results {
  max-height: min(66vh, 540px);
  overflow-y: auto;
  padding: 12px;
}

.command-results section + section {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--color-border);
}

.command-results h2 {
  margin: 0 8px 7px;
  color: var(--color-muted-foreground);
  font-size: 12px;
  font-weight: 600 !important;
}

.command-results button {
  width: 100%;
  min-height: 50px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 10px;
  color: var(--color-foreground);
  background: transparent;
  border: 0;
  border-radius: 10px;
  cursor: pointer;
  text-align: start;
}

.command-results button:hover,
.command-results button:focus-visible {
  background: var(--color-primary-subtle);
}

.command-results button > span:not(.command-avatar) {
  min-width: 0;
  flex: 1;
}

.command-results strong,
.command-results small {
  display: block;
}

.command-results strong {
  font-size: 14px;
  font-weight: 600;
}

.command-results small {
  margin-top: 1px;
  color: var(--color-muted-foreground);
  font-size: 12px;
  text-align: start;
}

.command-avatar {
  width: 34px;
  height: 34px;
  display: grid;
  flex: 0 0 34px;
  place-items: center;
  color: var(--color-primary);
  background: var(--color-primary-subtle);
  border-radius: 50%;
  font-weight: 600;
}

.command-arrow {
  color: var(--color-muted-foreground);
}

.command-loading,
.command-empty,
.command-hint {
  min-height: 54px;
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0;
  padding: 10px;
  color: var(--color-muted-foreground);
  font-size: 13px;
}

@media (max-width: 1180px) {
  .topbar {
    padding-inline: var(--space-6);
  }
}

@media (max-width: 640px) {
  .topbar {
    padding-inline: var(--space-3);
  }

  .global-search-trigger span,
  .global-search-trigger kbd,
  .user-copy {
    display: none;
  }

  .global-search-trigger {
    width: 42px;
    justify-content: center;
    padding: 0;
  }

  .user-cluster {
    gap: var(--space-1);
  }
}
</style>
