<template>
  <v-dialog :model-value="modelValue" persistent max-width="520" @update:model-value="$emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="dialog-title">{{ title }}</v-card-title>
      <v-card-text>
        <p class="dialog-message">{{ message }}</p>
        <v-textarea
          v-if="requireReason"
          v-model="reason"
          label="السبب"
          rows="3"
          counter="300"
          :error-messages="reasonError"
        />
      </v-card-text>
      <v-card-actions class="dialog-actions">
        <v-btn variant="text" :disabled="loading" @click="cancel">تراجع</v-btn>
        <v-btn :color="danger ? 'error' : 'primary'" :loading="loading" @click="confirm">{{ confirmLabel }}</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup>
import { ref, watch } from 'vue'

const props = defineProps({
  modelValue: Boolean,
  title: { type: String, required: true },
  message: { type: String, required: true },
  confirmLabel: { type: String, default: 'تأكيد' },
  requireReason: { type: Boolean, default: false },
  danger: { type: Boolean, default: false },
  loading: { type: Boolean, default: false },
})
const emit = defineEmits(['update:modelValue', 'confirm', 'cancel'])
const reason = ref('')
const reasonError = ref('')

watch(() => props.modelValue, (open) => { if (open) { reason.value = ''; reasonError.value = '' } })
function cancel() { emit('update:modelValue', false); emit('cancel') }
function confirm() {
  if (props.requireReason && !reason.value.trim()) { reasonError.value = 'يرجى إدخال السبب.'; return }
  emit('confirm', reason.value.trim())
}
</script>

<style scoped>
.dialog-title { padding: 24px 24px 4px; font-size: 22px; font-weight: 600; }
.dialog-message { margin: 0 0 20px; color: var(--color-ink-muted); line-height: 1.7; }
.dialog-actions { gap: 8px; justify-content: flex-end; padding: 8px 24px 24px; }
</style>

