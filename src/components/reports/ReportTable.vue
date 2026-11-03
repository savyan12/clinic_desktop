<template>
  <div class="report-table-scroll" :class="{ 'report-table-scroll--print': print }">
    <table class="report-data-table" :class="{ 'report-data-table--print': print }">
      <colgroup>
        <col v-for="column in definition.columns" :key="column.key" :style="{ width: `${column.width}%` }">
      </colgroup>
      <thead>
        <tr>
          <th v-for="column in definition.columns" :key="column.key" scope="col">{{ column.label }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-if="!rows.length">
          <td :colspan="definition.columns.length" class="report-empty-row">لا توجد بيانات ضمن الفترة المحددة</td>
        </tr>
        <tr v-for="(row, rowIndex) in rows" v-else :key="row.id ?? rowIndex">
          <td v-for="column in definition.columns" :key="column.key">
            <MoneyText v-if="column.kind === 'money'" :value="reportCellRawValue(column, row)" />
            <StatusChip
              v-else-if="column.kind === 'status'"
              :type="column.statusType"
              :value="reportCellRawValue(column, row)"
            />
            <bdi v-else :dir="cellDirection(column)">{{ reportCellLabel(column, row) }}</bdi>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import MoneyText from '@/components/common/MoneyText.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import { getReportDefinition, reportCellLabel, reportCellRawValue } from '@/constants/reports'

const props = defineProps({
  type: { type: String, required: true },
  rows: { type: Array, default: () => [] },
  print: { type: Boolean, default: false },
})

const definition = computed(() => getReportDefinition(props.type))
const cellDirection = (column) => ['number', 'reference'].includes(column.kind) ? 'ltr' : 'auto'
</script>

<style scoped>
.report-table-scroll {
  width: 100%;
  overflow-x: auto;
}

.report-data-table {
  width: 100%;
  min-width: 860px;
  border-collapse: collapse;
  table-layout: fixed;
  background: var(--color-canvas);
}

.report-data-table th,
.report-data-table td {
  padding: 12px 14px;
  border-bottom: 1px solid var(--color-hairline);
  text-align: right;
  vertical-align: middle;
  overflow-wrap: anywhere;
}

.report-data-table th {
  background: var(--color-parchment);
  color: var(--color-ink-muted);
  font-size: 14px;
  font-weight: 600;
  white-space: nowrap;
}

.report-data-table td {
  min-height: 60px;
  font-size: var(--font-table-size);
  font-weight: var(--font-table-weight);
}

.report-data-table tbody tr:last-child td {
  border-bottom: 0;
}

.report-empty-row {
  padding: 28px !important;
  color: var(--color-print-muted);
  text-align: center !important;
}

.report-table-scroll--print {
  overflow: visible;
  border: 1px solid var(--color-print-border);
}

.report-data-table--print {
  min-width: 0;
  font-size: 9px;
}

.report-data-table--print th,
.report-data-table--print td {
  padding: 7px 5px;
  border-bottom-color: var(--color-print-border);
  border-inline-start: 1px solid var(--color-print-border);
  font-size: 9px;
  line-height: 1.45;
  text-align: center;
}

.report-data-table--print th:first-child,
.report-data-table--print td:first-child {
  border-inline-start: 0;
}

.report-data-table--print th {
  background: var(--color-print-brand);
  color: var(--color-canvas);
  font-weight: 600;
  white-space: normal;
}

.report-data-table--print td {
  height: auto;
  color: var(--color-ink);
}

.report-data-table--print :deep(.money-text) {
  color: inherit;
  font-size: inherit;
}

.report-data-table--print :deep(.v-chip) {
  height: auto;
  min-height: 20px;
  padding: 2px 6px;
  font-size: 8px;
}

@media print {
  .report-data-table--print thead {
    display: table-header-group;
  }

  .report-data-table--print tr {
    break-inside: avoid;
  }
}
</style>
