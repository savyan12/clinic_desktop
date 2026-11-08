<template>
  <div class="page-header">
    <div class="page-heading-copy">
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
.page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-6);
}

.page-heading-copy {
  min-width: 0;
  max-width: 780px;
}

.page-header h1 {
  margin: 0;
  color: var(--color-foreground);
  font-size: var(--fs-page);
  line-height: 1.4;
  font-weight: var(--fw-page);
}

.page-header p {
  margin: var(--sp-2) 0 0;
  color: var(--color-muted-foreground);
  font-size: 14px;
  line-height: 1.6;
}

.page-actions {
  min-height: var(--control-h);
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2);
  justify-content: flex-end;
}

@media (max-width: 1120px) {
  .page-header {
    flex-direction: column;
    gap: var(--sp-4);
  }

  .page-actions {
    width: 100%;
    justify-content: flex-start;
  }
}

@media (max-width: 520px) {
  .page-actions :deep(.v-btn) {
    flex: 1 1 150px;
  }
}
</style>
