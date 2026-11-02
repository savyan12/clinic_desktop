<template>
  <section class="data-table-card" :aria-busy="loading">
    <div v-if="$slots.header" class="table-card-header"><slot name="header" /></div>
    <v-progress-linear v-if="loading && items.length" indeterminate color="primary" absolute location="top" aria-label="جارٍ تحديث النتائج" />
    <div v-if="loading && !items.length" class="loading-table">
      <v-skeleton-loader type="table-heading, table-row-divider@6" />
    </div>
    <div v-else-if="error" class="error-state">
      <v-icon icon="mdi-alert-circle-outline" size="36" color="error" />
      <h3>تعذر تحميل البيانات</h3>
      <p>{{ error }}</p>
      <v-btn color="primary" variant="outlined" prepend-icon="mdi-refresh" @click="$emit('retry')">إعادة المحاولة</v-btn>
    </div>
    <EmptyState v-else-if="!items.length" v-bind="empty" @action="$emit('empty-action')" />
    <template v-else>
      <div class="table-scroll" :class="{ 'table-refreshing': loading }" :inert="loading">
        <v-table>
          <thead><tr><th v-for="header in headers" :key="header.key" :class="header.class">{{ header.title }}</th></tr></thead>
          <tbody><slot name="rows" :items="items" /></tbody>
        </v-table>
      </div>
      <div v-if="meta && meta.last_page > 1" class="table-pagination">
        <span>إجمالي {{ meta.total }} سجل</span>
        <v-pagination :model-value="meta.current_page" :length="meta.last_page" :total-visible="5" :disabled="loading" density="comfortable" @update:model-value="$emit('page', $event)" />
      </div>
    </template>
  </section>
</template>

<script setup>
import EmptyState from './EmptyState.vue'

defineProps({
  headers: { type: Array, default: () => [] },
  items: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
  error: { type: String, default: '' },
  meta: { type: Object, default: null },
  empty: { type: Object, default: () => ({}) },
})
defineEmits(['retry', 'page', 'empty-action'])
</script>

<style scoped>
.data-table-card { position: relative; overflow: hidden; border: 1px solid var(--color-hairline); border-radius: var(--radius-card); background: var(--color-canvas); }
.table-card-header { padding: 20px 24px; border-bottom: 1px solid var(--color-hairline); }
.table-scroll { overflow-x: auto; }
.table-refreshing { opacity: 0.65; }
.table-scroll :deep(table) { min-width: 900px; }
.loading-table { min-height: 380px; padding: 8px 16px; }
.error-state { min-height: 280px; display: grid; place-items: center; align-content: center; gap: 8px; padding: 32px; text-align: center; }
.error-state h3, .error-state p { margin: 0; }
.error-state p { color: var(--color-ink-muted); }
.table-pagination { min-height: 68px; display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 20px; border-top: 1px solid var(--color-hairline); color: var(--color-ink-muted); font-size: 14px; }
@media (max-width: 640px) { .table-pagination { flex-direction: column; } }
</style>
