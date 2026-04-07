import api from './client'
import type { VM, VMCreateRequest, VMSpec, VMUpdateRequest } from '@/types'

export const vmsApi = {
  async listVMs(): Promise<VM[]> {
    const response = await api.get('/vms')
    return response.data || []
  },

  async getVM(id: string): Promise<VM> {
    const response = await api.get(`/vms/${id}`)
    return response.data
  },

  async createVM(request: VMCreateRequest): Promise<VM> {
    const response = await api.post('/vms', request)
    return response.data
  },

  async deleteVM(id: string): Promise<void> {
    await api.delete(`/vms/${id}`)
  },

  async startVM(id: string): Promise<void> {
    await api.post(`/vms/${id}/start`)
  },

  async stopVM(id: string): Promise<void> {
    await api.post(`/vms/${id}/stop`)
  },

  async rebootVM(id: string): Promise<void> {
    await api.post(`/vms/${id}/reboot`)
  },

  async resizeDisk(id: string, newSizeBytes: number): Promise<void> {
    await api.post(`/vms/${id}/resize-disk`, { new_size_bytes: newSizeBytes })
  },

  async updateVM(id: string, request: VMUpdateRequest): Promise<VM> {
    const response = await api.put(`/vms/${id}`, request)
    return response.data
  }
}
