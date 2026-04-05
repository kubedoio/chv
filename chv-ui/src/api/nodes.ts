import api from './client'
import type { Node } from '@/types'

export interface RegisterNodeRequest {
  hostname: string
  management_ip: string
  total_cpu_cores: number
  total_ram_mb: number
}

export interface MaintenanceRequest {
  enabled: boolean
}

export const nodesApi = {
  async listNodes(): Promise<Node[]> {
    const response = await api.get('/nodes')
    return response.data || []
  },

  async getNode(id: string): Promise<Node> {
    const response = await api.get(`/nodes/${id}`)
    return response.data
  },

  async registerNode(request: RegisterNodeRequest): Promise<Node> {
    const response = await api.post('/nodes/register', request)
    return response.data
  },

  async setMaintenance(id: string, enabled: boolean): Promise<void> {
    await api.post(`/nodes/${id}/maintenance`, { enabled })
  }
}
