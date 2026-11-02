<template>
  <div class="empty-state">
    <div class="empty-icon"><v-icon :icon="icon" size="32" /></div>
    <h3>{{ title }}</h3>
    <p>{{ description }}</p>
    <v-btn v-if="actionLabel" variant="outlined" color="primary" @click="$emit('action')">{{ actionLabel }}</v-btn>
  </div>
</template>

<script setup>
defineProps({
  icon: { type: String, default: 'mdi-database-off-outline' },
  title: { type: String, default: 'لا توجد بيانات' },
  description: { type: String, default: 'لم نجد سجلات مطابقة للبحث الحالي.' },
  actionLabel: { type: String, default: '' },
})
defineEmits(['action'])
</script>

<style scoped>
.empty-state { min-height: 260px; display: grid; place-items: center; align-content: center; gap: 8px; padding: 32px; text-align: center; }
.empty-icon { width: 64px; height: 64px; display: grid; place-items: center; border-radius: 18px; color: var(--color-primary); background: var(--color-info-bg); }
.empty-state h3 { margin: 8px 0 0; font-size: 18px; font-weight: 600; }
.empty-state p { max-width: 420px; margin: 0 0 8px; color: var(--color-ink-muted); }
</style>

