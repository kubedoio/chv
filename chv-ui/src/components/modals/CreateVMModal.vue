<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useToast } from 'primevue/usetoast'
import Dialog from 'primevue/dialog'
import Steps from 'primevue/steps'
import InputText from 'primevue/inputtext'
import InputNumber from 'primevue/inputnumber'
import Dropdown from 'primevue/dropdown'
import MultiSelect from 'primevue/multiselect'
import Slider from 'primevue/slider'
import Textarea from 'primevue/textarea'
import Button from 'primevue/button'
import { vmsApi } from '@/api/vms'
import { imagesApi } from '@/api/images'
import { networksApi } from '@/api/networks'
import type { Image, Network, VMCreateRequest, CloudInitSpec } from '@/types'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
  'vm-created': []
}>()

const toast = useToast()

// Wizard state
const activeStep = ref(0)
const isSubmitting = ref(false)
const images = ref<Image[]>([])
const networks = ref<Network[]>([])
const loadingImages = ref(false)
const loadingNetworks = ref(false)

// Form data
const formData = ref({
  // Step 1: Basic Info
  name: '',
  cpu: 2,
  memory_mb: 2048,
  // Step 2: Storage
  image_id: '',
  disk_size_gb: 20,
  // Step 3: Network
  selected_networks: [] as Network[],
  cloud_init_enabled: false,
  cloud_init_user_data: ''
})

// Validation errors
const errors = ref<Record<string, string>>({})

// Steps configuration
const steps = ref([
  { label: 'Basic Info', icon: 'pi pi-info-circle' },
  { label: 'Storage', icon: 'pi pi-database' },
  { label: 'Network', icon: 'pi pi-globe' }
])

// Computed
const dialogVisible = computed({
  get: () => props.visible,
  set: (value) => emit('update:visible', value)
})

const isFirstStep = computed(() => activeStep.value === 0)
const isLastStep = computed(() => activeStep.value === steps.value.length - 1)

const availableImages = computed(() => {
  return images.value.filter(img => img.status === 'ready')
})

const selectedImage = computed(() => {
  return images.value.find(img => img.id === formData.value.image_id)
})

// OS Icons mapping
function getOSIcon(osFamily: string): string {
  const family = osFamily.toLowerCase()
  if (family.includes('ubuntu')) return 'pi pi-microsoft'
  if (family.includes('debian')) return 'pi pi-microsoft'
  if (family.includes('centos')) return 'pi pi-microsoft'
  if (family.includes('fedora')) return 'pi pi-microsoft'
  if (family.includes('rhel') || family.includes('redhat')) return 'pi pi-microsoft'
  if (family.includes('alpine')) return 'pi pi-microsoft'
  if (family.includes('windows')) return 'pi pi-microsoft'
  return 'pi pi-desktop'
}

// Fetch data when modal opens
watch(() => props.visible, async (visible) => {
  if (visible) {
    activeStep.value = 0
    resetForm()
    await fetchData()
  }
})

async function fetchData() {
  loadingImages.value = true
  loadingNetworks.value = true
  
  try {
    const [imagesResult, networksResult] = await Promise.all([
      imagesApi.listImages(),
      networksApi.listNetworks()
    ])
    images.value = imagesResult
    networks.value = networksResult
  } catch (err) {
    toast.add({
      severity: 'error',
      summary: 'Error',
      detail: 'Failed to load required data',
      life: 3000
    })
  } finally {
    loadingImages.value = false
    loadingNetworks.value = false
  }
}

function resetForm() {
  formData.value = {
    name: '',
    cpu: 2,
    memory_mb: 2048,
    image_id: '',
    disk_size_gb: 20,
    selected_networks: [],
    cloud_init_enabled: false,
    cloud_init_user_data: ''
  }
  errors.value = {}
}

// Validation
function validateStep(step: number): boolean {
  errors.value = {}
  let isValid = true

  switch (step) {
    case 0: // Basic Info
      if (!formData.value.name.trim()) {
        errors.value.name = 'Name is required'
        isValid = false
      } else if (!/^[a-zA-Z0-9-]+$/.test(formData.value.name)) {
        errors.value.name = 'Name can only contain letters, numbers, and hyphens'
        isValid = false
      }
      if (!formData.value.cpu || formData.value.cpu < 1) {
        errors.value.cpu = 'CPU must be at least 1'
        isValid = false
      }
      if (!formData.value.memory_mb || formData.value.memory_mb < 512) {
        errors.value.memory_mb = 'Memory must be at least 512 MB'
        isValid = false
      }
      break

    case 1: // Storage
      if (!formData.value.image_id) {
        errors.value.image_id = 'Please select an image'
        isValid = false
      }
      if (!formData.value.disk_size_gb || formData.value.disk_size_gb < 10) {
        errors.value.disk_size_gb = 'Disk size must be at least 10 GB'
        isValid = false
      }
      break

    case 2: // Network
      // Network selection is optional
      break
  }

  return isValid
}

function nextStep() {
  if (validateStep(activeStep.value)) {
    if (activeStep.value < steps.value.length - 1) {
      activeStep.value++
    }
  }
}

function prevStep() {
  if (activeStep.value > 0) {
    activeStep.value--
  }
}

function onStepChange(e: { index: number }) {
  // Only allow navigation to previous steps or current step if valid
  const newIndex = e.index
  if (newIndex < activeStep.value) {
    activeStep.value = newIndex
  } else if (newIndex > activeStep.value && validateStep(activeStep.value)) {
    activeStep.value = newIndex
  }
}

async function createVM() {
  if (!validateStep(activeStep.value)) {
    return
  }

  isSubmitting.value = true

  try {
    const cloudInit: CloudInitSpec | undefined = formData.value.cloud_init_enabled && formData.value.cloud_init_user_data.trim()
      ? { user_data: formData.value.cloud_init_user_data.trim() }
      : undefined

    const request: VMCreateRequest = {
      name: formData.value.name.trim(),
      cpu: formData.value.cpu,
      memory_mb: formData.value.memory_mb,
      image_id: formData.value.image_id,
      disk_size_bytes: formData.value.disk_size_gb * 1024 * 1024 * 1024, // Convert GB to bytes
      networks: formData.value.selected_networks.map(n => ({ network_id: n.id })),
      cloud_init: cloudInit
    }

    await vmsApi.createVM(request)

    toast.add({
      severity: 'success',
      summary: 'VM Created',
      detail: `Virtual machine "${formData.value.name}" has been created`,
      life: 3000
    })

    emit('vm-created')
    dialogVisible.value = false
  } catch (err: any) {
    const message = err?.response?.data?.error?.message || 'Failed to create VM'
    toast.add({
      severity: 'error',
      summary: 'Error',
      detail: message,
      life: 5000
    })
  } finally {
    isSubmitting.value = false
  }
}

function cancel() {
  dialogVisible.value = false
}
</script>

<template>
  <Dialog
    v-model:visible="dialogVisible"
    header="Create Virtual Machine"
    modal
    :closable="!isSubmitting"
    :closeOnEscape="!isSubmitting"
    class="create-vm-modal"
    :style="{ width: '600px' }"
  >
    <div class="wizard-container">
      <!-- Step Indicator -->
      <Steps
        :model="steps"
        :active-step="activeStep"
        @update:activeStep="onStepChange"
        class="wizard-steps"
      />

      <!-- Step 1: Basic Info -->
      <div v-if="activeStep === 0" class="step-content">
        <div class="form-section">
          <div class="form-group">
            <label for="vm-name">
              VM Name <span class="required">*</span>
            </label>
            <InputText
              id="vm-name"
              v-model="formData.name"
              placeholder="e.g., web-server-01"
              :class="{ 'p-invalid': errors.name }"
              class="w-full"
            />
            <small v-if="errors.name" class="p-error">{{ errors.name }}</small>
          </div>

          <div class="form-row">
            <div class="form-group">
              <label for="vm-cpu">
                vCPUs <span class="required">*</span>
              </label>
              <InputNumber
                id="vm-cpu"
                v-model="formData.cpu"
                :min="1"
                :max="64"
                show-buttons
                :class="{ 'p-invalid': errors.cpu }"
                class="w-full"
              />
              <small v-if="errors.cpu" class="p-error">{{ errors.cpu }}</small>
            </div>

            <div class="form-group">
              <label for="vm-memory">
                Memory (MB) <span class="required">*</span>
              </label>
              <InputNumber
                id="vm-memory"
                v-model="formData.memory_mb"
                :min="512"
                :max="262144"
                :step="512"
                show-buttons
                :class="{ 'p-invalid': errors.memory_mb }"
                class="w-full"
              />
              <small v-if="errors.memory_mb" class="p-error">{{ errors.memory_mb }}</small>
            </div>
          </div>

          <div class="memory-presets">
            <span class="preset-label">Quick select:</span>
            <button
              v-for="preset in [1024, 2048, 4096, 8192]"
              :key="preset"
              class="preset-btn"
              @click="formData.memory_mb = preset"
            >
              {{ preset >= 1024 ? `${preset / 1024} GB` : `${preset} MB` }}
            </button>
          </div>
        </div>
      </div>

      <!-- Step 2: Storage -->
      <div v-if="activeStep === 1" class="step-content">
        <div class="form-section">
          <div class="form-group">
            <label for="vm-image">
              OS Image <span class="required">*</span>
            </label>
            <Dropdown
              id="vm-image"
              v-model="formData.image_id"
              :options="availableImages"
              option-label="name"
              option-value="id"
              placeholder="Select an image"
              :loading="loadingImages"
              :class="{ 'p-invalid': errors.image_id }"
              class="w-full"
            >
              <template #value="slotProps">
                <div v-if="slotProps.value" class="image-option">
                  <i :class="getOSIcon(selectedImage?.os_family || '')"></i>
                  <span>{{ selectedImage?.name }}</span>
                  <span class="image-meta">{{ selectedImage?.architecture }}</span>
                </div>
                <span v-else>Select an image</span>
              </template>
              <template #option="slotProps">
                <div class="image-option">
                  <i :class="getOSIcon(slotProps.option.os_family)"></i>
                  <span>{{ slotProps.option.name }}</span>
                  <span class="image-meta">{{ slotProps.option.architecture }}</span>
                </div>
              </template>
            </Dropdown>
            <small v-if="errors.image_id" class="p-error">{{ errors.image_id }}</small>
            <small v-else-if="availableImages.length === 0 && !loadingImages" class="p-text-secondary">
              No images available. Please import an image first.
            </small>
          </div>

          <div class="form-group">
            <label for="vm-disk-size">
              Disk Size (GB) <span class="required">*</span>
            </label>
            <div class="disk-size-control">
              <Slider
                v-model="formData.disk_size_gb"
                :min="10"
                :max="500"
                class="disk-slider"
              />
              <InputNumber
                v-model="formData.disk_size_gb"
                :min="10"
                :max="500"
                suffix=" GB"
                class="disk-input"
              />
            </div>
            <small v-if="errors.disk_size_gb" class="p-error">{{ errors.disk_size_gb }}</small>
            <small v-else class="p-text-secondary">
              Range: 10 GB - 500 GB
            </small>
          </div>
        </div>
      </div>

      <!-- Step 3: Network -->
      <div v-if="activeStep === 2" class="step-content">
        <div class="form-section">
          <div class="form-group">
            <label for="vm-networks">
              Networks
            </label>
            <MultiSelect
              id="vm-networks"
              v-model="formData.selected_networks"
              :options="networks"
              option-label="name"
              placeholder="Select networks (optional)"
              :loading="loadingNetworks"
              display="chip"
              class="w-full"
            >
              <template #option="slotProps">
                <div class="network-option">
                  <i class="pi pi-globe"></i>
                  <div class="network-info">
                    <span class="network-name">{{ slotProps.option.name }}</span>
                    <span class="network-cidr">{{ slotProps.option.cidr }}</span>
                  </div>
                </div>
              </template>
            </MultiSelect>
            <small class="p-text-secondary">
              Select one or more networks to attach to this VM
            </small>
          </div>

          <div class="form-group cloud-init-section">
            <label class="checkbox-label">
              <input
                v-model="formData.cloud_init_enabled"
                type="checkbox"
              />
              <span>Enable Cloud-Init</span>
            </label>

            <div v-if="formData.cloud_init_enabled" class="cloud-init-config">
              <label for="cloud-init-userdata">Cloud-Init User Data (YAML)</label>
              <Textarea
                id="cloud-init-userdata"
                v-model="formData.cloud_init_user_data"
                rows="8"
                placeholder="#cloud-config
users:
  - name: admin
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - ssh-rsa AAAA..."
                class="w-full monospace"
              />
              <small class="p-text-secondary">
                Enter cloud-init configuration in YAML format
              </small>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Footer -->
    <template #footer>
      <div class="wizard-footer">
        <Button
          label="Cancel"
          text
          :disabled="isSubmitting"
          @click="cancel"
        />
        <div class="nav-buttons">
          <Button
            v-if="!isFirstStep"
            label="Previous"
            icon="pi pi-arrow-left"
            text
            :disabled="isSubmitting"
            @click="prevStep"
          />
          <Button
            v-if="!isLastStep"
            label="Next"
            icon="pi pi-arrow-right"
            icon-pos="right"
            :disabled="isSubmitting"
            @click="nextStep"
          />
          <Button
            v-else
            label="Create VM"
            icon="pi pi-check"
            :loading="isSubmitting"
            @click="createVM"
          />
        </div>
      </div>
    </template>
  </Dialog>
</template>

<style scoped>
.wizard-container {
  padding: 1rem 0;
}

.wizard-steps {
  margin-bottom: 1.5rem;
}

.step-content {
  min-height: 300px;
}

.form-section {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.form-group label {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-primary);
}

.form-group label .required {
  color: var(--color-error);
}

.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
}

.memory-presets {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.preset-label {
  font-size: 12px;
  color: var(--color-text-secondary);
  margin-right: 0.5rem;
}

.preset-btn {
  padding: 4px 12px;
  background-color: var(--color-hover);
  border: 1px solid var(--color-border);
  border-radius: 2px;
  font-size: 12px;
  cursor: pointer;
  transition: background-color 0.15s;
}

.preset-btn:hover {
  background-color: var(--color-selected);
}

.image-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.image-option i {
  color: var(--color-primary);
}

.image-meta {
  font-size: 11px;
  color: var(--color-text-secondary);
  margin-left: auto;
  background-color: var(--color-hover);
  padding: 2px 6px;
  border-radius: 2px;
}

.disk-size-control {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.disk-slider {
  flex: 1;
}

.disk-input {
  width: 120px;
  flex-shrink: 0;
}

.network-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.network-option i {
  color: var(--color-primary);
}

.network-info {
  display: flex;
  flex-direction: column;
}

.network-name {
  font-size: 13px;
}

.network-cidr {
  font-size: 11px;
  color: var(--color-text-secondary);
}

.cloud-init-section {
  border-top: 1px solid var(--color-border);
  padding-top: 1rem;
  margin-top: 0.5rem;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  font-size: 13px;
}

.checkbox-label input[type="checkbox"] {
  width: 16px;
  height: 16px;
  cursor: pointer;
}

.cloud-init-config {
  margin-top: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.monospace {
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 12px;
}

.wizard-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}

.nav-buttons {
  display: flex;
  gap: 0.5rem;
}

:deep(.p-dialog-content) {
  padding: 0 1.5rem;
}

:deep(.p-dialog-footer) {
  padding: 1rem 1.5rem;
  border-top: 1px solid var(--color-border);
}

:deep(.p-error) {
  color: var(--color-error);
  font-size: 12px;
}

:deep(.p-text-secondary) {
  color: var(--color-text-secondary);
  font-size: 12px;
}

:deep(.p-steps) {
  font-size: 12px;
}

:deep(.p-steps-item.p-highlight .p-steps-number) {
  background-color: var(--color-primary);
  color: white;
}

:deep(.p-inputnumber-input) {
  width: 100%;
}

:deep(.p-slider) {
  background-color: var(--color-border);
}

:deep(.p-slider-range) {
  background-color: var(--color-primary);
}

:deep(.p-slider-handle) {
  background-color: var(--color-primary);
  border-color: var(--color-primary);
}
</style>
