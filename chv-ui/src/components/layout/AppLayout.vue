<script setup lang="ts">
import { onMounted } from 'vue'
import AppSidebar from './AppSidebar.vue'
import AppHeader from './AppHeader.vue'
import Toast from 'primevue/toast'
import { useAuthStore } from '@/stores/auth'
import { useRoute, useRouter } from 'vue-router'

const authStore = useAuthStore()
const router = useRouter()
const route = useRoute()

onMounted(() => {
  if (!authStore.checkAuth()) {
    router.push('/login')
  }
})
</script>

<template>
  <div class="app-layout">
    <Toast />
    <AppSidebar />
    <div class="main-content">
      <AppHeader />
      <main class="content-area">
        <RouterView />
      </main>
    </div>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  height: 100vh;
  overflow: hidden;
}

.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.content-area {
  flex: 1;
  overflow: auto;
  background-color: var(--color-bg-chrome);
  padding: 16px;
}
</style>
