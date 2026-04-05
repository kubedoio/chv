import api from './client'
import type { Network } from '@/types'

export interface CreateNetworkRequest {
  name: string
  bridge_name: string
  cidr: string
  gateway_ip: string
}

export const networksApi = {
  async listNetworks(): Promise<Network[]> {
    const response = await api.get('/networks')
    return response.data || []
  },

  async createNetwork(request: CreateNetworkRequest): Promise<Network> {
    const response = await api.post('/networks', request)
    return response.data
  },

  async deleteNetwork(id: string): Promise<void> {
    await api.delete(`/networks/${id}`)
  }
}
