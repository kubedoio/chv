<script lang="ts">
  import { Activity, Cpu, HardDrive, MemoryStick, Clock } from 'lucide-svelte';
  import type { NodeHealth, NodeMetrics } from '$lib/api/types';

  interface Props {
    health: NodeHealth;
    showDetails?: boolean;
  }

  let { health, showDetails = true }: Props = $props();

  function getStatusColor(status: string): string {
    switch (status) {
      case 'online': return 'bg-green-500';
      case 'offline': return 'bg-red-500';
      case 'maintenance': return 'bg-yellow-500';
      case 'error': return 'bg-orange-500';
      default: return 'bg-slate-400';
    }
  }

  function getStatusTextColor(status: string): string {
    switch (status) {
      case 'online': return 'text-green-600';
      case 'offline': return 'text-red-600';
      case 'maintenance': return 'text-yellow-600';
      case 'error': return 'text-orange-600';
      default: return 'text-slate-500';
    }
  }

  function formatPercent(value: number): string {
    return `${value.toFixed(1)}%`;
  }

  function formatTimeAgo(timestamp: string): string {
    if (!timestamp) return 'Never';
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
  }

  function getBarColor(percent: number): string {
    if (percent >= 90) return 'bg-red-500';
    if (percent >= 75) return 'bg-yellow-500';
    return 'bg-green-500';
  }

  $effect(() => {
    // Component mounted
  });
</script>

<div class="health-status">
  <!-- Status Header -->
  <div class="flex items-center gap-3 mb-3">
    <div class="relative">
      <div class="w-3 h-3 rounded-full {getStatusColor(health.status)}"></div>
      {#if health.status === 'online'}
        <div class="absolute inset-0 w-3 h-3 rounded-full {getStatusColor(health.status)} animate-ping opacity-75"></div>
      {/if}
    </div>
    <span class="text-sm font-medium capitalize {getStatusTextColor(health.status)}">
      {health.status}
    </span>
    {#if health.last_seen_at}
      <span class="text-xs text-slate-400 flex items-center gap-1">
        <Clock size={12} />
        {formatTimeAgo(health.last_seen_at)}
      </span>
    {/if}
  </div>

  {#if showDetails && health.metrics}
    {@const m = health.metrics}
    <div class="space-y-3">
      <!-- CPU -->
      <div class="metric">
        <div class="flex items-center justify-between text-xs mb-1">
          <div class="flex items-center gap-1.5 text-slate-600">
            <Cpu size={14} />
            <span>CPU</span>
          </div>
          <span class="font-medium text-slate-700">{formatPercent(m.cpu_percent)}</span>
        </div>
        <div class="h-2 bg-slate-100 rounded-full overflow-hidden">
          <div 
            class="h-full transition-all duration-500 {getBarColor(m.cpu_percent)}"
            style="width: {Math.min(m.cpu_percent, 100)}%"
          ></div>
        </div>
      </div>

      <!-- Memory -->
      {#if m.memory_total_mb > 0}
        {@const memPercent = (m.memory_used_mb / m.memory_total_mb) * 100}
        <div class="metric">
          <div class="flex items-center justify-between text-xs mb-1">
            <div class="flex items-center gap-1.5 text-slate-600">
              <MemoryStick size={14} />
              <span>Memory</span>
            </div>
            <span class="font-medium text-slate-700">
              {formatPercent(memPercent)} ({Math.round(m.memory_used_mb / 1024)}GB / {Math.round(m.memory_total_mb / 1024)}GB)
            </span>
          </div>
          <div class="h-2 bg-slate-100 rounded-full overflow-hidden">
            <div 
              class="h-full transition-all duration-500 {getBarColor(memPercent)}"
              style="width: {Math.min(memPercent, 100)}%"
            ></div>
          </div>
        </div>
      {/if}

      <!-- Disk -->
      {#if m.disk_total_gb > 0}
        {@const diskPercent = (m.disk_used_gb / m.disk_total_gb) * 100}
        <div class="metric">
          <div class="flex items-center justify-between text-xs mb-1">
            <div class="flex items-center gap-1.5 text-slate-600">
              <HardDrive size={14} />
              <span>Disk</span>
            </div>
            <span class="font-medium text-slate-700">
              {formatPercent(diskPercent)} ({m.disk_used_gb}GB / {m.disk_total_gb}GB)
            </span>
          </div>
          <div class="h-2 bg-slate-100 rounded-full overflow-hidden">
            <div 
              class="h-full transition-all duration-500 {getBarColor(diskPercent)}"
              style="width: {Math.min(diskPercent, 100)}%"
            ></div>
          </div>
        </div>
      {/if}
    </div>
  {:else if showDetails}
    <div class="text-xs text-slate-400 italic">
      No metrics available
    </div>
  {/if}
</div>

<style>
  .health-status {
    @apply p-3 bg-slate-50 rounded-lg border border-slate-100;
  }
  
  .metric {
    @apply space-y-1;
  }
</style>
