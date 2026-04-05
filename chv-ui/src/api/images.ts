import api from './client'
import type { Image } from '@/types'

export interface ImportImageRequest {
  name: string
  os_family: string
  source_url: string
  source_format: 'qcow2' | 'raw' | 'vmdk'
  architecture: string
  cloud_init_supported: boolean
}

export const imagesApi = {
  async listImages(): Promise<Image[]> {
    const response = await api.get('/images')
    return response.data || []
  },

  async importImage(request: ImportImageRequest): Promise<Image> {
    const response = await api.post('/images/import', request)
    return response.data
  },

  async deleteImage(id: string): Promise<void> {
    await api.delete(`/images/${id}`)
  }
}
