import axios from 'axios'
import type { AxiosInstance, AxiosError, AxiosRequestConfig } from 'axios'
import type { APIError } from '@/types'

// API base URL - empty means use same origin (works with proxy)
const API_BASE_URL = ''

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
        // Don't treat aborted requests as errors
        if (error.code === 'ERR_CANCELED' || error.message === 'canceled') {
          return Promise.reject(error)
        }
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

  // Create a request with abort controller for cleanup
  createRequest(config: AxiosRequestConfig) {
    const controller = new AbortController()
    const promise = this.client.request({
      ...config,
      signal: controller.signal
    })
    return { promise, controller }
  }

  async healthCheck(): Promise<{ status: string }> {
    const baseUrl = API_BASE_URL || window.location.origin
    const response = await axios.get(`${baseUrl}/health`)
    return response.data
  }
}

export const apiClient = new APIClient()
export default apiClient.instance
