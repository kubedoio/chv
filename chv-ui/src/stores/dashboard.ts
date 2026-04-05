import { ref, computed } from 'vue'
import { defineStore } from 'pinia'
import { useVMsStore } from './vms'
import { useNodesStore } from './nodes'
import { networksApi } from '@/api/networks'
import { storageApi } from '@/api/storage'

export const useDashboardStore = defineStore('dashboard', () => {
  // State
  const stats = ref({
    total_vms: 0,
    running_vms: 0,
    stopped_vms: 0,
    total_nodes: 0,
    online_nodes: 0,
    total_networks: 0,
    total_storage_pools: 0
  })
  const recentActivity = ref<Array<{ id: string; message: string; time: string; type: string }>>([])
  const loading = ref(false)

  // Getters
  const vmUtilization = computed(() => {
    if (stats.value.total_vms === 0) return 0
    return Math.round((stats.value.running_vms / stats.value.total_vms) * 100)
  })

  const nodeUtilization = computed(() => {
    if (stats.value.total_nodes === 0) return 0
    return Math.round((stats.value.online_nodes / stats.value.total_nodes) * 100)
  })

  // Actions
  async function fetchStats() {
    loading.value = true
    try {
      // Fetch data from other stores
      const vmsStore = useVMsStore()
      const nodesStore = useNodesStore()

      await Promise.all([
        vmsStore.fetchVMs(),
        nodesStore.fetchNodes(),
        networksApi.listNetworks(),
        storageApi.listStoragePools()
      ])

      const networks = await networksApi.listNetworks()
      const storagePools = await storageApi.listStoragePools()

      stats.value = {
        total_vms: vmsStore.vms.length,
        running_vms: vmsStore.runningVMs.length,
        stopped_vms: vmsStore.stoppedVMs.length,
        total_nodes: nodesStore.nodes.length,
        online_nodes: nodesStore.onlineNodes.length,
        total_networks: networks.length,
        total_storage_pools: storagePools.length
      }
    } finally {
      loading.value = false
    }
  }

  async function fetchRecentActivity() {
    // This would fetch from an activity log endpoint
    // For now, return mock data
    recentActivity.value = [
      { id: '1', message: 'VM "web-server" started', time: '2 minutes ago', type: 'success' },
      { id: '2', message: 'Node "node1" registered', time: '5 minutes ago', type: 'info' },
      { id: '3', message: 'VM "db-server" stopped', time: '10 minutes ago', type: 'warning' }
    ]
  }

  return {
    stats,
    recentActivity,
    loading,
    vmUtilization,
    nodeUtilization,
    fetchStats,
    fetchRecentActivity
  }
})
