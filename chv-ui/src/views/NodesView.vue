<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useNodesStore } from '@/stores/nodes'
import { useAppToast } from '@/utils/toast'
import RegisterNodeModal from '@/components/modals/RegisterNodeModal.vue'

const nodesStore = useNodesStore()
const toast = useAppToast()

const showRegisterModal = ref(false)

onMounted(() => {
  nodesStore.fetchNodes()
})

function onNodeRegistered() {
  showRegisterModal.value = false
  toast.success('Node registered successfully')
}

async function setMaintenance(id: string, enabled: boolean) {
  try {
    await nodesStore.setMaintenance(id, enabled)
    toast.success(enabled ? 'Node set to maintenance mode' : 'Node brought online')
  } catch (err: any) {
    toast.error(err.response?.data?.error?.message || 'Failed to update node status')
  }
}

function getStatusClass(state: string) {
  if (!state) return 'status-stopped'
  switch (state) {
    case 'online': return 'status-running'
    case 'offline': return 'status-error'
    case 'maintenance': return 'status-warning'
    default: return 'status-stopped'
  }
}

function formatState(state: string) {
  if (!state) return 'Unknown'
  return state.charAt(0).toUpperCase() + state.slice(1)
}
</script>

<template>
  <div class="nodes-page">
    <div class="page-header">
      <h1>Nodes</h1>
      <button class="create-btn" @click="showRegisterModal = true">
        <i class="pi pi-plus"></i>
        Register Node
      </button>
    </div>

    <div class="nodes-grid">
      <div v-for="node in nodesStore.nodes" :key="node.id" class="node-card">
        <div class="node-header">
          <div class="node-icon">
            <i class="pi pi-sitemap"></i>
          </div>
          <div class="node-title">
            <h3>{{ node.hostname }}</h3>
            <span class="mono">{{ node.management_ip }}</span>
          </div>
          <span :class="['status-badge', getStatusClass(node.status)]">
            {{ formatState(node.status) }}
          </span>
        </div>
        
        <div class="node-resources">
          <div class="resource">
            <i class="pi pi-microchip"></i>
            <span>{{ node.allocatable_cpu_cores }} / {{ node.total_cpu_cores }} vCPUs</span>
          </div>
          <div class="resource">
            <i class="pi pi-memory"></i>
            <span>{{ Math.round(node.allocatable_ram_mb / 1024) }} / {{ Math.round(node.total_ram_mb / 1024) }} GB RAM</span>
          </div>
        </div>
        
        <div class="node-footer">
          <span class="last-seen">Last seen: {{ node.last_heartbeat_at ? new Date(node.last_heartbeat_at).toLocaleString() : 'Never' }}</span>
          <button v-if="node.status === 'online'" class="action-link" @click="setMaintenance(node.id, true)">
            Maintenance
          </button>
        </div>
      </div>
      
      <div v-if="nodesStore.nodes.length === 0" class="empty-state">
        <i class="pi pi-sitemap"></i>
        <p>No nodes registered</p>
      </div>
    </div>

    <RegisterNodeModal
      v-model:visible="showRegisterModal"
      @created="onNodeRegistered"
      @cancel="showRegisterModal = false"
    />
  </div>
</template>

<style scoped>
.nodes-page {
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

.nodes-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(400px, 1fr));
  gap: 16px;
}

.node-card {
  background: white;
  border: 1px solid var(--color-border);
  padding: 16px;
}

.node-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.node-icon {
  width: 40px;
  height: 40px;
  background-color: rgba(0, 102, 204, 0.1);
  color: var(--color-primary);
  border-radius: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
}

.node-title {
  flex: 1;
}

.node-title h3 {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.node-title span {
  font-size: 12px;
  color: var(--color-text-secondary);
}

.node-resources {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  background-color: var(--color-bg-chrome);
  margin-bottom: 12px;
}

.resource {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--color-text-primary);
}

.resource i {
  color: var(--color-text-secondary);
  font-size: 14px;
}

.node-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11px;
}

.last-seen {
  color: var(--color-text-secondary);
}

.action-link {
  color: var(--color-primary);
  background: none;
  border: none;
  cursor: pointer;
  font-size: 12px;
}

.action-link:hover {
  text-decoration: underline;
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
