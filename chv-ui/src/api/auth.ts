import api from './client'
import type { Token } from '@/types'

export interface CreateTokenRequest {
  name: string
  expires_in: string
}

export const authApi = {
  async createToken(request: CreateTokenRequest): Promise<Token> {
    const response = await api.post('/tokens', request)
    return response.data
  },

  async revokeToken(tokenId: string): Promise<void> {
    await api.delete(`/tokens/${tokenId}`)
  },

  async listTokens(): Promise<Token[]> {
    const response = await api.get('/tokens')
    return response.data
  }
}
