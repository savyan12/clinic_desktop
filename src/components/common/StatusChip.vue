<template>
  <v-chip
    size="small"
    :class="['status-chip', `status-${tone}`, { 'status-chip-classic': classic }]"
    variant="flat"
  >
    <span class="status-label">{{ label }}</span>
  </v-chip>
</template>

<script setup>
import { computed } from 'vue'
import { labelFor } from '@/constants/labels'

const props = defineProps({
  type: { type: String, required: true },
  value: { type: [String, Boolean], required: true },
  classic: { type: Boolean, default: false },
})

const label = computed(() => {
  const group = props.type === 'session' ? 'sessionStatus'
    : props.type === 'subscription' ? 'subscriptionStatus'
      : props.type === 'debt' ? 'debtStatus'
        : props.type === 'expense' ? 'expenseStatus'
        : props.type === 'coverage' ? 'coverage' : 'active'
  return labelFor(group, props.value)
})

const tone = computed(() => {
  if (props.type === 'session') return { open: 'info', closed: 'success', cancelled: 'danger' }[props.value] || 'neutral'
  if (props.type === 'subscription') return { active: 'success', completed: 'neutral', cancelled: 'danger' }[props.value] || 'neutral'
  if (props.type === 'debt') return { open: 'warning', partial: 'info', paid: 'success', void: 'neutral' }[props.value] || 'neutral'
  if (props.type === 'expense') return { active: 'success', cancelled: 'danger' }[props.value] || 'neutral'
  if (props.type === 'active') return props.value ? 'success' : 'neutral'
  if (props.type === 'coverage') return props.value === 'subscription' ? 'violet' : 'info'
  return 'neutral'
})

</script>

<style scoped>
.status-chip {
  --chip-fg: var(--text-muted);
  --chip-bg: var(--surface-subtle);
  --chip-border: var(--text-muted);
  min-height: 32px;
  height: auto !important;
  padding: 3px 12px !important;
  color: var(--chip-fg);
  background: var(--chip-bg);
  border: 2px solid var(--chip-border);
  border-radius: var(--radius-pill) !important;
  font-size: 13px;
  font-weight: 600;
  line-height: 1.25;
}

.status-chip :deep(.v-chip__content) {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  white-space: nowrap;
}

.status-label {
  padding-inline: 1px;
}

.status-info { --chip-fg: var(--info-fg); --chip-bg: var(--info-bg); --chip-border: var(--info-fg); }
.status-success { --chip-fg: var(--success-fg); --chip-bg: var(--success-bg); --chip-border: var(--success-solid); }
.status-danger { --chip-fg: var(--danger-fg); --chip-bg: var(--danger-bg); --chip-border: var(--danger-solid); }
.status-warning { --chip-fg: var(--warning-fg); --chip-bg: var(--warning-bg); --chip-border: var(--warning-solid); }
.status-violet { --chip-fg: var(--violet-fg); --chip-bg: var(--violet-bg); --chip-border: var(--violet-fg); }
.status-neutral { --chip-fg: var(--text); --chip-bg: var(--surface-subtle); --chip-border: var(--text-muted); }
.status-chip-classic {
  min-height: 24px;
  padding-inline: 10px !important;
  border: 0;
  font-weight: 600;
}
.status-chip-classic .status-label { padding-inline: 0; }
.status-chip-classic.status-info { color: var(--color-primary); background: var(--color-info-bg); }
.status-chip-classic.status-success { color: var(--color-success); background: var(--color-success-bg); }
.status-chip-classic.status-danger { color: var(--color-danger); background: var(--color-danger-bg); }
.status-chip-classic.status-warning { color: var(--color-warning); background: var(--color-warning-bg); }
.status-chip-classic.status-violet { color: var(--violet-fg); background: var(--violet-bg); }
.status-chip-classic.status-neutral { color: var(--color-ink-muted); background: var(--color-parchment); }
</style>
