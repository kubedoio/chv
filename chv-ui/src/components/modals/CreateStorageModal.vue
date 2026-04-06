<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Dropdown from 'primevue/dropdown'
import Checkbox from 'primevue/checkbox'
import Button from 'primevue/button'
import { storageApi, type CreateStoragePoolRequest } from '@/api/storage'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  cancel: []
  created: []
}>()

const loading = ref(false)
const error = ref<string | null>(null)

const form = ref<CreateStoragePoolRequest>({
  name: '',
  pool_type: 'local',
  path_or_export: '',
  supports_online_resize: true
})

const typeOptions = [
  { label: 'Local', value: 'local' },
  { label: 'NFS', value: 'nfs' }
]

const pathLabel = computed(() => {
  return form.value.pool_type === 'nfs' ? 'Export' : 'Path'
})

const pathHelp = computed(() => {
  return form.value.pool_type === 'nfs'
    ? 'Format: server:/export (e.g., nfs-server:/export/pool)'
    : 'Directory path (e.g., /var/lib/chv/storage)'
})

function resetForm() {
  form.value = {
    name: '',
    pool_type: 'local',
    path_or_export: '',
    supports_online_resize: true
  }
  error.value = null
}

watch(() => props.visible, (newVisible) => {
  if (!newVisible) {
    resetForm()
  }
})

async function handleSubmit() {
  if (!form.value.name.trim()) {
    error.value = 'Name is required'
    return
  }
  if (!form.value.path_or_export.trim()) {
    error.value = `${pathLabel.value} is required`
    return
  }

  loading.value = true
  error.value = null

  try {
    await storageApi.createStoragePool(form.value)
    emit('created')
    resetForm()
  } catch (err: any) {
    error.value = err.response?.data?.error?.message || 'Failed to create storage pool'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <Dialog :visible="visible" header="Create Storage Pool" modal @update:visible="$emit('cancel')">
    <div class="form-grid">
      <div v-if="error" class="error-message">
        {{ error }}
      </div>
      <div class="field">
        <label>Name *</label>
        <InputText v-model="form.name" :disabled="loading" />
      </div>
      <div class="field">
        <label>Type *</label>
        <Dropdown v-model="form.pool_type" :options="typeOptions" optionLabel="label" optionValue="value" :disabled="loading" />
      </div>
      <div class="field">
        <label>{{ pathLabel }} *</label>
        <InputText v-model="form.path_or_export" :disabled="loading" />
        <small>{{ pathHelp }}</small>
      </div>
      <div class="field-checkbox">
        <Checkbox v-model="form.supports_online_resize" :binary="true" inputId="resize" :disabled="loading" />
        <label for="resize">Supports Online Resize</label>
      </div>
    </div>
    <template #footer>
      <Button label="Cancel" class="p-button-text" @click="$emit('cancel')" :disabled="loading" />
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
  background-color: var(--color-error-bg, rgba(244, 67, 54, 0.1));
  color: var(--color-error, #f44336);
  border-radius: 4px;
  font-size: 13px;
}
</style>
