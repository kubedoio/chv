import axios from 'axios'
import type { AxiosInstance, AxiosError } from 'axios'
import type { APIError } from '@/types'

const API_BASE_URL = import.meta.env.VITE_API_URL || ''

class APIClient {
  private client: AxiosInstance

  constructor() {
    this.client = axios.create({
      baseURL: `${API_BASE_URL}/api/v1`,
      headers: {
        'Content-Type': 'application/json'
      },
      timeout: 30000
    })

    // Request interceptor to add auth token
    this.client.interceptors.request.use(
      (config) => {
        const token = localStorage.getItem('chv_token')
        if (token) {
          config.headers.Authorization = `Bearer ${token}`
        }
        return config
      },
      (error) => Promise.reject(error)
    )

    // Response interceptor for error handling
    this.client.interceptors.response.use(
      (response) => response,
      (error: AxiosError<APIError>) => {
        if (error.response?.status === 401) {
          // Token expired or invalid
          localStorage.removeItem('chv_token')
          window.location.href = '/login'
        }
        return Promise.reject(error)
      }
    )
  }

  get instance(): AxiosInstance {
    return this.client
  }

  async healthCheck(): Promise<{ status: string }> {
    const baseUrl = API_BASE_URL || window.location.origin
    const response = await axios.get(`${baseUrl}/health`)
    return response.data
  }
}

export const apiClient = new APIClient()
export default apiClient.instance
