import { ref } from 'vue'
import { defineStore } from 'pinia'
import { imagesApi } from '@/api/images'
import type { Image, ImportImageRequest } from '@/types'

export const useImagesStore = defineStore('images', () => {
  // State
  const images = ref<Image[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Actions
  async function fetchImages() {
    loading.value = true
    error.value = null
    try {
      const result = await imagesApi.listImages()
      images.value = result
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to fetch images'
    } finally {
      loading.value = false
    }
  }

  async function importImage(request: ImportImageRequest): Promise<Image> {
    loading.value = true
    error.value = null
    try {
      const result = await imagesApi.importImage(request)
      await fetchImages()
      return result
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to import image'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function deleteImage(id: string): Promise<void> {
    loading.value = true
    error.value = null
    try {
      await imagesApi.deleteImage(id)
      await fetchImages()
    } catch (err: any) {
      // Ignore aborted requests (component unmounted or navigation)
      if (err.code === 'ERR_CANCELED' || err.message === 'canceled') {
        return
      }
      error.value = err.response?.data?.error?.message || 'Failed to delete image'
      throw err
    } finally {
      loading.value = false
    }
  }

  return {
    images,
    loading,
    error,
    fetchImages,
    importImage,
    deleteImage
  }
})
