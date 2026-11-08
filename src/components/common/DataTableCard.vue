<template>
  <section class="data-table-card" :aria-busy="loading" :aria-label="tableLabel">
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
      <div class="table-scroll" :class="{ 'table-refreshing': loading }" :inert="loading" tabindex="0">
        <v-table>
          <thead><tr><th v-for="header in headers" :key="header.key" scope="col" :class="header.class">{{ header.title }}</th></tr></thead>
          <tbody><slot name="rows" :items="items" /></tbody>
        </v-table>
      </div>
      <div v-if="$slots.cards" class="mobile-card-list"><slot name="cards" :items="items" /></div>
      <div v-if="meta && meta.last_page > 1" class="table-pagination">
        <span role="status">إجمالي {{ meta.total }} سجل</span>
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
  tableLabel: { type: String, default: 'جدول البيانات' },
})
defineEmits(['retry', 'page', 'empty-action'])
</script>

<style scoped>
.data-table-card { position: relative; min-width: 0; overflow: hidden; border: 1px solid var(--color-border); border-radius: var(--r-card); background: var(--color-surface); box-shadow: var(--shadow-card); }
.table-card-header { padding: var(--space-4) var(--space-5); border-bottom: 1px solid var(--color-border); background: var(--color-surface); }
.table-scroll { overflow-x: auto; outline-offset: -3px; }
.table-refreshing { opacity: 0.65; }
.table-scroll :deep(table) { min-width: 900px; }
.mobile-card-list { display: none; }
.loading-table { min-height: 380px; padding: 8px 16px; }
.error-state { min-height: 280px; display: grid; place-items: center; align-content: center; gap: 8px; padding: 32px; text-align: center; }
.error-state h3, .error-state p { margin: 0; }
.error-state p { color: var(--color-muted-foreground); }
.table-pagination { min-height: 64px; display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 10px 20px; border-top: 1px solid var(--color-border); color: var(--color-muted-foreground); font-size: 14px; }
@media (max-width: 767px) {
  .mobile-card-list { display: grid; gap: 10px; padding: 12px; }
  .mobile-card-list + .table-pagination { border-top: 1px solid var(--color-border); }
  .data-table-card:has(.mobile-card-list) .table-scroll { display: none; }
}
@media (max-width: 640px) { .table-pagination { flex-direction: column; } }
</style>
