<script setup lang="ts">
import { onMounted } from 'vue'
import { useDashboardStore } from '@/stores/dashboard'

const dashboardStore = useDashboardStore()

onMounted(() => {
  dashboardStore.fetchStats()
  dashboardStore.fetchRecentActivity()
})

function formatNumber(num: number): string {
  return num.toLocaleString()
}
</script>

<template>
  <div class="dashboard">
    <!-- Stats Cards -->
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-icon vms">
          <i class="pi pi-server"></i>
        </div>
        <div class="stat-content">
          <div class="stat-value">{{ formatNumber(dashboardStore.stats.total_vms) }}</div>
          <div class="stat-label">Total VMs</div>
          <div class="stat-sub">
            <span class="running">{{ dashboardStore.stats.running_vms }} running</span>
            <span class="separator">•</span>
            <span class="stopped">{{ dashboardStore.stats.stopped_vms }} stopped</span>
          </div>
        </div>
      </div>

      <div class="stat-card">
        <div class="stat-icon nodes">
          <i class="pi pi-sitemap"></i>
        </div>
        <div class="stat-content">
          <div class="stat-value">{{ formatNumber(dashboardStore.stats.total_nodes) }}</div>
          <div class="stat-label">Nodes</div>
          <div class="stat-sub">
            <span class="online">{{ dashboardStore.stats.online_nodes }} online</span>
          </div>
        </div>
      </div>

      <div class="stat-card">
        <div class="stat-icon networks">
          <i class="pi pi-globe"></i>
        </div>
        <div class="stat-content">
          <div class="stat-value">{{ formatNumber(dashboardStore.stats.total_networks) }}</div>
          <div class="stat-label">Networks</div>
        </div>
      </div>

      <div class="stat-card">
        <div class="stat-icon storage">
          <i class="pi pi-database"></i>
        </div>
        <div class="stat-content">
          <div class="stat-value">{{ formatNumber(dashboardStore.stats.total_storage_pools) }}</div>
          <div class="stat-label">Storage Pools</div>
        </div>
      </div>
    </div>

    <!-- Main Content Grid -->
    <div class="content-grid">
      <!-- Recent Activity -->
      <div class="card activity-card">
        <div class="card-header">
          <h2>Recent Activity</h2>
          <button class="view-all">View All</button>
        </div>
        <div class="activity-list">
          <div v-for="activity in dashboardStore.recentActivity" :key="activity.id" class="activity-item">
            <div :class="['activity-icon', activity.type]">
              <i :class="activity.type === 'success' ? 'pi pi-check' : activity.type === 'warning' ? 'pi pi-exclamation' : 'pi pi-info'"></i>
            </div>
            <div class="activity-content">
              <div class="activity-message">{{ activity.message }}</div>
              <div class="activity-time">{{ activity.time }}</div>
            </div>
          </div>
        </div>
      </div>

      <!-- Quick Actions -->
      <div class="card actions-card">
        <div class="card-header">
          <h2>Quick Actions</h2>
        </div>
        <div class="actions-list">
          <RouterLink to="/vms" class="action-item">
            <i class="pi pi-plus"></i>
            <span>Create VM</span>
          </RouterLink>
          <RouterLink to="/nodes" class="action-item">
            <i class="pi pi-plus"></i>
            <span>Register Node</span>
          </RouterLink>
          <RouterLink to="/networks" class="action-item">
            <i class="pi pi-plus"></i>
            <span>Create Network</span>
          </RouterLink>
          <RouterLink to="/storage" class="action-item">
            <i class="pi pi-plus"></i>
            <span>Add Storage</span>
          </RouterLink>
        </div>
      </div>

      <!-- System Health -->
      <div class="card health-card">
        <div class="card-header">
          <h2>System Health</h2>
        </div>
        <div class="health-items">
          <div class="health-item">
            <div class="health-status healthy">
              <i class="pi pi-check-circle"></i>
            </div>
            <div class="health-info">
              <div class="health-name">Database</div>
              <div class="health-state">Connected</div>
            </div>
          </div>
          <div class="health-item">
            <div class="health-status healthy">
              <i class="pi pi-check-circle"></i>
            </div>
            <div class="health-info">
              <div class="health-name">Controller</div>
              <div class="health-state">Running</div>
            </div>
          </div>
          <div class="health-item">
            <div class="health-status warning">
              <i class="pi pi-exclamation-circle"></i>
            </div>
            <div class="health-info">
              <div class="health-name">Storage</div>
              <div class="health-state">75% used</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
}

.stat-card {
  background: white;
  border: 1px solid var(--color-border);
  padding: 16px;
  display: flex;
  align-items: flex-start;
  gap: 16px;
}

.stat-icon {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 2px;
}

.stat-icon i {
  font-size: 24px;
}

.stat-icon.vms {
  background-color: rgba(0, 102, 204, 0.1);
  color: var(--color-primary);
}

.stat-icon.nodes {
  background-color: rgba(84, 180, 53, 0.1);
  color: var(--color-success);
}

.stat-icon.networks {
  background-color: rgba(240, 171, 0, 0.1);
  color: var(--color-warning);
}

.stat-icon.storage {
  background-color: rgba(102, 102, 102, 0.1);
  color: var(--color-text-secondary);
}

.stat-content {
  flex: 1;
}

.stat-value {
  font-size: 28px;
  font-weight: 600;
  color: var(--color-text-primary);
  line-height: 1;
}

.stat-label {
  font-size: 13px;
  color: var(--color-text-secondary);
  margin-top: 4px;
}

.stat-sub {
  font-size: 11px;
  margin-top: 8px;
  display: flex;
  gap: 8px;
  align-items: center;
}

.stat-sub .running {
  color: var(--color-success);
}

.stat-sub .stopped {
  color: var(--color-text-secondary);
}

.stat-sub .online {
  color: var(--color-success);
}

.stat-sub .separator {
  color: var(--color-border);
}

.content-grid {
  display: grid;
  grid-template-columns: 2fr 1fr 1fr;
  gap: 16px;
}

.card {
  background: white;
  border: 1px solid var(--color-border);
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
}

.card-header h2 {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.view-all {
  font-size: 12px;
  color: var(--color-primary);
  background: none;
  border: none;
  cursor: pointer;
}

.view-all:hover {
  text-decoration: underline;
}

.activity-list {
  padding: 8px 0;
}

.activity-item {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
}

.activity-item:last-child {
  border-bottom: none;
}

.activity-icon {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.activity-icon.success {
  background-color: rgba(84, 180, 53, 0.1);
  color: var(--color-success);
}

.activity-icon.warning {
  background-color: rgba(240, 171, 0, 0.1);
  color: var(--color-warning);
}

.activity-icon.info {
  background-color: rgba(0, 102, 204, 0.1);
  color: var(--color-primary);
}

.activity-icon i {
  font-size: 14px;
}

.activity-content {
  flex: 1;
}

.activity-message {
  font-size: 13px;
  color: var(--color-text-primary);
}

.activity-time {
  font-size: 11px;
  color: var(--color-text-secondary);
  margin-top: 2px;
}

.actions-list {
  padding: 8px;
}

.action-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  text-decoration: none;
  color: var(--color-text-primary);
  border-radius: 2px;
  transition: background-color 0.15s;
}

.action-item:hover {
  background-color: var(--color-hover);
}

.action-item i {
  font-size: 16px;
  color: var(--color-primary);
}

.action-item span {
  font-size: 13px;
}

.health-items {
  padding: 8px 0;
}

.health-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
}

.health-item:last-child {
  border-bottom: none;
}

.health-status {
  font-size: 20px;
}

.health-status.healthy {
  color: var(--color-success);
}

.health-status.warning {
  color: var(--color-warning);
}

.health-status.error {
  color: var(--color-error);
}

.health-info {
  flex: 1;
}

.health-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-primary);
}

.health-state {
  font-size: 11px;
  color: var(--color-text-secondary);
  margin-top: 2px;
}

@media (max-width: 1200px) {
  .stats-grid {
    grid-template-columns: repeat(2, 1fr);
  }
  
  .content-grid {
    grid-template-columns: 1fr 1fr;
  }
  
  .activity-card {
    grid-column: span 2;
  }
}

@media (max-width: 768px) {
  .stats-grid {
    grid-template-columns: 1fr;
  }
  
  .content-grid {
    grid-template-columns: 1fr;
  }
  
  .activity-card {
    grid-column: span 1;
  }
}
</style>
