<script setup lang="ts">
import { ref, reactive, watch } from 'vue'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import InputNumber from 'primevue/inputnumber'
import Button from 'primevue/button'
import { useNodesStore } from '@/stores/nodes'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  created: []
  cancel: []
}>()

const nodesStore = useNodesStore()

const loading = ref(false)
const error = ref<string | null>(null)

interface FormErrors {
  hostname: string
  management_ip: string
  total_cpu_cores: string
  total_ram_mb: string
}

const form = reactive({
  hostname: '',
  management_ip: '',
  total_cpu_cores: 4,
  total_ram_mb: 8192
})

const errors = reactive<FormErrors>({
  hostname: '',
  management_ip: '',
  total_cpu_cores: '',
  total_ram_mb: ''
})

watch(() => props.visible, (newVisible) => {
  if (!newVisible) {
    resetForm()
  }
})

function resetForm() {
  form.hostname = ''
  form.management_ip = ''
  form.total_cpu_cores = 4
  form.total_ram_mb = 8192
  clearErrors()
  error.value = null
}

function clearErrors() {
  errors.hostname = ''
  errors.management_ip = ''
  errors.total_cpu_cores = ''
  errors.total_ram_mb = ''
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

  if (!form.hostname.trim()) {
    errors.hostname = 'Hostname is required'
    isValid = false
  } else if (!/^[a-zA-Z0-9][a-zA-Z0-9-]{0,62}[a-zA-Z0-9]$/.test(form.hostname.trim())) {
    errors.hostname = 'Invalid hostname format'
    isValid = false
  }

  if (!form.management_ip.trim()) {
    errors.management_ip = 'Management IP is required'
    isValid = false
  } else if (!validateIP(form.management_ip.trim())) {
    errors.management_ip = 'Invalid IP address format'
    isValid = false
  }

  if (!form.total_cpu_cores || form.total_cpu_cores < 1) {
    errors.total_cpu_cores = 'CPU cores must be at least 1'
    isValid = false
  }

  if (!form.total_ram_mb || form.total_ram_mb < 512) {
    errors.total_ram_mb = 'RAM must be at least 512 MB'
    isValid = false
  }

  return isValid
}

async function handleSubmit() {
  if (!validate()) return

  loading.value = true
  error.value = null

  try {
    await nodesStore.registerNode({
      hostname: form.hostname.trim(),
      management_ip: form.management_ip.trim(),
      total_cpu_cores: form.total_cpu_cores,
      total_ram_mb: form.total_ram_mb
    })
    emit('created')
    resetForm()
  } catch (err: any) {
    error.value = err.response?.data?.error?.message || 'Failed to register node'
  } finally {
    loading.value = false
  }
}

function handleCancel() {
  emit('cancel')
}
</script>

<template>
  <Dialog :visible="visible" header="Register Node" modal @update:visible="$emit('cancel')">
    <div class="form-grid">
      <div v-if="error" class="error-message">
        {{ error }}
      </div>
      <div class="field">
        <label>Hostname *</label>
        <InputText v-model="form.hostname" :class="{'p-invalid': errors.hostname}" :disabled="loading" placeholder="e.g., node-01" />
        <small v-if="errors.hostname" class="p-error">{{ errors.hostname }}</small>
      </div>
      <div class="field">
        <label>Management IP *</label>
        <InputText v-model="form.management_ip" :class="{'p-invalid': errors.management_ip}" :disabled="loading" placeholder="e.g., 192.168.1.10" />
        <small v-if="errors.management_ip" class="p-error">{{ errors.management_ip }}</small>
      </div>
      <div class="field-row">
        <div class="field">
          <label>CPU Cores *</label>
          <InputNumber v-model="form.total_cpu_cores" :min="1" :max="256" :disabled="loading" />
          <small v-if="errors.total_cpu_cores" class="p-error">{{ errors.total_cpu_cores }}</small>
        </div>
        <div class="field">
          <label>RAM (MB) *</label>
          <InputNumber v-model="form.total_ram_mb" :min="512" :step="512" :disabled="loading" />
          <small v-if="errors.total_ram_mb" class="p-error">{{ errors.total_ram_mb }}</small>
        </div>
      </div>
    </div>
    <template #footer>
      <Button label="Cancel" class="p-button-text" @click="handleCancel" :disabled="loading" />
      <Button label="Register" @click="handleSubmit" :loading="loading" />
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

.field-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
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
