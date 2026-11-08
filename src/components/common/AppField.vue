<template>
  <div class="app-field">
    <label :for="fieldId">{{ label }}<span v-if="required" aria-hidden="true"> *</span></label>
    <slot :id="fieldId" />
    <p v-if="helper" :id="`${fieldId}-helper`" class="field-helper">{{ helper }}</p>
  </div>
</template>

<script setup>
import { useId } from 'vue'

defineProps({
  label: { type: String, required: true },
  helper: { type: String, default: '' },
  required: { type: Boolean, default: false },
})
const fieldId = `app-field-${useId().replaceAll(':', '')}`
</script>

<style scoped>
.app-field { min-width: 0; display: grid; align-content: start; gap: 7px; }
.app-field > label { color: var(--text); font-size: 13px; font-weight: 500; }
.app-field > label span { color: var(--danger-fg); }
.field-helper { margin: -2px 0 0; color: var(--text-muted); font-size: 12px; line-height: 1.5; }
</style>
