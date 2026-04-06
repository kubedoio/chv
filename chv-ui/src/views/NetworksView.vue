<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useNetworksStore } from '@/stores/networks'
import { useAppToast } from '@/utils/toast'
import CreateNetworkModal from '@/components/modals/CreateNetworkModal.vue'

const networksStore = useNetworksStore()
const toast = useAppToast()

const showCreateModal = ref(false)

onMounted(() => {
  networksStore.fetchNetworks()
})

function onNetworkCreated() {
  showCreateModal.value = false
  toast.success('Network created successfully')
}

function onCreateError(message: string) {
  toast.error(message || 'Failed to create network')
}
</script>

<template>
  <div class="networks-page">
    <div class="page-header">
      <h1>Networks</h1>
      <button class="create-btn" @click="showCreateModal = true">
        <i class="pi pi-plus"></i>
        Create Network
      </button>
    </div>

    <div class="table-container">
      <table class="data-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Bridge</th>
            <th>CIDR</th>
            <th>Gateway</th>
            <th>Created</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="network in networksStore.networks" :key="network.id">
            <td class="name-cell">
              <i class="pi pi-globe"></i>
              {{ network.name }}
            </td>
            <td class="mono">{{ network.bridge_name }}</td>
            <td class="mono">{{ network.cidr }}</td>
            <td class="mono">{{ network.gateway_ip }}</td>
            <td>{{ new Date(network.created_at).toLocaleDateString() }}</td>
          </tr>
          <tr v-if="networksStore.networks.length === 0">
            <td colspan="5" class="empty-cell">No networks found</td>
          </tr>
        </tbody>
      </table>
    </div>

    <CreateNetworkModal
      v-model:visible="showCreateModal"
      @created="onNetworkCreated"
      @cancel="showCreateModal = false"
    />
  </div>
</template>

<style scoped>
.networks-page {
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
  display: flex;
  align-items: center;
  gap: 8px;
}

.name-cell i {
  color: var(--color-primary);
}

.empty-cell {
  text-align: center;
  padding: 32px;
  color: var(--color-text-secondary);
}
</style>
