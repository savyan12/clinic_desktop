<template>
  <v-app>
    <router-view />
    <v-snackbar
      v-model="ui.snackbar.visible"
      :color="ui.snackbar.color"
      location="bottom start"
      :timeout="ui.snackbar.timeout"
    >
      {{ ui.snackbar.message }}
      <template #actions>
        <v-btn variant="text" @click="ui.hideSnackbar">إغلاق</v-btn>
      </template>
    </v-snackbar>
  </v-app>
</template>

<script setup>
import { useFormFlow } from '@/composables/useFormFlow'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
useFormFlow()
</script>
