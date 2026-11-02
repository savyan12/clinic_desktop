<template>
  <span class="money-text ltr-value" :class="{ 'money-negative': numeric(value) < 0 }">{{ formatMoney(value) }}</span>
</template>

<script setup>
import { formatMoney, numeric } from '@/composables/useMoney'

defineProps({ value: { type: [String, Number], default: '0.000' } })
</script>

<style scoped>
.money-text { white-space: nowrap; font-variant-numeric: tabular-nums; }
.money-negative { color: var(--color-danger); }
</style>
