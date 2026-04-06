<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useStorageStore } from '@/stores/storage'
import { useAppToast } from '@/utils/toast'
import CreateStorageModal from '@/components/modals/CreateStorageModal.vue'

const storageStore = useStorageStore()
const toast = useAppToast()

const showCreateModal = ref(false)

onMounted(() => {
  storageStore.fetchStoragePools()
})

function onStorageCreated() {
  showCreateModal.value = false
  toast.success('Storage pool created successfully')
}

function formatBytes(bytes: number): string {
  const gb = bytes / (1024 * 1024 * 1024)
  return `${gb.toFixed(1)} GB`
}

function getUsagePercent(pool: any): number {
  if (pool.total_bytes === 0) return 0
  return Math.round((pool.used_bytes / pool.total_bytes) * 100)
}
</script>

<template>
  <div class="storage-page">
    <div class="page-header">
      <h1>Storage Pools</h1>
      <button class="create-btn" @click="showCreateModal = true">
        <i class="pi pi-plus"></i>
        Add Storage
      </button>
    </div>

    <div class="pools-grid">
      <div v-for="pool in storageStore.pools" :key="pool.id" class="pool-card">
        <div class="pool-header">
          <div class="pool-icon">
            <i class="pi pi-database"></i>
          </div>
          <div class="pool-info">
            <h3>{{ pool.name }}</h3>
            <span :class="['pool-type', pool.pool_type]">{{ pool.pool_type }}</span>
          </div>
        </div>
        
        <div class="pool-usage">
          <div class="usage-bar">
            <div 
              class="usage-fill" 
              :style="{ width: getUsagePercent(pool) + '%' }"
              :class="{ warning: getUsagePercent(pool) > 80 }"
            ></div>
          </div>
          <div class="usage-stats">
            <span>{{ formatBytes(pool.used_bytes) }} used</span>
            <span>{{ formatBytes(pool.total_bytes) }} total</span>
          </div>
        </div>
        
        <div class="pool-path">
          <i class="pi pi-folder"></i>
          <span class="mono">{{ pool.path_or_export }}</span>
        </div>
      </div>
      
      <div v-if="storageStore.pools.length === 0" class="empty-state">
        <i class="pi pi-database"></i>
        <p>No storage pools configured</p>
      </div>
    </div>

    <CreateStorageModal
      v-model:visible="showCreateModal"
      @created="onStorageCreated"
      @cancel="showCreateModal = false"
    />
  </div>
</template>

<style scoped>
.storage-page {
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

.pools-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
  gap: 16px;
}

.pool-card {
  background: white;
  border: 1px solid var(--color-border);
  padding: 16px;
}

.pool-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.pool-icon {
  width: 40px;
  height: 40px;
  background-color: rgba(102, 102, 102, 0.1);
  color: var(--color-text-secondary);
  border-radius: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
}

.pool-info {
  flex: 1;
}

.pool-info h3 {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.pool-type {
  font-size: 11px;
  text-transform: uppercase;
  padding: 2px 6px;
  border-radius: 2px;
  background-color: var(--color-bg-chrome);
  color: var(--color-text-secondary);
}

.pool-usage {
  margin-bottom: 12px;
}

.usage-bar {
  height: 8px;
  background-color: var(--color-bg-chrome);
  border-radius: 4px;
  overflow: hidden;
  margin-bottom: 8px;
}

.usage-fill {
  height: 100%;
  background-color: var(--color-success);
  transition: width 0.3s ease;
}

.usage-fill.warning {
  background-color: var(--color-warning);
}

.usage-stats {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--color-text-secondary);
}

.pool-path {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background-color: var(--color-bg-chrome);
  font-size: 12px;
  color: var(--color-text-secondary);
}

.pool-path i {
  font-size: 14px;
}

.empty-state {
  grid-column: span 2;
  padding: 64px;
  text-align: center;
  background: white;
  border: 1px solid var(--color-border);
}

.empty-state i {
  font-size: 48px;
  color: var(--color-border);
  margin-bottom: 16px;
}

.empty-state p {
  color: var(--color-text-secondary);
}
</style>
