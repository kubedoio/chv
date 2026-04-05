<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const authStore = useAuthStore()

const token = ref('')
const loading = ref(false)
const error = ref('')

async function handleLogin() {
  if (!token.value.trim()) {
    error.value = 'Please enter an API token'
    return
  }

  loading.value = true
  error.value = ''

  try {
    await authStore.login(token.value.trim())
    router.push('/')
  } catch (err) {
    error.value = 'Invalid token. Please check and try again.'
  } finally {
    loading.value = false
  }
}

async function handleCreateToken() {
  loading.value = true
  error.value = ''

  try {
    const newToken = await authStore.createToken('web-ui', '24h')
    token.value = newToken
    await handleLogin()
  } catch (err: any) {
    error.value = err.response?.data?.error?.message || 'Failed to create token'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="login-page">
    <div class="login-box">
      <div class="login-header">
        <i class="pi pi-cloud logo-icon"></i>
        <h1>CHV</h1>
        <p>Cloud Hypervisor Platform</p>
      </div>

      <form @submit.prevent="handleLogin" class="login-form">
        <div class="form-group">
          <label for="token">API Token</label>
          <input
            id="token"
            v-model="token"
            type="password"
            placeholder="Enter your API token"
            :disabled="loading"
          />
          <span class="help-text">
            Create a token using: 
            <code>curl -X POST http://localhost:8081/api/v1/tokens -H "Content-Type: application/json" -d '{"name":"ui","expires_in":"24h"}'</code>
          </span>
        </div>

        <div v-if="error" class="error-message">
          <i class="pi pi-exclamation-circle"></i>
          {{ error }}
        </div>

        <button type="submit" class="login-button" :disabled="loading">
          <i v-if="loading" class="pi pi-spin pi-spinner"></i>
          <span v-else>Login</span>
        </button>

        <div class="divider">
          <span>or</span>
        </div>

        <button type="button" class="create-token-button" @click="handleCreateToken" :disabled="loading">
          <i class="pi pi-plus"></i>
          Create New Token
        </button>
      </form>

      <div class="login-footer">
        <a href="#">Documentation</a>
        <span>•</span>
        <a href="#">Support</a>
      </div>
    </div>
  </div>
</template>

<style scoped>
.login-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: var(--color-bg-chrome);
  padding: 20px;
}

.login-box {
  width: 100%;
  max-width: 400px;
  background: white;
  border: 1px solid var(--color-border);
  padding: 32px;
}

.login-header {
  text-align: center;
  margin-bottom: 32px;
}

.logo-icon {
  font-size: 48px;
  color: var(--color-primary);
  margin-bottom: 16px;
}

.login-header h1 {
  font-size: 24px;
  font-weight: 700;
  color: var(--color-text-primary);
  margin-bottom: 4px;
}

.login-header p {
  font-size: 14px;
  color: var(--color-text-secondary);
}

.login-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-group label {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-primary);
}

.form-group input {
  padding: 10px 12px;
  border: 1px solid var(--color-border);
  border-radius: 2px;
  font-size: 14px;
  font-family: 'Roboto Mono', monospace;
}

.form-group input:focus {
  outline: none;
  border-color: var(--color-primary);
}

.help-text {
  font-size: 11px;
  color: var(--color-text-secondary);
  line-height: 1.4;
}

.help-text code {
  display: block;
  background: #f5f5f5;
  padding: 8px;
  margin-top: 6px;
  border-radius: 2px;
  font-family: 'Roboto Mono', monospace;
  font-size: 10px;
  overflow-x: auto;
}

.error-message {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  background-color: rgba(230, 0, 0, 0.1);
  color: var(--color-error);
  font-size: 13px;
  border-radius: 2px;
}

.login-button {
  padding: 12px;
  background-color: var(--color-primary);
  color: white;
  border: none;
  border-radius: 2px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.login-button:hover:not(:disabled) {
  background-color: #0052a3;
}

.login-button:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.divider {
  display: flex;
  align-items: center;
  text-align: center;
  margin: 8px 0;
}

.divider::before,
.divider::after {
  content: '';
  flex: 1;
  border-bottom: 1px solid var(--color-border);
}

.divider span {
  padding: 0 16px;
  font-size: 12px;
  color: var(--color-text-secondary);
}

.create-token-button {
  padding: 12px;
  background-color: white;
  color: var(--color-text-primary);
  border: 1px solid var(--color-border);
  border-radius: 2px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.create-token-button:hover:not(:disabled) {
  background-color: var(--color-hover);
}

.create-token-button:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.login-footer {
  margin-top: 24px;
  text-align: center;
  font-size: 12px;
  color: var(--color-text-secondary);
}

.login-footer a {
  color: var(--color-primary);
  text-decoration: none;
}

.login-footer a:hover {
  text-decoration: underline;
}

.login-footer span {
  margin: 0 8px;
}
</style>
