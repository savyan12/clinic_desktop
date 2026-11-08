<template>
  <v-app>
    <main v-if="startup.error" class="startup-state" role="alert" aria-live="assertive">
      <div class="startup-panel">
        <v-icon icon="mdi-alert-circle-outline" size="40" color="error" aria-hidden="true" />
        <div>
          <h1>تعذر تشغيل المنظومة</h1>
          <p>{{ startup.error }}</p>
        </div>
        <v-btn color="primary" prepend-icon="mdi-refresh" :loading="startup.loading" @click="retryStartup">
          إعادة المحاولة
        </v-btn>
      </div>
    </main>
    <main v-else-if="!startup.initialized || !routeReady" class="startup-state" aria-live="polite" aria-busy="true">
      <div class="startup-panel startup-panel-loading">
        <v-progress-circular indeterminate color="primary" :size="38" :width="3" />
        <div>
          <h1>جارٍ تشغيل المنظومة</h1>
          <p>يتم الآن التحقق من الخدمات وجلسة الدخول.</p>
        </div>
      </div>
    </main>
    <router-view v-else />
    <v-snackbar
      v-model="ui.snackbar.visible"
      :color="ui.snackbar.color"
      location="bottom start"
      :timeout="ui.snackbar.timeout"
      :role="ui.snackbar.color === 'error' ? 'alert' : 'status'"
      :aria-live="ui.snackbar.color === 'error' ? 'assertive' : 'polite'"
    >
      {{ ui.snackbar.message }}
      <template #actions>
        <v-btn variant="text" @click="ui.hideSnackbar">إغلاق</v-btn>
      </template>
    </v-snackbar>
  </v-app>
</template>

<script setup>
import { ref } from 'vue'
import { useFormFlow } from '@/composables/useFormFlow'
import { useRouter } from 'vue-router'
import { useStartupStore } from '@/stores/startup'
import { useUiStore } from '@/stores/ui'

const router = useRouter()
const startup = useStartupStore()
const ui = useUiStore()
const routeReady = ref(false)
useFormFlow()

router.isReady()
  .then(() => {
    routeReady.value = true
  })
  .catch((cause) => startup.fail(cause))

async function retryStartup() {
  routeReady.value = false
  if (!await startup.initialize({ force: true })) return
  const target = `${window.location.pathname}${window.location.search}${window.location.hash}`
  try {
    await router.replace(target)
    routeReady.value = true
  } catch (cause) {
    startup.fail(cause)
  }
}
</script>

<style scoped>
.startup-state {
  min-height: 100dvh;
  display: grid;
  place-items: center;
  padding: var(--space-6);
  background: var(--color-background);
}

.startup-panel {
  width: min(100%, 520px);
  display: grid;
  justify-items: start;
  gap: var(--space-5);
  padding: var(--space-8);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-dialog);
  box-shadow: var(--shadow-raised);
}

.startup-panel-loading {
  grid-template-columns: auto 1fr;
  align-items: center;
}

.startup-panel h1 {
  margin: 0;
  color: var(--color-foreground);
  font-size: 22px;
  line-height: 1.4;
}

.startup-panel p {
  margin: var(--space-2) 0 0;
  color: var(--color-muted-foreground);
  line-height: 1.7;
}
</style>
