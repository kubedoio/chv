import { ref, computed } from 'vue'
import { defineStore } from 'pinia'
import { vmsApi } from '@/api/vms'
import type { VM, VMCreateRequest, VMUpdateRequest } from '@/types'

export const useVMsStore = defineStore('vms', () => {
  // State
  const vms = ref<VM[]>([])
  const selectedVM = ref<VM | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Getters
  const runningVMs = computed(() => vms.value.filter(vm => vm.actual_state === 'running'))
  const stoppedVMs = computed(() => vms.value.filter(vm => vm.actual_state === 'stopped'))
  const errorVMs = computed(() => vms.value.filter(vm => vm.actual_state === 'error'))

  // Actions
  async function fetchVMs() {
    loading.value = true
    error.value = null
    try {
      const result = await vmsApi.listVMs()
      vms.value = result
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to fetch VMs'
    } finally {
      loading.value = false
    }
  }

  async function fetchVM(id: string) {
    loading.value = true
    error.value = null
    try {
      const result = await vmsApi.getVM(id)
      selectedVM.value = result
      return result
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to fetch VM'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function createVM(request: VMCreateRequest): Promise<VM> {
    loading.value = true
    error.value = null
    try {
      const result = await vmsApi.createVM(request)
      await fetchVMs() // Refresh list
      return result
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to create VM'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function deleteVM(id: string) {
    loading.value = true
    error.value = null
    try {
      await vmsApi.deleteVM(id)
      await fetchVMs() // Refresh list
      if (selectedVM.value?.id === id) {
        selectedVM.value = null
      }
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to delete VM'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function startVM(id: string) {
    loading.value = true
    try {
      await vmsApi.startVM(id)
      await fetchVMs()
      if (selectedVM.value?.id === id) {
        await fetchVM(id)
      }
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to start VM'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function stopVM(id: string) {
    loading.value = true
    try {
      await vmsApi.stopVM(id)
      await fetchVMs()
      if (selectedVM.value?.id === id) {
        await fetchVM(id)
      }
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to stop VM'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function rebootVM(id: string) {
    loading.value = true
    try {
      await vmsApi.rebootVM(id)
      await fetchVMs()
      if (selectedVM.value?.id === id) {
        await fetchVM(id)
      }
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to reboot VM'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function updateVM(id: string, spec: VMUpdateRequest['spec']) {
    loading.value = true
    error.value = null
    try {
      const updated = await vmsApi.updateVM(id, { spec })
      // Update selectedVM if it's the one being updated
      if (selectedVM.value?.id === id) {
        selectedVM.value = updated
      }
      // Refresh the VM list
      await fetchVMs()
      return updated
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to update VM'
      throw err
    } finally {
      loading.value = false
    }
  }

  function selectVM(vm: VM | null) {
    selectedVM.value = vm
  }

  return {
    vms,
    selectedVM,
    loading,
    error,
    runningVMs,
    stoppedVMs,
    errorVMs,
    fetchVMs,
    fetchVM,
    createVM,
    deleteVM,
    startVM,
    stopVM,
    rebootVM,
    updateVM,
    selectVM
  }
})
