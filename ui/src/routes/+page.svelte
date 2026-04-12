<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { 
    Server, 
    Image as ImageIcon, 
    HardDrive, 
    Network,
    Cpu,
    Database,
    Plus,
    Download,
    Settings,
    Activity,
    Loader2,
    RefreshCw
  } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { getDefaultNode } from '$lib/api/nodes';
  import { toast } from '$lib/stores/toast';
  import Button from '$lib/components/primitives/Button.svelte';
  import ResourceCard from '$lib/components/ResourceCard.svelte';
  import HealthStatus from '$lib/components/HealthStatus.svelte';
  import EventList from '$lib/components/EventList.svelte';
  import CreateVMModal from '$lib/components/CreateVMModal.svelte';
  import ImportImageModal from '$lib/components/ImportImageModal.svelte';
  import CreateNetworkModal from '$lib/components/CreateNetworkModal.svelte';
  import type { VM, Image, StoragePool, Network as NetworkType, Event, NodeWithResources } from '$lib/api/types';
  import type { HealthCheck } from '$lib/components/HealthStatus.svelte';

  const token = getStoredToken();
  let client: ReturnType<typeof createAPIClient>;

  // State
  let vms = $state<VM[]>([]);
  let images = $state<Image[]>([]);
  let pools = $state<StoragePool[]>([]);
  let networks = $state<NetworkType[]>([]);
  let events = $state<Event[]>([]);
  let nodes = $state<NodeWithResources[]>([]);
  let installState = $state<string>('unknown');
  let loading = $state(true);
  let scanning = $state(false);
  let lastUpdated = $state<Date>(new Date());
  let pollInterval: ReturnType<typeof setInterval> | null = $state(null);

  // Modal states
  let showCreateVM = $state(false);
  let showImportImage = $state(false);
  let showCreateNetwork = $state(false);

  // Permission state (placeholder - would come from auth context)
  let permissions = $state({
    canCreateVM: true,
    canImportImage: true,
    canCreateNetwork: true,
    canManageStorage: true
  });

  // Derived stats
  const runningVMs = $derived(vms.filter(v => v.actual_state === 'running').length);
  const stoppedVMs = $derived(vms.filter(v => v.actual_state === 'stopped').length);
  const totalVcpus = $derived(vms.reduce((acc, v) => acc + (v.vcpu || 0), 0));
  const totalMemoryGB = $derived(vms.reduce((acc, v) => acc + ((v.memory_mb || 0) / 1024), 0));
  
  const totalStorageGB = $derived(
    pools.reduce((acc, p) => acc + (p.capacity_bytes || 0), 0) / (1024 ** 3)
  );
  const usedStorageGB = $derived(
    pools.reduce((acc, p) => acc + ((p.capacity_bytes || 0) - (p.allocatable_bytes || 0)), 0) / (1024 ** 3)
  );

  // Mock historical data for sparklines (would come from metrics API)
  const vmHistory = $derived([vms.length * 0.8, vms.length * 0.85, vms.length * 0.9, vms.length * 0.88, vms.length * 0.92, vms.length * 0.95, vms.length]);
  const storageHistory = $derived([usedStorageGB * 0.7, usedStorageGB * 0.75, usedStorageGB * 0.8, usedStorageGB * 0.78, usedStorageGB * 0.85, usedStorageGB * 0.9, usedStorageGB]);

  // Current node info
  const currentNode = $derived(nodes.length > 0 ? nodes[0] : getDefaultNode());

  // Health checks derived from data
  const healthChecks = $derived<HealthCheck[]>([
    {
      id: 'api',
      name: 'API Status',
      status: loading ? 'pending' : 'healthy',
      message: loading ? 'Checking...' : 'Responding normally',
      lastChecked: lastUpdated.toISOString()
    },
    {
      id: 'node',
      name: 'Node Status',
      status: currentNode?.status === 'online' ? 'healthy' : 'warning',
      message: currentNode?.status === 'online' ? `${currentNode.name} online` : 'Node unavailable',
      lastChecked: lastUpdated.toISOString()
    },
    {
      id: 'storage',
      name: 'Storage Health',
      status: totalStorageGB > 0 ? 'healthy' : 'warning',
      message: pools.length > 0 ? `${pools.length} pools active` : 'No storage pools',
      details: pools.map(p => `${p.name}: ${((p.capacity_bytes || 0) / (1024**3)).toFixed(1)} GB`),
      lastChecked: lastUpdated.toISOString()
    },
    {
      id: 'platform',
      name: 'Platform',
      status: installState === 'ready' ? 'healthy' : installState === 'bootstrap_required' ? 'warning' : 'pending',
      message: installState.replace('_', ' '),
      lastChecked: lastUpdated.toISOString()
    }
  ]);

  async function loadData() {
    if (!client) return;
    try {
      const [vmsData, imagesData, poolsData, networksData, eventsData, installData, nodesData] = await Promise.all([
        client.listVMs(),
        client.listImages(),
        client.listStoragePools(),
        client.listNetworks(),
        client.listEvents(),
        client.getInstallStatus(),
        client.listNodes()
      ]);
      vms = vmsData ?? [];
      images = imagesData ?? [];
      pools = poolsData ?? [];
      networks = networksData ?? [];
      events = eventsData ?? [];
      installState = installData.overall_state;
      nodes = nodesData ?? [];
      lastUpdated = new Date();
    } catch (e) {
      console.error('Failed to load dashboard data:', e);
      toast.error('Failed to load dashboard data');
    } finally {
      loading = false;
    }
  }

  function startPolling() {
    pollInterval = setInterval(loadData, 10000);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  function handleRefresh() {
    loading = true;
    loadData();
  }

  function handleFilterChange(filter: string | null) {
    // Filter is handled by EventList component internally
    console.log('Filter changed:', filter);
  }

  function handleClearEvents() {
    // In a real app, this would call an API to clear events
    toast.info('Clear events functionality would be implemented here');
  }

  function handleMarkAllRead() {
    toast.info('Mark all as read functionality would be implemented here');
  }

  function handleVMCreated() {
    showCreateVM = false;
    loadData();
    toast.success('VM created successfully');
  }

  function handleImageImported() {
    showImportImage = false;
    loadData();
    toast.success('Image import started');
  }

  function handleNetworkCreated() {
    showCreateNetwork = false;
    loadData();
    toast.success('Network created successfully');
  }

  async function handleScanNode() {
    if (!client || !currentNode || currentNode.id === 'placeholder') return;
    
    scanning = true;
    try {
      const result = await client.discoverNode(currentNode.id);
      if (result.count > 0) {
        toast.success(`Discovered ${result.count} new VMs`);
        loadData();
      } else {
        toast.info('No new VMs discovered');
      }
    } catch (e) {
      console.error('Scan failed:', e);
      toast.error('Failed to scan node for existing VMs');
    } finally {
      scanning = false;
    }
  }

  onMount(() => {
    client = createAPIClient();
    if (!token) {
      goto('/login');
      return;
    }
    loadData();
    startPolling();
  });

  onDestroy(() => {
    stopPolling();
  });
</script>

<svelte:head>
  <title>Dashboard | CHV</title>
</svelte:head>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
    <div>
      <h1 class="text-2xl font-bold text-slate-900">Dashboard</h1>
      <p class="text-sm text-slate-500 mt-1">
        {currentNode?.name || 'Datacenter'} overview and system status
      </p>
    </div>
    <div class="flex items-center gap-2">
      <span class="text-xs text-slate-500">
        Last updated: {lastUpdated.toLocaleTimeString()}
      </span>
      <button
        onclick={handleRefresh}
        class="p-2 text-slate-400 hover:text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
        aria-label="Refresh data"
        disabled={loading}
      >
        <RefreshCw size={16} class={loading ? 'animate-spin' : ''} />
      </button>
    </div>
  </div>

  {#if loading && vms.length === 0}
    <!-- Initial Loading State -->
    <div class="flex items-center justify-center h-96">
      <div class="flex items-center gap-3 text-slate-500">
        <Loader2 class="animate-spin" size={24} />
        <span>Loading dashboard...</span>
      </div>
    </div>
  {:else}
    <!-- Resource Overview Cards -->
    <section class="grid gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-4">
      <!-- Node Card -->
      <ResourceCard
        title="Node"
        value={currentNode?.name || 'Unknown'}
        subtitle={currentNode?.hostname || ''}
        icon={Server}
        iconColor="slate"
        loading={loading && vms.length === 0}
        href="/nodes"
      />

      <!-- VMs Card -->
      <ResourceCard
        title="Virtual Machines"
        value={vms.length}
        subtitle={runningVMs + ' running, ' + stoppedVMs + ' stopped'}
        icon={Cpu}
        iconColor="blue"
        trend={vms.length > 0 ? 'up' : 'neutral'}
        trendValue={vms.length > 0 ? '+1 this week' : undefined}
        sparklineData={vmHistory}
        loading={loading && vms.length === 0}
        href="/vms"
      />

      <!-- Storage Card -->
      <ResourceCard
        title="Storage"
        value={pools.length}
        subtitle={usedStorageGB.toFixed(1) + ' GB of ' + totalStorageGB.toFixed(1) + ' GB used'}
        icon={Database}
        iconColor="amber"
        progress={totalStorageGB > 0 ? {
          value: usedStorageGB,
          max: totalStorageGB,
          label: 'Usage'
        } : undefined}
        sparklineData={storageHistory}
        loading={loading && vms.length === 0}
        href="/storage"
      />

      <!-- Resources Summary Card -->
      <ResourceCard
        title="Resources"
        value={images.length + networks.length}
        subtitle={images.length + ' images, ' + networks.length + ' networks'}
        icon={Activity}
        iconColor="purple"
        loading={loading && vms.length === 0}
        href="/resources"
      />
    </section>

    <!-- Quick Actions -->
    <section class="bg-white rounded-lg border border-slate-200 p-4">
      <h3 class="text-sm font-medium text-slate-700 mb-3">Quick Actions</h3>
      <div class="flex flex-wrap gap-2">
        <Button
          variant="primary"
          size="sm"
          onclick={() => showCreateVM = true}
          disabled={!permissions.canCreateVM}
        >
          <Plus size={16} />
          Create VM
        </Button>
        <Button
          variant="secondary"
          size="sm"
          onclick={handleScanNode}
          disabled={scanning || currentNode.id === 'placeholder'}
        >
          <Loader2 size={16} class={scanning ? 'animate-spin' : 'hidden'} />
          <RefreshCw size={16} class={scanning ? 'hidden' : ''} />
          Scan Node for VMs
        </Button>
        <Button
          variant="secondary"
          size="sm"
          onclick={() => showImportImage = true}
          disabled={!permissions.canImportImage}
        >
          <Download size={16} />
          Import Image
        </Button>
        <Button
          variant="secondary"
          size="sm"
          onclick={() => showCreateNetwork = true}
          disabled={!permissions.canCreateNetwork}
        >
          <Network size={16} />
          Add Network
        </Button>
        <Button
          variant="secondary"
          size="sm"
          onclick={() => goto('/storage/pools/create')}
          disabled={!permissions.canManageStorage}
        >
          <HardDrive size={16} />
          Add Storage
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onclick={() => goto('/install')}
        >
          <Settings size={16} />
          Platform Settings
        </Button>
      </div>
    </section>

    <!-- Main Content Grid -->
    <div class="grid gap-6 lg:grid-cols-3">
      <!-- Events Section (2/3 width) -->
      <div class="lg:col-span-2">
        <EventList
          {events}
          {loading}
          onFilter={handleFilterChange}
          onClear={handleClearEvents}
          onMarkAllRead={handleMarkAllRead}
        />
      </div>

      <!-- Side Widgets (1/3 width) -->
      <div class="space-y-6">
        <!-- Health Status -->
        <HealthStatus
          checks={healthChecks}
          {loading}
          onRefresh={handleRefresh}
          {lastUpdated}
        />

        <!-- Resource Usage Widget -->
        <div class="card">
          <div class="px-5 py-4 border-b border-slate-100">
            <h3 class="font-semibold text-slate-900">Resource Usage</h3>
            <p class="text-xs text-slate-500 mt-0.5">Across all VMs</p>
          </div>
          <div class="p-5 space-y-4">
            <!-- CPU Usage -->
            <div>
              <div class="flex items-center justify-between text-sm mb-1.5">
                <span class="text-slate-600 flex items-center gap-2">
                  <Cpu size={14} class="text-slate-400" />
                  CPU Cores
                </span>
                <span class="font-medium text-slate-700">{totalVcpus}</span>
              </div>
              <div class="w-full bg-slate-100 rounded-full h-2">
                <div
                  class="h-2 rounded-full bg-gradient-to-r from-blue-500 to-blue-600 transition-all duration-500"
                  style="width: {Math.min(100, (totalVcpus / 32) * 100)}%"
                ></div>
              </div>
              <p class="text-xs text-slate-400 mt-1">
                {totalVcpus} of 32 vCPUs allocated
              </p>
            </div>

            <!-- Memory Usage -->
            <div>
              <div class="flex items-center justify-between text-sm mb-1.5">
                <span class="text-slate-600 flex items-center gap-2">
                  <Activity size={14} class="text-slate-400" />
                  Memory
                </span>
                <span class="font-medium text-slate-700">{totalMemoryGB.toFixed(1)} GB</span>
              </div>
              <div class="w-full bg-slate-100 rounded-full h-2">
                <div
                  class="h-2 rounded-full bg-gradient-to-r from-purple-500 to-purple-600 transition-all duration-500"
                  style="width: {Math.min(100, (totalMemoryGB / 64) * 100)}%"
                ></div>
              </div>
              <p class="text-xs text-slate-400 mt-1">
                {totalMemoryGB.toFixed(1)} of 64 GB allocated
              </p>
            </div>

            <!-- Storage by Pool -->
            {#if pools.length > 0}
              <div class="pt-2 border-t border-slate-100">
                <span class="text-xs font-medium text-slate-500 uppercase tracking-wider">Storage by Pool</span>
                <div class="mt-2 space-y-2">
                  {#each pools.slice(0, 3) as pool}
                    {@const used = ((pool.capacity_bytes || 0) - (pool.allocatable_bytes || 0)) / (1024**3)}
                    {@const total = (pool.capacity_bytes || 0) / (1024**3)}
                    {@const pct = total > 0 ? (used / total) * 100 : 0}
                    <div>
                      <div class="flex items-center justify-between text-xs mb-1">
                        <span class="text-slate-600 truncate max-w-[120px]">{pool.name}</span>
                        <span class="text-slate-500">{pct.toFixed(0)}%</span>
                      </div>
                      <div class="w-full bg-slate-100 rounded-full h-1.5">
                        <div
                          class="h-1.5 rounded-full bg-gradient-to-r from-amber-500 to-orange-500 transition-all duration-500"
                          style="width: {pct}%"
                        ></div>
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<!-- Modals - using bind:open pattern -->
<CreateVMModal 
  bind:open={showCreateVM}
  onSuccess={handleVMCreated}
  {images}
  {pools}
  {networks}
/>

<ImportImageModal
  bind:open={showImportImage}
  onSuccess={handleImageImported}
/>

<CreateNetworkModal
  bind:open={showCreateNetwork}
  onSuccess={handleNetworkCreated}
/>
