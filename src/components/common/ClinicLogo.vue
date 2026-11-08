<template>
  <img
    class="clinic-logo"
    src="/clinic-logo-128.png"
    srcset="/clinic-logo-128.png 1x, /clinic-logo-256.png 2x, /clinic-logo-384.png 3x"
    :alt="alt"
    :width="numericSize"
    :height="numericSize"
    :style="logoStyle"
    decoding="sync"
    draggable="false"
  >
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  alt: { type: String, default: 'شعار المركز' },
  size: { type: [Number, String], default: 52 },
})

const logoStyle = computed(() => {
  const size = typeof props.size === 'number' ? `${props.size}px` : props.size
  return { width: size, height: size }
})

const numericSize = computed(() => (typeof props.size === 'number' ? props.size : undefined))
</script>

<style scoped>
.clinic-logo {
  display: block;
  flex: 0 0 auto;
  object-fit: contain;
  background: transparent;
  image-rendering: auto;
  user-select: none;
}
</style>
