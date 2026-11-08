<template>
  <div class="empty-state" :class="`empty-${size}`">
    <div class="empty-icon"><v-icon :icon="icon" :size="size === 'inline' ? 20 : 30" /></div>
    <h3>{{ title }}</h3>
    <p>{{ description }}</p>
    <v-btn v-if="actionLabel" color="primary" @click="$emit('action')">{{ actionLabel }}</v-btn>
  </div>
</template>

<script setup>
defineProps({
  icon: { type: String, default: 'mdi-database-off-outline' },
  title: { type: String, default: 'لا توجد بيانات' },
  description: { type: String, default: 'لم نجد سجلات مطابقة للبحث الحالي.' },
  actionLabel: { type: String, default: '' },
  size: { type: String, default: 'page' },
})
defineEmits(['action'])
</script>

<style scoped>
.empty-state {
  min-height: 220px;
  display: grid;
  place-items: center;
  align-content: center;
  gap: var(--sp-2);
  padding: var(--sp-8);
  text-align: center;
}

.empty-icon {
  width: 56px;
  height: 56px;
  display: grid;
  place-items: center;
  color: var(--accent);
  background: var(--accent-tint);
  border-radius: 16px;
}

.empty-state h3 {
  margin: var(--sp-2) 0 0;
  color: var(--color-foreground);
  font-size: 17px;
  font-weight: 600;
}

.empty-section {
  min-height: 150px;
  grid-template-columns: auto 1fr;
  justify-items: start;
  place-items: center start;
  gap: 4px 14px;
  padding: var(--sp-5);
  text-align: start;
}

.empty-section .empty-icon {
  width: 44px;
  height: 44px;
  grid-row: 1 / span 2;
}

.empty-section h3,
.empty-section p {
  margin: 0;
}

.empty-section :deep(.v-btn) {
  grid-column: 2;
}

.empty-inline {
  min-height: 56px;
  display: flex;
  justify-content: flex-start;
  gap: 10px;
  padding: var(--sp-3);
  text-align: start;
}

.empty-inline .empty-icon {
  width: 32px;
  height: 32px;
}

.empty-inline h3 {
  margin: 0;
  font-size: 14px;
}

.empty-inline p,
.empty-inline :deep(.v-btn) {
  display: none;
}

.empty-state p {
  max-width: 440px;
  margin: 0 0 var(--space-2);
  color: var(--color-muted-foreground);
  font-size: 14px;
  line-height: 1.6;
}
</style>
