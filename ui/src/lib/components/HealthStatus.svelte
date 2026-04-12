<script lang="ts">
  import { ChevronDown, ChevronUp, RefreshCw, CheckCircle, AlertCircle, XCircle, Loader2 } from 'lucide-svelte';

  export interface HealthCheck {
    id: string;
    name: string;
    status: 'healthy' | 'warning' | 'error' | 'pending';
    message?: string;
    lastChecked?: string;
    details?: string[];
  }

  interface Props {
    checks: HealthCheck[];
    loading?: boolean;
    onRefresh?: () => void;
    lastUpdated?: Date;
  }

  let { checks, loading = false, onRefresh, lastUpdated = new Date() }: Props = $props();

  let expandedChecks = $state<Set<string>>(new Set());
  let isCollapsed = $state(false);

  const statusConfig = {
    healthy: { 
      icon: CheckCircle, 
      colorClass: 'text-green-600', 
      bgClass: 'bg-green-50',
      borderClass: 'border-green-200',
      label: 'Healthy'
    },
    warning: { 
      icon: AlertCircle, 
      colorClass: 'text-amber-600', 
      bgClass: 'bg-amber-50',
      borderClass: 'border-amber-200',
      label: 'Warning'
    },
    error: { 
      icon: XCircle, 
      colorClass: 'text-red-600', 
      bgClass: 'bg-red-50',
      borderClass: 'border-red-200',
      label: 'Error'
    },
    pending: { 
      icon: Loader2, 
      colorClass: 'text-blue-600', 
      bgClass: 'bg-blue-50',
      borderClass: 'border-blue-200',
      label: 'Checking'
    }
  };

  const overallStatus = $derived.by(() => {
    if (checks.length === 0) return 'pending';
    if (checks.some(c => c.status === 'error')) return 'error';
    if (checks.some(c => c.status === 'warning')) return 'warning';
    return 'healthy';
  });

  const overallConfig = $derived(statusConfig[overallStatus]);

  function toggleExpand(id: string) {
    const newSet = new Set(expandedChecks);
    if (newSet.has(id)) {
      newSet.delete(id);
    } else {
      newSet.add(id);
    }
    expandedChecks = newSet;
  }

  function formatTime(date: Date): string {
    const now = new Date();
    const diff = Math.floor((now.getTime() - date.getTime()) / 1000);
    
    if (diff < 60) return 'just now';
    if (diff < 3600) return `${Math.floor(diff / 60)} min ago`;
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }
</script>

<div class="health-status card" role="region" aria-label="System Health Status">
  <!-- Header -->
  <div class="px-5 py-4 border-b border-slate-100">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-3">
        <div class="p-2 rounded-lg {overallConfig.bgClass}">
          <overallConfig.icon size={20} class={overallConfig.colorClass} />
        </div>
        <div>
          <h3 class="font-semibold text-slate-900">System Health</h3>
          <p class="text-xs text-slate-500">
            {#if loading}
              Checking status...
            {:else}
              Updated {formatTime(lastUpdated)}
            {/if}
          </p>
        </div>
      </div>
      <div class="flex items-center gap-2">
        {#if onRefresh}
          <button
            onclick={onRefresh}
            class="p-2 text-slate-400 hover:text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
            aria-label="Refresh health status"
            disabled={loading}
          >
            <RefreshCw size={16} class={loading ? 'animate-spin' : ''} />
          </button>
        {/if}
        <button
          onclick={() => isCollapsed = !isCollapsed}
          class="p-2 text-slate-400 hover:text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
          aria-label={isCollapsed ? 'Expand' : 'Collapse'}
        >
          {#if isCollapsed}
            <ChevronDown size={16} />
          {:else}
            <ChevronUp size={16} />
          {/if}
        </button>
      </div>
    </div>
  </div>

  <!-- Health Checks List -->
  {#if !isCollapsed}
    <div class="divide-y divide-slate-100">
      {#each checks as check}
        {@const config = statusConfig[check.status]}
        <div class="health-check">
          <button
            class="w-full px-5 py-3 flex items-center justify-between hover:bg-slate-50 transition-colors text-left"
            onclick={() => toggleExpand(check.id)}
            aria-expanded={expandedChecks.has(check.id)}
          >
            <div class="flex items-center gap-3">
              <div class="p-1.5 rounded-md {config.bgClass}">
                <config.icon size={14} class={config.colorClass} />
              </div>
              <div>
                <span class="text-sm font-medium text-slate-700">{check.name}</span>
                {#if check.message}
                  <p class="text-xs text-slate-500">{check.message}</p>
                {/if}
              </div>
            </div>
            <div class="flex items-center gap-2">
              <span class="text-xs font-medium {config.colorClass}">{config.label}</span>
              {#if check.details && check.details.length > 0}
                <ChevronDown 
                  size={14} 
                  class="text-slate-400 transition-transform {expandedChecks.has(check.id) ? 'rotate-180' : ''}" 
                />
              {/if}
            </div>
          </button>
          
          {#if expandedChecks.has(check.id) && check.details && check.details.length > 0}
            <div class="px-5 pb-3 pl-14">
              <ul class="space-y-1">
                {#each check.details as detail}
                  <li class="text-xs text-slate-500 flex items-start gap-2">
                    <span class="w-1 h-1 rounded-full bg-slate-400 mt-1.5"></span>
                    {detail}
                  </li>
                {/each}
              </ul>
            </div>
          {/if}
        </div>
      {/each}
      
      {#if checks.length === 0}
        <div class="px-5 py-8 text-center">
          <Loader2 size={24} class="mx-auto mb-2 text-slate-400 animate-spin" />
          <p class="text-sm text-slate-500">Loading health checks...</p>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .health-status {
    transition: box-shadow var(--duration-normal) var(--ease-default);
  }

  .health-status:hover {
    box-shadow: var(--shadow-md);
  }

  .health-check {
    transition: background-color var(--duration-fast) var(--ease-default);
  }
</style>
