<script setup lang="ts">
import { ref } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { useRouter } from 'vue-router'

const authStore = useAuthStore()
const router = useRouter()
const showUserMenu = ref(false)

function handleLogout() {
  authStore.logout()
  router.push('/login')
}
</script>

<template>
  <header class="app-header">
    <div class="header-left">
      <h1 class="page-title">{{ $route.name?.toString().charAt(0).toUpperCase() + $route.name?.toString().slice(1) }}</h1>
    </div>
    
    <div class="header-right">
      <!-- Notifications -->
      <button class="icon-button">
        <i class="pi pi-bell"></i>
        <span class="badge">3</span>
      </button>
      
      <!-- Help -->
      <button class="icon-button">
        <i class="pi pi-question-circle"></i>
      </button>
      
      <!-- User menu -->
      <div class="user-menu">
        <button class="user-button" @click="showUserMenu = !showUserMenu">
          <i class="pi pi-user"></i>
          <span>Admin</span>
          <i class="pi pi-chevron-down"></i>
        </button>
        
        <div v-if="showUserMenu" class="dropdown">
          <button @click="handleLogout">
            <i class="pi pi-sign-out"></i>
            Logout
          </button>
        </div>
      </div>
    </div>
  </header>
</template>

<style scoped>
.app-header {
  height: 48px;
  background-color: white;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
}

.header-left {
  display: flex;
  align-items: center;
}

.page-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--color-text-primary);
  text-transform: capitalize;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.icon-button {
  position: relative;
  width: 32px;
  height: 32px;
  border: none;
  background: transparent;
  border-radius: 2px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-secondary);
}

.icon-button:hover {
  background-color: var(--color-hover);
}

.icon-button i {
  font-size: 18px;
}

.badge {
  position: absolute;
  top: 2px;
  right: 2px;
  width: 16px;
  height: 16px;
  background-color: var(--color-error);
  color: white;
  font-size: 10px;
  font-weight: 600;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.user-menu {
  position: relative;
}

.user-button {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border: 1px solid var(--color-border);
  background: white;
  border-radius: 2px;
  cursor: pointer;
  font-size: 13px;
  color: var(--color-text-primary);
}

.user-button:hover {
  background-color: var(--color-hover);
}

.user-button i:first-child {
  font-size: 16px;
}

.user-button i:last-child {
  font-size: 12px;
  color: var(--color-text-secondary);
}

.dropdown {
  position: absolute;
  top: 100%;
  right: 0;
  margin-top: 4px;
  background: white;
  border: 1px solid var(--color-border);
  border-radius: 2px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  min-width: 150px;
  z-index: 100;
}

.dropdown button {
  width: 100%;
  padding: 10px 12px;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--color-text-primary);
}

.dropdown button:hover {
  background-color: var(--color-hover);
}

.dropdown button i {
  font-size: 14px;
  color: var(--color-text-secondary);
}
</style>
