<template>
  <v-chip size="small" :class="['status-chip', `status-${tone}`]" variant="flat">
    {{ label }}
  </v-chip>
</template>

<script setup>
import { computed } from 'vue'
import { labelFor } from '@/constants/labels'

const props = defineProps({
  type: { type: String, required: true },
  value: { type: [String, Boolean], required: true },
})

const label = computed(() => {
  const group = props.type === 'session' ? 'sessionStatus'
    : props.type === 'subscription' ? 'subscriptionStatus'
      : props.type === 'debt' ? 'debtStatus'
        : props.type === 'coverage' ? 'coverage' : 'active'
  return labelFor(group, props.value)
})

const tone = computed(() => {
  if (props.type === 'session') return { open: 'info', closed: 'success', cancelled: 'danger' }[props.value] || 'neutral'
  if (props.type === 'subscription') return { active: 'success', completed: 'neutral', stopped: 'warning', cancelled: 'danger' }[props.value] || 'neutral'
  if (props.type === 'debt') return { open: 'warning', partial: 'info', paid: 'success', void: 'neutral' }[props.value] || 'neutral'
  if (props.type === 'active') return props.value ? 'success' : 'neutral'
  return 'outline'
})
</script>

<style scoped>
.status-chip { font-weight: 600; border-radius: var(--radius-pill) !important; }
.status-info { color: var(--color-primary); background: var(--color-info-bg); }
.status-success { color: var(--color-success); background: var(--color-success-bg); }
.status-danger { color: var(--color-danger); background: var(--color-danger-bg); }
.status-warning { color: var(--color-warning); background: var(--color-warning-bg); }
.status-neutral { color: var(--color-ink-muted); background: var(--color-parchment); }
.status-outline { color: var(--color-ink); background: transparent; border: 1px solid var(--color-hairline); }
</style>
