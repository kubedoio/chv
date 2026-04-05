<script setup lang="ts">
import { useRoute } from 'vue-router'

const route = useRoute()

const menuItems = [
  { path: '/', name: 'Dashboard', icon: 'pi pi-home' },
  { path: '/vms', name: 'Virtual Machines', icon: 'pi pi-server' },
  { path: '/nodes', name: 'Nodes', icon: 'pi pi-sitemap' },
  { path: '/networks', name: 'Networks', icon: 'pi pi-globe' },
  { path: '/storage', name: 'Storage', icon: 'pi pi-database' },
  { path: '/images', name: 'Images', icon: 'pi pi-image' }
]

function isActive(path: string) {
  if (path === '/') {
    return route.path === '/'
  }
  return route.path.startsWith(path)
}
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header">
      <div class="logo">
        <i class="pi pi-cloud"></i>
        <span>CHV</span>
      </div>
      <div class="subtitle">Cloud Hypervisor</div>
    </div>
    
    <nav class="sidebar-nav">
      <RouterLink
        v-for="item in menuItems"
        :key="item.path"
        :to="item.path"
        :class="['nav-item', { active: isActive(item.path) }]"
      >
        <i :class="item.icon"></i>
        <span>{{ item.name }}</span>
      </RouterLink>
    </nav>
    
    <div class="sidebar-footer">
      <div class="version">v0.1.0</div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 240px;
  background-color: white;
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
}

.sidebar-header {
  padding: 16px;
  border-bottom: 1px solid var(--color-border);
}

.logo {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 20px;
  font-weight: 700;
  color: var(--color-primary);
}

.logo i {
  font-size: 24px;
}

.subtitle {
  font-size: 11px;
  color: var(--color-text-secondary);
  margin-top: 2px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.sidebar-nav {
  flex: 1;
  padding: 8px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  color: var(--color-text-primary);
  text-decoration: none;
  border-radius: 2px;
  transition: background-color 0.15s;
}

.nav-item:hover {
  background-color: var(--color-hover);
}

.nav-item.active {
  background-color: var(--color-selected);
  color: var(--color-primary);
  font-weight: 500;
}

.nav-item i {
  font-size: 18px;
  width: 20px;
  text-align: center;
}

.nav-item span {
  font-size: 14px;
}

.sidebar-footer {
  padding: 12px 16px;
  border-top: 1px solid var(--color-border);
}

.version {
  font-size: 11px;
  color: var(--color-text-secondary);
  text-align: center;
}
</style>
