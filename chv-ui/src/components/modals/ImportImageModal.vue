<script setup lang="ts">
import { ref, reactive, watch } from 'vue'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Dropdown from 'primevue/dropdown'
import Checkbox from 'primevue/checkbox'
import Button from 'primevue/button'
import { useImagesStore } from '@/stores/images'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  created: []
  cancel: []
}>()

const imagesStore = useImagesStore()

const loading = ref(false)
const error = ref<string | null>(null)

interface FormErrors {
  name: string
  source_url: string
  os_family: string
  architecture: string
  source_format: string
}

const form = reactive({
  name: '',
  source_url: '',
  os_family: '',
  architecture: 'x86_64',
  source_format: 'qcow2',
  cloud_init_supported: true
})

const errors = reactive<FormErrors>({
  name: '',
  source_url: '',
  os_family: '',
  architecture: '',
  source_format: ''
})

const osFamilyOptions = [
  { label: 'Ubuntu', value: 'ubuntu' },
  { label: 'Debian', value: 'debian' },
  { label: 'CentOS', value: 'centos' },
  { label: 'Fedora', value: 'fedora' },
  { label: 'RHEL', value: 'rhel' },
  { label: 'Alpine', value: 'alpine' },
  { label: 'Windows', value: 'windows' },
  { label: 'Other', value: 'other' }
]

const architectureOptions = [
  { label: 'x86_64', value: 'x86_64' },
  { label: 'aarch64', value: 'aarch64' }
]

const formatOptions = [
  { label: 'QCOW2', value: 'qcow2' },
  { label: 'Raw', value: 'raw' },
  { label: 'VMDK', value: 'vmdk' }
]

watch(() => props.visible, (newVisible) => {
  if (!newVisible) {
    resetForm()
  }
})

function resetForm() {
  form.name = ''
  form.source_url = ''
  form.os_family = ''
  form.architecture = 'x86_64'
  form.source_format = 'qcow2'
  form.cloud_init_supported = true
  clearErrors()
  error.value = null
}

function clearErrors() {
  errors.name = ''
  errors.source_url = ''
  errors.os_family = ''
  errors.architecture = ''
  errors.source_format = ''
}

function validate(): boolean {
  clearErrors()
  let isValid = true

  if (!form.name.trim()) {
    errors.name = 'Name is required'
    isValid = false
  }

  if (!form.source_url.trim()) {
    errors.source_url = 'Source URL is required'
    isValid = false
  } else {
    try {
      new URL(form.source_url.trim())
    } catch {
      errors.source_url = 'Invalid URL format'
      isValid = false
    }
  }

  if (!form.os_family) {
    errors.os_family = 'OS family is required'
    isValid = false
  }

  return isValid
}

async function handleSubmit() {
  if (!validate()) return

  loading.value = true
  error.value = null

  try {
    await imagesStore.importImage({
      name: form.name.trim(),
      source_url: form.source_url.trim(),
      os_family: form.os_family,
      architecture: form.architecture,
      source_format: form.source_format as 'qcow2' | 'raw' | 'vmdk',
      cloud_init_supported: form.cloud_init_supported
    })
    emit('created')
    resetForm()
  } catch (err: any) {
    error.value = err.response?.data?.error?.message || 'Failed to import image'
  } finally {
    loading.value = false
  }
}

function handleCancel() {
  emit('cancel')
}
</script>

<template>
  <Dialog :visible="visible" header="Import Image" modal @update:visible="$emit('cancel')">
    <div class="form-grid">
      <div v-if="error" class="error-message">
        {{ error }}
      </div>
      <div class="field">
        <label>Name *</label>
        <InputText v-model="form.name" :class="{'p-invalid': errors.name}" :disabled="loading" placeholder="e.g., Ubuntu 22.04 LTS" />
        <small v-if="errors.name" class="p-error">{{ errors.name }}</small>
      </div>
      <div class="field">
        <label>Source URL *</label>
        <InputText v-model="form.source_url" :class="{'p-invalid': errors.source_url}" :disabled="loading" placeholder="https://example.com/image.qcow2" />
        <small v-if="errors.source_url" class="p-error">{{ errors.source_url }}</small>
        <small v-else>Direct URL to the image file</small>
      </div>
      <div class="field-row">
        <div class="field">
          <label>OS Family *</label>
          <Dropdown v-model="form.os_family" :options="osFamilyOptions" optionLabel="label" optionValue="value" :class="{'p-invalid': errors.os_family}" :disabled="loading" placeholder="Select OS" />
          <small v-if="errors.os_family" class="p-error">{{ errors.os_family }}</small>
        </div>
        <div class="field">
          <label>Architecture *</label>
          <Dropdown v-model="form.architecture" :options="architectureOptions" optionLabel="label" optionValue="value" :disabled="loading" />
        </div>
      </div>
      <div class="field">
        <label>Format *</label>
        <Dropdown v-model="form.source_format" :options="formatOptions" optionLabel="label" optionValue="value" :disabled="loading" />
      </div>
      <div class="field-checkbox">
        <Checkbox v-model="form.cloud_init_supported" :binary="true" inputId="cloud-init" :disabled="loading" />
        <label for="cloud-init">Supports Cloud-Init</label>
      </div>
    </div>
    <template #footer>
      <Button label="Cancel" class="p-button-text" @click="handleCancel" :disabled="loading" />
      <Button label="Import" @click="handleSubmit" :loading="loading" />
    </template>
  </Dialog>
</template>

<style scoped>
.form-grid {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 450px;
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
.field :deep(.p-dropdown) {
  width: 100%;
}

.field-checkbox {
  display: flex;
  align-items: center;
  gap: 8px;
}

.field-checkbox label {
  font-size: 13px;
  color: var(--color-text-primary);
}

.error-message {
  padding: 8px 12px;
  background-color: var(--color-error-bg, rgba(230, 0, 0, 0.1));
  color: var(--color-error);
  border-radius: 2px;
  font-size: 13px;
}
</style>
