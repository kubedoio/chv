import { ref } from 'vue'
import { defineStore } from 'pinia'
import { networksApi } from '@/api/networks'
import type { Network, CreateNetworkRequest } from '@/types'

export const useNetworksStore = defineStore('networks', () => {
  const networks = ref<Network[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchNetworks() {
    loading.value = true
    error.value = null
    try {
      const result = await networksApi.listNetworks()
      networks.value = result
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to fetch networks'
    } finally {
      loading.value = false
    }
  }

  async function createNetwork(request: CreateNetworkRequest): Promise<Network> {
    loading.value = true
    error.value = null
    try {
      const result = await networksApi.createNetwork(request)
      await fetchNetworks()
      return result
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to create network'
      throw err
    } finally {
      loading.value = false
    }
  }

  return {
    networks,
    loading,
    error,
    fetchNetworks,
    createNetwork
  }
})
