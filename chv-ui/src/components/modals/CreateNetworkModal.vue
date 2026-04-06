<script setup lang="ts">
import { ref, reactive, watch } from 'vue'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import InputNumber from 'primevue/inputnumber'
import Button from 'primevue/button'
import { networksApi } from '@/api/networks'
import type { CreateNetworkRequest } from '@/api/networks'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  created: []
  cancel: []
}>()

const loading = ref(false)
const error = ref<string | null>(null)

interface FormErrors {
  name: string
  bridge_name: string
  cidr: string
  gateway_ip: string
  dns_servers: string
}

const form = reactive({
  name: '',
  bridge_name: '',
  cidr: '',
  gateway_ip: '',
  dns_servers: '',
  mtu: 1500 as number | null
})

const errors = reactive<FormErrors>({
  name: '',
  bridge_name: '',
  cidr: '',
  gateway_ip: '',
  dns_servers: ''
})

watch(() => props.visible, (newVisible) => {
  if (!newVisible) {
    resetForm()
  }
})

function resetForm() {
  form.name = ''
  form.bridge_name = ''
  form.cidr = ''
  form.gateway_ip = ''
  form.dns_servers = ''
  form.mtu = 1500
  clearErrors()
  error.value = null
}

function clearErrors() {
  errors.name = ''
  errors.bridge_name = ''
  errors.cidr = ''
  errors.gateway_ip = ''
  errors.dns_servers = ''
}

function validateCIDR(cidr: string): boolean {
  const cidrRegex = /^(\d{1,3}\.){3}\d{1,3}\/\d{1,2}$/
  if (!cidrRegex.test(cidr)) return false
  
  const [ip, prefix] = cidr.split('/')
  const parts = ip.split('.')
  if (parts.length !== 4) return false
  
  for (const part of parts) {
    const num = parseInt(part, 10)
    if (num < 0 || num > 255) return false
  }
  
  const prefixNum = parseInt(prefix, 10)
  return prefixNum >= 0 && prefixNum <= 32
}

function validateIP(ip: string): boolean {
  const ipRegex = /^(\d{1,3}\.){3}\d{1,3}$/
  if (!ipRegex.test(ip)) return false
  
  const parts = ip.split('.')
  for (const part of parts) {
    const num = parseInt(part, 10)
    if (num < 0 || num > 255) return false
  }
  
  return true
}

function validate(): boolean {
  clearErrors()
  let isValid = true

  if (!form.name.trim()) {
    errors.name = 'Name is required'
    isValid = false
  }

  if (!form.bridge_name.trim()) {
    errors.bridge_name = 'Bridge name is required'
    isValid = false
  }

  if (!form.cidr.trim()) {
    errors.cidr = 'CIDR is required'
    isValid = false
  } else if (!validateCIDR(form.cidr.trim())) {
    errors.cidr = 'Invalid CIDR format (e.g., 10.0.0.0/24)'
    isValid = false
  }

  if (!form.gateway_ip.trim()) {
    errors.gateway_ip = 'Gateway IP is required'
    isValid = false
  } else if (!validateIP(form.gateway_ip.trim())) {
    errors.gateway_ip = 'Invalid IP address format'
    isValid = false
  }

  // Validate DNS servers if provided
  if (form.dns_servers.trim()) {
    const dnsList = form.dns_servers.split(',').map(s => s.trim()).filter(s => s)
    for (const dns of dnsList) {
      if (!validateIP(dns)) {
        errors.dns_servers = `Invalid DNS server IP: ${dns}`
        isValid = false
        break
      }
    }
  }

  // Validate MTU
  if (form.mtu !== null && (form.mtu < 576 || form.mtu > 9000)) {
    error.value = 'MTU must be between 576 and 9000'
    isValid = false
  }

  return isValid
}

async function handleSubmit() {
  if (!validate()) return

  loading.value = true

  try {
    const request: CreateNetworkRequest & { dns_servers?: string; mtu?: number } = {
      name: form.name.trim(),
      bridge_name: form.bridge_name.trim(),
      cidr: form.cidr.trim(),
      gateway_ip: form.gateway_ip.trim()
    }

    if (form.dns_servers.trim()) {
      request.dns_servers = form.dns_servers.trim()
    }

    if (form.mtu && form.mtu !== 1500) {
      request.mtu = form.mtu
    }

    await networksApi.createNetwork(request)
    emit('created')
  } catch (err: any) {
    error.value = err.response?.data?.error?.message || 'Failed to create network'
  } finally {
    loading.value = false
  }
}

function handleCancel() {
  emit('cancel')
}

// Auto-generate bridge name from network name
function onNameBlur() {
  if (form.name && !form.bridge_name) {
    const sanitized = form.name.toLowerCase().replace(/[^a-z0-9-]/g, '-')
    form.bridge_name = `br-${sanitized}`
  }
}
</script>

<template>
  <Dialog :visible="visible" header="Create Network" modal @update:visible="$emit('cancel')">
    <div class="form-grid">
      <div v-if="error" class="error-message">
        {{ error }}
      </div>
      <div class="field">
        <label>Name *</label>
        <InputText v-model="form.name" :class="{'p-invalid': errors.name}" @blur="onNameBlur" :disabled="loading" />
        <small v-if="errors.name" class="p-error">{{ errors.name }}</small>
      </div>
      <div class="field">
        <label>Bridge Name *</label>
        <InputText v-model="form.bridge_name" :class="{'p-invalid': errors.bridge_name}" :disabled="loading" />
        <small v-if="errors.bridge_name" class="p-error">{{ errors.bridge_name }}</small>
      </div>
      <div class="field">
        <label>CIDR *</label>
        <InputText v-model="form.cidr" :class="{'p-invalid': errors.cidr}" placeholder="e.g., 10.0.0.0/24" :disabled="loading" />
        <small v-if="errors.cidr" class="p-error">{{ errors.cidr }}</small>
      </div>
      <div class="field">
        <label>Gateway IP *</label>
        <InputText v-model="form.gateway_ip" :class="{'p-invalid': errors.gateway_ip}" placeholder="e.g., 10.0.0.1" :disabled="loading" />
        <small v-if="errors.gateway_ip" class="p-error">{{ errors.gateway_ip }}</small>
      </div>
      <div class="field">
        <label>DNS Servers</label>
        <InputText v-model="form.dns_servers" :class="{'p-invalid': errors.dns_servers}" placeholder="e.g., 8.8.8.8, 8.8.4.4" :disabled="loading" />
        <small v-if="errors.dns_servers" class="p-error">{{ errors.dns_servers }}</small>
        <small v-else>Comma-separated list of DNS server IPs</small>
      </div>
      <div class="field">
        <label>MTU</label>
        <InputNumber v-model="form.mtu" :min="576" :max="9000" :disabled="loading" />
        <small>Default: 1500</small>
      </div>
    </div>
    <template #footer>
      <Button label="Cancel" class="p-button-text" @click="handleCancel" :disabled="loading" />
      <Button label="Create" @click="handleSubmit" :loading="loading" />
    </template>
  </Dialog>
</template>

<style scoped>
.form-grid {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 400px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.field label {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-primary);
}

.field small {
  font-size: 11px;
  color: var(--color-text-secondary);
  margin-top: 2px;
}

.field .p-error {
  color: var(--color-error);
}

.field :deep(.p-inputtext),
.field :deep(.p-inputnumber) {
  width: 100%;
}

.field :deep(.p-inputnumber-input) {
  width: 100%;
}

.error-message {
  padding: 8px 12px;
  background-color: var(--color-error-bg, rgba(230, 0, 0, 0.1));
  color: var(--color-error);
  border-radius: 2px;
  font-size: 13px;
}
</style>
