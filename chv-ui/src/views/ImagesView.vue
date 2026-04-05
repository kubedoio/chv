<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { imagesApi } from '@/api/images'
import type { Image } from '@/types'

const images = ref<Image[]>([])
const loading = ref(false)

onMounted(async () => {
  loading.value = true
  try {
    images.value = await imagesApi.listImages()
  } finally {
    loading.value = false
  }
})

function getStatusIcon(status: string): string {
  switch (status) {
    case 'ready': return 'pi-check-circle'
    case 'importing': return 'pi-spin pi-spinner'
    case 'failed': return 'pi-exclamation-circle'
    default: return 'pi-question-circle'
  }
}

function formatBytes(bytes: number): string {
  const gb = bytes / (1024 * 1024 * 1024)
  return `${gb.toFixed(2)} GB`
}
</script>

<template>
  <div class="images-page">
    <div class="page-header">
      <h1>Images</h1>
      <button class="create-btn">
        <i class="pi pi-plus"></i>
        Import Image
      </button>
    </div>

    <div class="table-container">
      <table class="data-table">
        <thead>
          <tr>
            <th>Status</th>
            <th>Name</th>
            <th>OS Family</th>
            <th>Architecture</th>
            <th>Size</th>
            <th>Created</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="image in images" :key="image.id">
            <td>
              <i :class="['pi', getStatusIcon(image.status), image.status]"></i>
            </td>
            <td class="name-cell">{{ image.name }}</td>
            <td>{{ image.os_family }}</td>
            <td class="mono">{{ image.architecture }}</td>
            <td>{{ formatBytes(image.size_bytes) }}</td>
            <td>{{ new Date(image.created_at).toLocaleDateString() }}</td>
          </tr>
          <tr v-if="images.length === 0">
            <td colspan="6" class="empty-cell">No images found</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.images-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.page-header h1 {
  font-size: 18px;
  font-weight: 600;
}

.create-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  background-color: var(--color-primary);
  color: white;
  border: none;
  border-radius: 2px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
}

.table-container {
  background: white;
  border: 1px solid var(--color-border);
}

.name-cell {
  font-weight: 500;
}

.pi.ready {
  color: var(--color-success);
}

.pi.importing {
  color: var(--color-warning);
}

.pi.failed {
  color: var(--color-error);
}

.empty-cell {
  text-align: center;
  padding: 32px;
  color: var(--color-text-secondary);
}
</style>
