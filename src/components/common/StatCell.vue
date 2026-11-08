<template>
  <component
    :is="to ? RouterLink : 'div'"
    :to="to || undefined"
    class="stat-card"
    :class="[`stat-${variant}`, `tone-${resolvedTone}`, { 'stat-interactive': to }]"
  >
    <div class="stat-topline">
      <span class="stat-label">{{ label }}</span>
      <span class="stat-icon" aria-hidden="true"><v-icon :icon="resolvedIcon" size="21" /></span>
    </div>
    <MoneyText v-if="money" class="stat-value" :value="value" />
    <strong v-else class="stat-value ltr-value">{{ formatNumber(value) }}</strong>
    <span v-if="caption" class="stat-caption">{{ caption }}</span>
    <v-icon v-if="to" class="stat-affordance" icon="mdi-arrow-left" size="18" aria-hidden="true" />
  </component>
</template>

<script setup>
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import MoneyText from './MoneyText.vue'
import { formatNumber } from '@/composables/useMoney'

const props = defineProps({
  label: { type: String, required: true },
  value: { type: [String, Number], required: true },
  caption: { type: String, default: '' },
  money: { type: Boolean, default: false },
  to: { type: String, default: '' },
  icon: { type: String, default: '' },
  tone: { type: String, default: '' },
  variant: { type: String, default: 'default' },
  statKey: { type: String, default: '' },
})

const lookup = {
  sessions_today: ['mdi-clipboard-pulse-outline', 'blue'],
  patients_today: ['mdi-account-group-outline', 'violet'],
  service_value: ['mdi-receipt-text-outline', 'green'],
  service_value_today: ['mdi-receipt-text-outline', 'green'],
  collections_today: ['mdi-cash-check', 'amber'],
  debts: ['mdi-file-clock-outline', 'amber'],
  near_subscriptions: ['mdi-wallet-membership', 'violet'],
  doctor_dues: ['mdi-doctor', 'blue'],
}

const inferredKey = computed(() => props.statKey || Object.keys(lookup).find((key) => props.label.includes({
  sessions_today: 'جلسات', patients_today: 'المرضى', service_value: 'قيمة', service_value_today: 'قيمة', collections_today: 'المقبوضات', debts: 'الديون', near_subscriptions: 'اشتراكات', doctor_dues: 'مستحقات',
}[key])) || '')
const resolvedIcon = computed(() => props.icon || lookup[inferredKey.value]?.[0] || (props.money ? 'mdi-cash-multiple' : 'mdi-chart-box-outline'))
const resolvedTone = computed(() => props.tone || lookup[inferredKey.value]?.[1] || 'blue')
</script>

<style scoped>
.stat-card {
  position: relative;
  min-width: 0;
  min-height: 154px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: var(--sp-2);
  padding: 20px;
  color: inherit;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--r-card);
  box-shadow: var(--shadow-card);
  transition: border-color var(--duration-normal) ease, box-shadow var(--duration-normal) ease, transform var(--duration-normal) ease;
}

.stat-interactive:hover,
.stat-interactive:focus-visible {
  border-color: var(--accent-border);
  box-shadow: var(--shadow-card-hover);
  transform: translateY(-2px);
}

.stat-topline {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.stat-label {
  color: var(--text-muted);
  font-size: 14px;
  font-weight: 500;
}

.stat-icon {
  width: 40px;
  height: 40px;
  display: grid;
  flex: 0 0 40px;
  place-items: center;
  color: var(--accent);
  background: var(--accent-tint);
  border-radius: 12px;
}

.stat-value {
  align-self: flex-start;
  max-width: 100%;
  color: var(--ink);
  font-size: var(--fs-stat);
  line-height: 1.2;
  font-weight: var(--fw-stat);
  text-align: start;
  unicode-bidi: plaintext;
}

.stat-card :deep(.money-text) {
  display: block;
  align-self: flex-start;
}

.stat-caption {
  width: fit-content;
  max-width: 100%;
  padding: 3px 9px;
  color: var(--text-muted);
  background: color-mix(in srgb, var(--surface) 58%, transparent);
  border-radius: var(--r-chip);
  font-size: 12px;
  line-height: 1.5;
}

.stat-affordance {
  position: absolute;
  inset-inline-end: 18px;
  inset-block-end: 18px;
  color: var(--accent);
}

.stat-tinted {
  border-color: transparent;
  box-shadow: none;
}

.stat-tinted .stat-icon {
  background: var(--surface);
  box-shadow: 0 1px 3px rgba(16, 24, 40, 0.08);
}

.stat-dark {
  color: var(--text-on-dark);
  background: var(--ink);
  border-color: var(--ink);
}

.stat-dark .stat-label,
.stat-dark .stat-caption,
.stat-dark .stat-affordance {
  color: var(--text-on-dark-muted);
}

.stat-dark .stat-value {
  color: var(--text-on-dark);
}

.stat-dark .stat-icon {
  color: var(--text-on-dark);
  background: rgba(255, 255, 255, 0.12);
}

.tone-blue.stat-tinted { background: var(--kpi-blue); }
.tone-violet.stat-tinted { background: var(--kpi-violet); }
.tone-green.stat-tinted { background: var(--kpi-green); }
.tone-amber.stat-tinted { background: var(--kpi-amber); }
.tone-violet .stat-icon { color: var(--violet-fg); background: var(--violet-bg); }
.tone-green .stat-icon { color: var(--success-fg); background: var(--success-bg); }
.tone-amber .stat-icon { color: var(--warning-fg); background: var(--warning-bg); }
.tone-violet.stat-tinted .stat-icon,
.tone-green.stat-tinted .stat-icon,
.tone-amber.stat-tinted .stat-icon { background: var(--surface); }

@media (max-width: 640px) {
  .stat-card {
    min-height: 140px;
    padding: 18px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .stat-interactive:hover,
  .stat-interactive:focus-visible {
    transform: none;
  }
}
</style>
