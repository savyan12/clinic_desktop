<template>
  <div class="page-header">
    <div>
      <h1>{{ title }}</h1>
      <p v-if="subtitle || $slots.subtitle"><slot name="subtitle">{{ subtitle }}</slot></p>
    </div>
    <div class="page-actions">
      <slot name="actions">
        <v-btn v-if="actionLabel" color="primary" :prepend-icon="actionIcon" @click="$emit('action')">
          {{ actionLabel }}
        </v-btn>
      </slot>
    </div>
  </div>
</template>

<script setup>
defineProps({
  title: { type: String, required: true },
  subtitle: { type: String, default: '' },
  actionLabel: { type: String, default: '' },
  actionIcon: { type: String, default: 'mdi-plus' },
})
defineEmits(['action'])
</script>

<style scoped>
.page-header { display: flex; align-items: flex-start; justify-content: space-between; gap: 24px; }
.page-header h1 { margin: 0; color: var(--color-ink); font-size: var(--font-heading-size); line-height: 1.25; font-weight: var(--font-heading-weight); }
.page-header p { margin: 8px 0 0; color: var(--color-ink-muted); font-size: 15px; }
.page-actions { display: flex; flex-wrap: wrap; gap: 10px; justify-content: flex-end; }
@media (max-width: 640px) { .page-header { flex-direction: column; } .page-actions, .page-actions :deep(.v-btn) { width: 100%; } }
</style>
