<script setup lang="ts">
import ProgressSpinner from 'primevue/progressspinner'

interface Props {
  loading: boolean
  message?: string
}

defineProps<Props>()
</script>

<template>
  <div v-if="loading" class="loading-overlay">
    <ProgressSpinner />
    <span v-if="message" class="message">{{ message }}</span>
  </div>
</template>

<style scoped>
.loading-overlay {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  gap: 16px;
}

.message {
  font-size: 14px;
  color: var(--color-text-secondary);
}
</style>
