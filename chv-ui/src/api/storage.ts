import api from './client'
import type { StoragePool } from '@/types'

export interface CreateStoragePoolRequest {
  name: string
  pool_type: 'local' | 'nfs'
  path_or_export: string
  supports_online_resize: boolean
}

export const storageApi = {
  async listStoragePools(): Promise<StoragePool[]> {
    const response = await api.get('/storage-pools')
    return response.data || []
  },

  async createStoragePool(request: CreateStoragePoolRequest): Promise<StoragePool> {
    const response = await api.post('/storage-pools', request)
    return response.data
  },

  async deleteStoragePool(id: string): Promise<void> {
    await api.delete(`/storage-pools/${id}`)
  }
}
