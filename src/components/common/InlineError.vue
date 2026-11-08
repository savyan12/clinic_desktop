<template>
  <div class="inline-error" role="alert">
    <v-icon icon="mdi-alert-circle-outline" size="22" />
    <div><strong>{{ title }}</strong><span>{{ message }}</span></div>
    <v-btn v-if="retry" size="small" variant="text" @click="$emit('retry')">إعادة المحاولة</v-btn>
  </div>
</template>

<script setup>
defineProps({ title: { type: String, default: 'تعذر إتمام الطلب' }, message: { type: String, required: true }, retry: { type: Boolean, default: false } })
defineEmits(['retry'])
</script>

<style scoped>
.inline-error {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  color: var(--color-danger);
  background: var(--color-danger-bg);
  border: 1px solid var(--color-danger-border);
  border-radius: var(--radius-md);
}

.inline-error div {
  min-width: 0;
  flex: 1;
}

.inline-error strong,
.inline-error span {
  display: block;
}

.inline-error strong {
  font-weight: 600;
}

.inline-error span {
  margin-top: 2px;
  font-size: 14px;
  line-height: 1.55;
}
</style>
