<template>
  <span class="money-text ltr-value" :class="{ 'money-negative': amount < 0, 'money-classic': classic }" :aria-label="accessibleLabel">
    <template v-if="classic">{{ formatMoney(value) }}</template>
    <template v-else>
      <span class="money-major">{{ major }}</span><span v-if="fraction" class="money-fraction">.{{ fraction }}</span>
      <span class="money-currency">د.ل</span>
    </template>
  </span>
</template>

<script setup>
import { computed } from 'vue'
import { formatMoney, numeric } from '@/composables/useMoney'

const props = defineProps({
  value: { type: [String, Number], default: '0.000' },
  classic: { type: Boolean, default: false },
})
const amount = computed(() => numeric(props.value))
const parts = computed(() => Math.abs(amount.value).toFixed(3).split('.'))
const major = computed(() => `${amount.value < 0 ? '−' : ''}${new Intl.NumberFormat('en-US-u-nu-latn', { maximumFractionDigits: 0 }).format(Number(parts.value[0]))}`)
const fraction = computed(() => parts.value[1].replace(/0+$/, ''))
const accessibleLabel = computed(() => `${amount.value} دينار ليبي`)
</script>

<style scoped>
.money-text {
  display: inline-flex !important;
  align-items: baseline;
  gap: 2px;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.money-major { font: inherit; }
.money-fraction { color: var(--text-muted); font-size: .68em; font-weight: 500; }
.money-currency { margin-inline-start: 3px; color: var(--text-muted); font-size: .55em; font-weight: 500; }
.money-classic { display: inline-block !important; }
.money-negative,
.money-negative .money-fraction,
.money-negative .money-currency { color: var(--danger-fg); }
</style>
