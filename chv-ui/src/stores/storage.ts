import { ref } from 'vue'
import { defineStore } from 'pinia'
import { storageApi } from '@/api/storage'
import type { StoragePool, CreateStoragePoolRequest } from '@/types'

export const useStorageStore = defineStore('storage', () => {
  // State
  const pools = ref<StoragePool[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Actions
  async function fetchStoragePools() {
    loading.value = true
    error.value = null
    try {
      const result = await storageApi.listStoragePools()
      pools.value = result
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to fetch storage pools'
    } finally {
      loading.value = false
    }
  }

  async function createStoragePool(request: CreateStoragePoolRequest): Promise<StoragePool> {
    loading.value = true
    error.value = null
    try {
      const result = await storageApi.createStoragePool(request)
      await fetchStoragePools()
      return result
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to create storage pool'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function deleteStoragePool(id: string): Promise<void> {
    loading.value = true
    error.value = null
    try {
      await storageApi.deleteStoragePool(id)
      await fetchStoragePools()
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to delete storage pool'
      throw err
    } finally {
      loading.value = false
    }
  }

  return {
    pools,
    loading,
    error,
    fetchStoragePools,
    createStoragePool,
    deleteStoragePool
  }
})
