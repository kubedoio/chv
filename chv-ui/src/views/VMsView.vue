<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useVMsStore } from '@/stores/vms'
import { useToast } from 'primevue/usetoast'
import { useConfirm } from 'primevue/useconfirm'

const vmsStore = useVMsStore()
const toast = useToast()
const confirm = useConfirm()

const showCreateModal = ref(false)
const activeTab = ref('summary')

const newVM = ref({
  name: '',
  cpu: 1,
  memory_mb: 1024,
  image_id: '',
  disk_size_gb: 10
})

onMounted(() => {
  vmsStore.fetchVMs()
})

function selectVM(vm: any) {
  vmsStore.selectVM(vm)
  activeTab.value = 'summary'
}

async function startVM() {
  if (!vmsStore.selectedVM) return
  try {
    await vmsStore.startVM(vmsStore.selectedVM.id)
    toast.add({ severity: 'success', summary: 'VM Started', detail: `${vmsStore.selectedVM.name} is starting`, life: 3000 })
  } catch (err) {
    toast.add({ severity: 'error', summary: 'Error', detail: 'Failed to start VM', life: 3000 })
  }
}

async function stopVM() {
  if (!vmsStore.selectedVM) return
  try {
    await vmsStore.stopVM(vmsStore.selectedVM.id)
    toast.add({ severity: 'success', summary: 'VM Stopped', detail: `${vmsStore.selectedVM.name} is stopping`, life: 3000 })
  } catch (err) {
    toast.add({ severity: 'error', summary: 'Error', detail: 'Failed to stop VM', life: 3000 })
  }
}

async function rebootVM() {
  if (!vmsStore.selectedVM) return
  try {
    await vmsStore.rebootVM(vmsStore.selectedVM.id)
    toast.add({ severity: 'success', summary: 'VM Rebooted', detail: `${vmsStore.selectedVM.name} is rebooting`, life: 3000 })
  } catch (err) {
    toast.add({ severity: 'error', summary: 'Error', detail: 'Failed to reboot VM', life: 3000 })
  }
}

function confirmDelete() {
  if (!vmsStore.selectedVM) return
  confirm.require({
    message: `Are you sure you want to delete ${vmsStore.selectedVM.name}?`,
    header: 'Delete VM',
    icon: 'pi pi-exclamation-triangle',
    acceptClass: 'p-button-danger',
    accept: async () => {
      try {
        await vmsStore.deleteVM(vmsStore.selectedVM!.id)
        toast.add({ severity: 'success', summary: 'VM Deleted', detail: 'VM has been deleted', life: 3000 })
      } catch (err) {
        toast.add({ severity: 'error', summary: 'Error', detail: 'Failed to delete VM', life: 3000 })
      }
    }
  })
}

function getStatusClass(state: string) {
  switch (state) {
    case 'running': return 'status-running'
    case 'stopped': return 'status-stopped'
    case 'error': return 'status-error'
    case 'starting':
    case 'stopping': return 'status-warning'
    default: return 'status-stopped'
  }
}

function formatState(state: string) {
  return state.charAt(0).toUpperCase() + state.slice(1)
}
</script>

<template>
  <div class="vms-page">
    <!-- VM List Sidebar -->
    <div class="vm-list-panel">
      <div class="panel-header">
        <h2>Virtual Machines</h2>
        <button class="create-btn" @click="showCreateModal = true">
          <i class="pi pi-plus"></i>
          Create
        </button>
      </div>
      
      <div class="vm-list">
        <div
          v-for="vm in vmsStore.vms"
          :key="vm.id"
          :class="['vm-item', { selected: vmsStore.selectedVM?.id === vm.id }]"
          @click="selectVM(vm)"
        >
          <div class="vm-status">
            <span :class="['status-badge', getStatusClass(vm.actual_state)]">
              {{ formatState(vm.actual_state) }}
            </span>
          </div>
          <div class="vm-info">
            <div class="vm-name">{{ vm.name }}</div>
            <div class="vm-meta mono">{{ vm.id.substring(0, 8) }}</div>
          </div>
        </div>
        
        <div v-if="vmsStore.vms.length === 0" class="empty-state">
          <i class="pi pi-server"></i>
          <p>No VMs found</p>
          <button @click="showCreateModal = true">Create your first VM</button>
        </div>
      </div>
    </div>

    <!-- VM Details Panel -->
    <div class="vm-details-panel">
      <div v-if="vmsStore.selectedVM" class="details-content">
        <div class="details-header">
          <h1>{{ vmsStore.selectedVM.name }}</h1>
          <div class="header-actions">
            <button
              v-if="vmsStore.selectedVM.actual_state === 'stopped'"
              class="action-btn primary"
              @click="startVM"
            >
              <i class="pi pi-play"></i>
              Start
            </button>
            <button
              v-if="vmsStore.selectedVM.actual_state === 'running'"
              class="action-btn"
              @click="stopVM"
            >
              <i class="pi pi-stop"></i>
              Stop
            </button>
            <button
              v-if="vmsStore.selectedVM.actual_state === 'running'"
              class="action-btn"
              @click="rebootVM"
            >
              <i class="pi pi-refresh"></i>
              Reboot
            </button>
            <button class="action-btn danger" @click="confirmDelete">
              <i class="pi pi-trash"></i>
              Delete
            </button>
          </div>
        </div>

        <div class="tabs">
          <button
            :class="['tab', { active: activeTab === 'summary' }]"
            @click="activeTab = 'summary'"
          >
            Summary
          </button>
          <button
            :class="['tab', { active: activeTab === 'console' }]"
            @click="activeTab = 'console'"
          >
            Console
          </button>
          <button
            :class="['tab', { active: activeTab === 'settings' }]"
            @click="activeTab = 'settings'"
          >
            Settings
          </button>
          <button
            :class="['tab', { active: activeTab === 'logs' }]"
            @click="activeTab = 'logs'"
          >
            Logs
          </button>
        </div>

        <div class="tab-content">
          <div v-if="activeTab === 'summary'" class="summary-tab">
            <div class="info-grid">
              <div class="info-item">
                <label>ID</label>
                <span class="mono">{{ vmsStore.selectedVM.id }}</span>
              </div>
              <div class="info-item">
                <label>Status</label>
                <span :class="['status-badge', getStatusClass(vmsStore.selectedVM.actual_state)]">
                  {{ formatState(vmsStore.selectedVM.actual_state) }}
                </span>
              </div>
              <div class="info-item">
                <label>Node</label>
                <span>{{ vmsStore.selectedVM.node_id || 'Not assigned' }}</span>
              </div>
              <div class="info-item">
                <label>Created</label>
                <span>{{ new Date(vmsStore.selectedVM.created_at).toLocaleString() }}</span>
              </div>
              <div class="info-item">
                <label>vCPUs</label>
                <span>{{ vmsStore.selectedVM.spec?.cpu || '-' }}</span>
              </div>
              <div class="info-item">
                <label>Memory</label>
                <span>{{ vmsStore.selectedVM.spec?.memory_mb ? `${vmsStore.selectedVM.spec.memory_mb} MB` : '-' }}</span>
              </div>
            </div>
          </div>

          <div v-else-if="activeTab === 'console'" class="console-tab">
            <div class="console-placeholder">
              <i class="pi pi-desktop"></i>
              <p>VM Console</p>
              <span>Console access coming in v0.2.0</span>
            </div>
          </div>

          <div v-else-if="activeTab === 'settings'" class="settings-tab">
            <p>VM settings will be available in a future release.</p>
          </div>

          <div v-else-if="activeTab === 'logs'" class="logs-tab">
            <div class="logs-placeholder">
              <p>No logs available</p>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="no-selection">
        <i class="pi pi-server"></i>
        <p>Select a VM to view details</p>
      </div>
    </div>

    <!-- Create VM Modal -->
    <Dialog v-model:visible="showCreateModal" header="Create Virtual Machine" modal style="width: 500px">
      <div class="create-form">
        <div class="form-group">
          <label>Name</label>
          <input v-model="newVM.name" type="text" placeholder="vm-name" />
        </div>
        
        <div class="form-row">
          <div class="form-group">
            <label>vCPUs</label>
            <input v-model.number="newVM.cpu" type="number" min="1" max="32" />
          </div>
          <div class="form-group">
            <label>Memory (MB)</label>
            <input v-model.number="newVM.memory_mb" type="number" min="512" step="512" />
          </div>
        </div>
        
        <div class="form-group">
          <label>Disk Size (GB)</label>
          <input v-model.number="newVM.disk_size_gb" type="number" min="10" />
        </div>
        
        <div class="form-group">
          <label>Image ID</label>
          <input v-model="newVM.image_id" type="text" placeholder="Image UUID" />
        </div>
      </div>
      
      <template #footer>
        <Button label="Cancel" text @click="showCreateModal = false" />
        <Button label="Create" @click="showCreateModal = false" />
      </template>
    </Dialog>
  </div>
</template>

<style scoped>
.vms-page {
  display: flex;
  height: 100%;
  gap: 16px;
}

.vm-list-panel {
  width: 320px;
  background: white;
  border: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
}

.panel-header h2 {
  font-size: 14px;
  font-weight: 600;
}

.create-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background-color: var(--color-primary);
  color: white;
  border: none;
  border-radius: 2px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
}

.create-btn:hover {
  background-color: #0052a3;
}

.vm-list {
  flex: 1;
  overflow-y: auto;
}

.vm-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
  cursor: pointer;
  transition: background-color 0.15s;
}

.vm-item:hover {
  background-color: var(--color-hover);
}

.vm-item.selected {
  background-color: var(--color-selected);
}

.vm-status {
  flex-shrink: 0;
}

.vm-info {
  flex: 1;
  min-width: 0;
}

.vm-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.vm-meta {
  font-size: 11px;
  color: var(--color-text-secondary);
  margin-top: 2px;
}

.empty-state {
  padding: 32px 16px;
  text-align: center;
}

.empty-state i {
  font-size: 32px;
  color: var(--color-border);
  margin-bottom: 12px;
}

.empty-state p {
  font-size: 13px;
  color: var(--color-text-secondary);
  margin-bottom: 12px;
}

.empty-state button {
  padding: 8px 16px;
  background-color: var(--color-primary);
  color: white;
  border: none;
  border-radius: 2px;
  font-size: 12px;
  cursor: pointer;
}

.vm-details-panel {
  flex: 1;
  background: white;
  border: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.details-content {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.details-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px;
  border-bottom: 1px solid var(--color-border);
}

.details-header h1 {
  font-size: 18px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.header-actions {
  display: flex;
  gap: 8px;
}

.action-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  background-color: white;
  border: 1px solid var(--color-border);
  border-radius: 2px;
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-primary);
  cursor: pointer;
}

.action-btn:hover {
  background-color: var(--color-hover);
}

.action-btn.primary {
  background-color: var(--color-primary);
  color: white;
  border-color: var(--color-primary);
}

.action-btn.primary:hover {
  background-color: #0052a3;
}

.action-btn.danger {
  color: var(--color-error);
  border-color: var(--color-error);
}

.action-btn.danger:hover {
  background-color: rgba(230, 0, 0, 0.1);
}

.tabs {
  display: flex;
  gap: 2px;
  padding: 0 16px;
  border-bottom: 1px solid var(--color-border);
  background-color: #fafafa;
}

.tab {
  padding: 12px 16px;
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-secondary);
  cursor: pointer;
  margin-bottom: -1px;
}

.tab:hover {
  color: var(--color-text-primary);
}

.tab.active {
  color: var(--color-primary);
  border-bottom-color: var(--color-primary);
  background-color: white;
}

.tab-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.summary-tab .info-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 16px;
}

.info-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.info-item label {
  font-size: 11px;
  font-weight: 600;
  color: var(--color-text-secondary);
  text-transform: uppercase;
}

.info-item span {
  font-size: 13px;
  color: var(--color-text-primary);
}

.console-tab,
.logs-tab {
  height: 100%;
}

.console-placeholder,
.logs-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--color-text-secondary);
}

.console-placeholder i {
  font-size: 48px;
  margin-bottom: 16px;
}

.console-placeholder p {
  font-size: 16px;
  font-weight: 500;
  margin-bottom: 8px;
}

.console-placeholder span {
  font-size: 12px;
}

.no-selection {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--color-text-secondary);
}

.no-selection i {
  font-size: 48px;
  margin-bottom: 16px;
}

.no-selection p {
  font-size: 14px;
}

.create-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-group label {
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-primary);
}

.form-group input {
  padding: 8px 12px;
  border: 1px solid var(--color-border);
  border-radius: 2px;
  font-size: 13px;
}

.form-group input:focus {
  outline: none;
  border-color: var(--color-primary);
}
</style>
