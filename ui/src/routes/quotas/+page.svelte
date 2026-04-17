<script lang="ts">
  import { onMount } from 'svelte';
  import { Cpu, HardDrive, Network, Server, Settings, AlertTriangle } from 'lucide-svelte';
  import { createAPIClient, APIError } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import QuotaSettingsModal from '$lib/components/modals/QuotaSettingsModal.svelte';
  import type { UsageWithQuota, Quota, UserInfo } from '$lib/api/types';

  // State
  let usageData = $state<UsageWithQuota | null>(null);
  let allQuotas = $state<Quota[]>([]);
  let users = $state<UserInfo[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let showSettingsModal = $state(false);
  let editingQuota = $state<Quota | null>(null);
  let isAdmin = $state(false); // TODO: Get from auth context

  const client = createAPIClient();

  // Resource configuration
  const resources = [
    { key: 'vms', label: 'Virtual Machines', icon: Server, unit: '', color: 'bg-blue-500' },
    { key: 'cpus', label: 'CPU Cores', icon: Cpu, unit: '', color: 'bg-green-500' },
    { key: 'memory_gb', label: 'Memory', icon: HardDrive, unit: 'GB', color: 'bg-purple-500' },
    { key: 'storage_gb', label: 'Storage', icon: HardDrive, unit: 'GB', color: 'bg-orange-500' },
    { key: 'networks', label: 'Networks', icon: Network, unit: '', color: 'bg-pink-500' },
  ] as const;

  type ResourceKey = typeof resources[number]['key'];

  async function loadQuotaData() {
    loading = true;
    error = null;

    try {
      usageData = await client.getUsage();
    } catch (err) {
      console.error('Failed to load quota data:', err);
      if (err instanceof APIError) {
        error = err.message;
      } else {
        error = 'Failed to load quota data. Please try again.';
      }
      toast.error(error);
    } finally {
      loading = false;
    }
  }

  async function loadAllQuotas() {
    try {
      allQuotas = await client.listQuotas();
    } catch (err) {
      console.error('Failed to load quotas:', err);
    }
  }

  function handleEditQuota() {
    if (usageData) {
      editingQuota = usageData.quota;
      showSettingsModal = true;
    }
  }

  function handleModalSuccess() {
    loadQuotaData();
    loadAllQuotas();
  }

  function getUsageValue(key: ResourceKey): number {
    if (!usageData) return 0;
    switch (key) {
      case 'vms': return usageData.usage.vms;
      case 'cpus': return usageData.usage.cpus;
      case 'memory_gb': return usageData.usage.memory_gb;
      case 'storage_gb': return usageData.usage.storage_gb;
      case 'networks': return usageData.usage.networks;
      default: return 0;
    }
  }

  function getQuotaValue(key: ResourceKey): number {
    if (!usageData) return 0;
    switch (key) {
      case 'vms': return usageData.quota.max_vms;
      case 'cpus': return usageData.quota.max_cpu;
      case 'memory_gb': return usageData.quota.max_memory_gb;
      case 'storage_gb': return usageData.quota.max_storage_gb;
      case 'networks': return usageData.quota.max_networks;
      default: return 0;
    }
  }

  function getPercentage(key: ResourceKey): number {
    const used = getUsageValue(key);
    const max = getQuotaValue(key);
    if (max === 0) return 0;
    return Math.min(100, Math.round((used / max) * 100));
  }

  function formatValue(key: ResourceKey, value: number): string {
    const unit = resources.find(r => r.key === key)?.unit || '';
    return `${value}${unit ? ' ' + unit : ''}`;
  }

  function getStatusColor(percentage: number): string {
    if (percentage >= 90) return 'text-red-500';
    if (percentage >= 75) return 'text-amber-500';
    return 'text-green-500';
  }

  function getProgressColor(percentage: number): string {
    if (percentage >= 90) return 'bg-red-500';
    if (percentage >= 75) return 'bg-amber-500';
    return 'bg-green-500';
  }

  onMount(() => {
    loadQuotaData();
  });
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-bold text-slate-900">Resource Quotas</h1>
      <p class="text-sm text-slate-500 mt-1">
        Monitor your resource usage and limits
      </p>
    </div>
    <div class="flex items-center gap-2">
      <button
        onclick={handleEditQuota}
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
      >
        <Settings size={18} />
        Adjust Quota
      </button>
      <button
        onclick={loadQuotaData}
        disabled={loading}
        class="px-4 py-2 bg-white border border-slate-200 text-slate-700 rounded-lg hover:bg-slate-50 transition-colors disabled:opacity-50 flex items-center gap-2"
      >
        <span class:hidden={!loading} class="animate-spin">↻</span>
        Refresh
      </button>
    </div>
  </div>

  <!-- Loading State -->
  {#if loading}
    <div class="flex items-center justify-center py-12">
      <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
      <span class="ml-3 text-slate-500">Loading quota data...</span>
    </div>
  {:else if error}
    <!-- Error State -->
    <div class="bg-red-50 border border-red-200 rounded-lg p-6 text-center">
      <div class="text-red-500 text-lg font-medium mb-2">Failed to Load</div>
      <p class="text-red-600 text-sm mb-4">{error}</p>
      <button
        onclick={loadQuotaData}
        class="px-4 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 transition-colors"
      >
        Try Again
      </button>
    </div>
  {:else if usageData}
    <!-- Quota Status Banner -->
    {@const criticalResources = resources.filter(r => getPercentage(r.key) >= 95)}
    {@const warningResources = resources.filter(r => {
      const p = getPercentage(r.key);
      return p >= 80 && p < 95;
    })}
    
    {#if criticalResources.length > 0}
      <div class="bg-red-50 border border-red-200 rounded-lg p-4 flex items-start gap-3">
        <AlertTriangle class="text-red-500 mt-0.5" size={20} />
        <div>
          <h3 class="text-sm font-medium text-red-800">Critical Usage Alert</h3>
          <p class="text-sm text-red-700 mt-1">
            The following resources are at critical usage levels (≥95%):
            {criticalResources.map(r => `${r.label} (${getPercentage(r.key)}%)`).join(', ')}
          </p>
        </div>
      </div>
    {:else if warningResources.length > 0}
      <div class="bg-amber-50 border border-amber-200 rounded-lg p-4 flex items-start gap-3">
        <AlertTriangle class="text-amber-500 mt-0.5" size={20} />
        <div>
          <h3 class="text-sm font-medium text-amber-800">Usage Warning</h3>
          <p class="text-sm text-amber-700 mt-1">
            The following resources are approaching limits (≥80%):
            {warningResources.map(r => `${r.label} (${getPercentage(r.key)}%)`).join(', ')}
          </p>
        </div>
      </div>
    {/if}

    <!-- Overview Cards -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
      {#each resources as resource}
        {@const Icon = resource.icon}
        {@const used = getUsageValue(resource.key)}
        {@const max = getQuotaValue(resource.key)}
        {@const percentage = getPercentage(resource.key)}
        
        <div class="bg-white rounded-lg border border-slate-200 p-5 hover:shadow-md transition-shadow">
          <div class="flex items-start justify-between">
            <div class="flex items-center gap-3">
              <div class="p-2 rounded-lg {resource.color} bg-opacity-10">
                <Icon size={20} class="{resource.color.replace('bg-', 'text-')}" />
              </div>
              <div>
                <h3 class="text-sm font-medium text-slate-600">{resource.label}</h3>
                <div class="flex items-baseline gap-1 mt-1">
                  <span class="text-2xl font-bold text-slate-900">
                    {used}
                  </span>
                  <span class="text-sm text-slate-500">
                    / {formatValue(resource.key, max)}
                  </span>
                </div>
              </div>
            </div>
            <div class="text-right">
              <span class="text-sm font-medium {getStatusColor(percentage)}">
                {percentage}%
              </span>
            </div>
          </div>

          <!-- Progress Bar -->
          <div class="mt-4">
            <div class="h-2 bg-slate-100 rounded-full overflow-hidden">
              <div
                class="h-full {getProgressColor(percentage)} transition-all duration-500"
                style="width: {percentage}%"
              ></div>
            </div>
            <p class="text-xs text-slate-500 mt-2">
              {formatValue(resource.key, max - used)} available
            </p>
          </div>
        </div>
      {/each}
    </div>

    <!-- Summary Section -->
    <div class="bg-white rounded-lg border border-slate-200 p-6">
      <h2 class="text-lg font-semibold text-slate-900 mb-4">Quota Summary</h2>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
        <!-- Quota Details -->
        <div>
          <h3 class="text-sm font-medium text-slate-600 mb-3">Current Limits</h3>
          <dl class="space-y-2">
            {#each resources as resource}
              <div class="flex justify-between text-sm">
                <dt class="text-slate-500">{resource.label}</dt>
                <dd class="font-medium text-slate-900">
                  {formatValue(resource.key, getQuotaValue(resource.key))}
                </dd>
              </div>
            {/each}
          </dl>
        </div>

        <!-- Usage Details -->
        <div>
          <h3 class="text-sm font-medium text-slate-600 mb-3">Current Usage</h3>
          <dl class="space-y-2">
            {#each resources as resource}
              {@const percentage = getPercentage(resource.key)}
              <div class="flex justify-between text-sm">
                <dt class="text-slate-500">{resource.label}</dt>
                <dd class="font-medium {getStatusColor(percentage)}">
                  {formatValue(resource.key, getUsageValue(resource.key))}
                  ({percentage}%)
                </dd>
              </div>
            {/each}
          </dl>
        </div>
      </div>
    </div>

    <!-- Alerts Section -->
    {@const highUsageResources = resources.filter(r => getPercentage(r.key) >= 90)}
    {#if highUsageResources.length > 0}
      <div class="bg-red-50 border border-red-200 rounded-lg p-4">
        <h3 class="text-sm font-medium text-red-800 mb-2">⚠️ High Usage Alerts</h3>
        <ul class="space-y-1">
          {#each highUsageResources as resource}
            <li class="text-sm text-red-700">
              {resource.label} is at {getPercentage(resource.key)}% capacity
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    {@const warningResources2 = resources.filter(r => {
      const p = getPercentage(r.key);
      return p >= 75 && p < 90;
    })}
    {#if warningResources2.length > 0}
      <div class="bg-amber-50 border border-amber-200 rounded-lg p-4">
        <h3 class="text-sm font-medium text-amber-800 mb-2">⚡ Usage Warnings</h3>
        <ul class="space-y-1">
          {#each warningResources2 as resource}
            <li class="text-sm text-amber-700">
              {resource.label} is at {getPercentage(resource.key)}% capacity
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  {/if}
</div>

<!-- Quota Settings Modal -->
<QuotaSettingsModal
  bind:open={showSettingsModal}
  quota={editingQuota}
  {users}
  onSuccess={handleModalSuccess}
/>
