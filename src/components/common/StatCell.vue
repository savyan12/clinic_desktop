<template>
  <component :is="to ? RouterLink : 'div'" :to="to || undefined" class="stat-cell">
    <span class="stat-label">{{ label }}</span>
    <MoneyText v-if="money" class="stat-value" :value="value" />
    <strong v-else class="stat-value ltr-value">{{ formatNumber(value) }}</strong>
    <span class="stat-caption">{{ caption }}</span>
  </component>
</template>

<script setup>
import { RouterLink } from 'vue-router'
import MoneyText from './MoneyText.vue'
import { formatNumber } from '@/composables/useMoney'

defineProps({
  label: { type: String, required: true },
  value: { type: [String, Number], required: true },
  caption: { type: String, default: '' },
  money: { type: Boolean, default: false },
  to: { type: String, default: '' },
})

</script>

<style scoped>
.stat-cell { min-height: 152px; display: flex; flex-direction: column; justify-content: center; gap: 5px; padding: 24px; color: inherit; border-inline-start: 1px solid var(--color-hairline); }
.stat-cell:first-child { border-inline-start: 0; }
a.stat-cell:hover { background: var(--color-pearl); }
.stat-label, .stat-caption { color: var(--color-ink-muted); }
.stat-label { font-size: 14px; }
.stat-value { color: var(--color-ink); font-size: 36px; line-height: 1.2; font-weight: 600; }
.stat-caption { font-size: 13px; }
@media (max-width: 720px) { .stat-cell { border-top: 1px solid var(--color-hairline); border-inline-start: 0; } .stat-cell:first-child { border-top: 0; } }
</style>
