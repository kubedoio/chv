import { ref, computed } from 'vue'
import { defineStore } from 'pinia'
import { authApi } from '@/api/auth'
import type { Token } from '@/types'

export const useAuthStore = defineStore('auth', () => {
  // State
  const token = ref<string | null>(localStorage.getItem('chv_token'))
  const tokenInfo = ref<Token | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Getters
  const isAuthenticated = computed(() => !!token.value)

  // Actions
  async function login(tokenValue: string) {
    token.value = tokenValue
    localStorage.setItem('chv_token', tokenValue)
    error.value = null
  }

  function logout() {
    token.value = null
    tokenInfo.value = null
    localStorage.removeItem('chv_token')
  }

  async function createToken(name: string, expiresIn: string): Promise<string> {
    loading.value = true
    error.value = null
    try {
      const result = await authApi.createToken({ name, expires_in: expiresIn })
      return result.token
    } catch (err: any) {
      error.value = err.response?.data?.error?.message || 'Failed to create token'
      throw err
    } finally {
      loading.value = false
    }
  }

  function checkAuth(): boolean {
    const stored = localStorage.getItem('chv_token')
    if (stored) {
      token.value = stored
      return true
    }
    return false
  }

  return {
    token,
    tokenInfo,
    loading,
    error,
    isAuthenticated,
    login,
    logout,
    createToken,
    checkAuth
  }
})
