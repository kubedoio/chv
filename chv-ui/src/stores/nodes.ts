import { ref, computed } from 'vue'
import { defineStore } from 'pinia'
import { nodesApi } from '@/api/nodes'
import type { Node, RegisterNodeRequest } from '@/types'

export const useNodesStore = defineStore('nodes', () => {
  // State
  const nodes = ref<Node[]>([])
  const selectedNode = ref<Node | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Getters
  const onlineNodes = computed(() => nodes.value.filter(n => n.state === 'online'))
  const offlineNodes = computed(() => nodes.value.filter(n => n.state === 'offline'))
  const maintenanceNodes = computed(() => nodes.value.filter(n => n.maintenance_mode))

  // Actions
  async function fetchNodes() {
    loading.value = true
    error.value = null
    try {
      const result = await nodesApi.listNodes()
      nodes.value = result
    } catch (err: any) {
      error.value = err.response?.data?.error?.message || 'Failed to fetch nodes'
    } finally {
      loading.value = false
    }
  }

  async function registerNode(request: RegisterNodeRequest): Promise<Node> {
    loading.value = true
    error.value = null
    try {
      const result = await nodesApi.registerNode(request)
      await fetchNodes()
      return result
    } catch (err: any) {
      error.value = err.response?.data?.error?.message || 'Failed to register node'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function setMaintenance(id: string, enabled: boolean) {
    loading.value = true
    try {
      await nodesApi.setMaintenance(id, enabled)
      await fetchNodes()
    } catch (err: any) {
      error.value = err.response?.data?.error?.message || 'Failed to set maintenance mode'
      throw err
    } finally {
      loading.value = false
    }
  }

  function selectNode(node: Node | null) {
    selectedNode.value = node
  }

  return {
    nodes,
    selectedNode,
    loading,
    error,
    onlineNodes,
    offlineNodes,
    maintenanceNodes,
    fetchNodes,
    registerNode,
    setMaintenance,
    selectNode
  }
})
