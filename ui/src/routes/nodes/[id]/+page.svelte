<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import {
    Server,
    Cpu,
    MemoryStick,
    HardDrive,
    Activity,
    CheckCircle,
    AlertCircle,
    Clock,
    TrendingUp,
    ArrowLeft,
    RefreshCw
  } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import type { VM, Image, StoragePool, Network as NetworkType } from '$lib/api/types';
  import { getDefaultNode } from '$lib/api/nodes';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  const nodeId = $derived($page.params.id);

  // Resources
  let vms = $state<VM[]>([]);
  let images = $state<Image[]>([]);
  let pools = $state<StoragePool[]>([]);
  let networks = $state<NetworkType[]>([]);
  let loading = $state(true);
  let pollInterval: ReturnType<typeof setInterval> | null = $state(null);
  
  // Active tab
  let activeTab = $state<'vms' | 'images' | 'storage' | 'networks'>('vms');

  // Node info
  let node = $state<Node | null>(null);

  // Derived stats
  const runningVMs = $derived(vms.filter(v => v.actual_state === 'running').length);
  const totalStorageGB = $derived(
    pools.reduce((acc, p) => acc + (p.capacity_bytes || 0), 0) / (1024 ** 3)
  );
  const usedStorageGB = $derived(
    pools.reduce((acc, p) => acc + ((p.capacity_bytes || 0) - (p.allocatable_bytes || 0)), 0) / (1024 ** 3)
  );

  async function loadData() {
    try {
      // Fetch node details and resources for this node
      const [nodeData, vmsResponse, imagesResponse, storageResponse, networksResponse] = await Promise.all([
        client.getNode(nodeId),
        client.listNodeVMs(nodeId),
        client.listNodeImages(nodeId),
        client.listNodeStoragePools(nodeId),
        client.listNodeNetworks(nodeId)
      ]);
      node = nodeData;
      vms = vmsResponse.resources;
      images = imagesResponse.resources;
      pools = storageResponse.resources;
      networks = networksResponse.resources;
    } catch (e) {
      console.error('Failed to load node data:', e);
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

  onMount(() => {
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

  const tabs = [
    { id: 'vms', label: 'Virtual Machines', count: () => vms.length, icon: Server },
    { id: 'images', label: 'Images', count: () => images.length, icon: CheckCircle },
    { id: 'storage', label: 'Storage', count: () => pools.length, icon: HardDrive },
    { id: 'networks', label: 'Networks', count: () => networks.length, icon: Activity }
  ] as const;
</script>

<svelte:head>
  <title>{node?.name ?? 'Node'} | Node Details</title>
</svelte:head>

{#if loading}
  <div class="flex items-center justify-center h-96">
    <div class="flex items-center gap-3 text-slate-500">
      <RefreshCw class="animate-spin" size={24} />
      <span>Loading node information...</span>
    </div>
  </div>
{:else}
  <div class="space-y-6">
    <!-- Node Header -->
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-4">
        <div class="w-12 h-12 rounded-lg bg-gradient-to-br from-orange-500 to-red-600 flex items-center justify-center">
          <Server class="text-white" size={24} />
        </div>
        <div>
          <h1 class="text-2xl font-bold text-slate-900">{node?.name ?? 'Node'}</h1>
          <div class="flex items-center gap-3 mt-1">
            <span class="flex items-center gap-1.5 text-sm text-slate-500">
              <span class="w-2 h-2 rounded-full bg-green-500"></span>
              {node.status}
            </span>
            <span class="text-slate-300">|</span>
            <span class="text-sm text-slate-500">{node?.hostname ?? ''}</span>
            {#if node?.hostname}
              <span class="text-slate-300">|</span>
            {/if}
            <span class="text-sm text-slate-500">{node?.ip_address ?? ''}</span>
          </div>
        </div>
      </div>
      <button 
        onclick={loadData}
        class="flex items-center gap-2 px-4 py-2 text-sm font-medium text-slate-600 bg-white border border-slate-300 rounded-lg hover:bg-slate-50 transition-colors"
      >
        <RefreshCw size={16} />
        Refresh
      </button>
    </div>

    <!-- Node Stats Cards -->
    <div class="grid gap-4 lg:grid-cols-4">
      <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
        <div class="flex items-center gap-3">
          <div class="p-2 bg-blue-50 rounded-lg">
            <Cpu size={20} class="text-blue-600" />
          </div>
          <div>
            <p class="text-xs text-slate-500 uppercase">Virtual Machines</p>
            <p class="text-xl font-bold text-slate-900">{vms.length}</p>
          </div>
        </div>
        <div class="mt-3 flex items-center gap-2 text-sm">
          <span class="w-2 h-2 rounded-full bg-green-500"></span>
          <span class="text-slate-600">{runningVMs} running</span>
        </div>
      </div>

      <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
        <div class="flex items-center gap-3">
          <div class="p-2 bg-purple-50 rounded-lg">
            <CheckCircle size={20} class="text-purple-600" />
          </div>
          <div>
            <p class="text-xs text-slate-500 uppercase">Images</p>
            <p class="text-xl font-bold text-slate-900">{images.length}</p>
          </div>
        </div>
      </div>

      <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
        <div class="flex items-center gap-3">
          <div class="p-2 bg-amber-50 rounded-lg">
            <HardDrive size={20} class="text-amber-600" />
          </div>
          <div>
            <p class="text-xs text-slate-500 uppercase">Storage Pools</p>
            <p class="text-xl font-bold text-slate-900">{pools.length}</p>
          </div>
        </div>
        <div class="mt-3">
          <div class="w-full bg-slate-200 rounded-full h-1.5">
            <div 
              class="bg-amber-500 h-1.5 rounded-full" 
              style="width: {totalStorageGB > 0 ? (usedStorageGB / totalStorageGB * 100) : 0}%"
            ></div>
          </div>
          <p class="text-xs text-slate-500 mt-1">{usedStorageGB.toFixed(1)} / {totalStorageGB.toFixed(1)} GB</p>
        </div>
      </div>

      <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
        <div class="flex items-center gap-3">
          <div class="p-2 bg-green-50 rounded-lg">
            <Activity size={20} class="text-green-600" />
          </div>
          <div>
            <p class="text-xs text-slate-500 uppercase">Networks</p>
            <p class="text-xl font-bold text-slate-900">{networks.length}</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Tabs -->
    <div class="bg-white rounded-lg shadow-sm border border-slate-200">
      <div class="border-b border-slate-200">
        <nav class="flex -mb-px">
          {#each tabs as tab}
            {@const Icon = tab.icon}
            <button
              onclick={() => activeTab = tab.id}
              class="px-6 py-4 text-sm font-medium border-b-2 transition-colors flex items-center gap-2 {activeTab === tab.id 
                ? 'border-orange-500 text-orange-600' 
                : 'border-transparent text-slate-500 hover:text-slate-700 hover:border-slate-300'}"
            >
              <Icon size={16} />
              {tab.label}
              <span class="ml-2 px-2 py-0.5 text-xs rounded-full bg-slate-100 text-slate-600">
                {tab.count()}
              </span>
            </button>
          {/each}
        </nav>
      </div>

      <!-- Tab Content -->
      <div class="p-6">
        {#if activeTab === 'vms'}
          {#if vms.length === 0}
            <div class="text-center py-12 text-slate-500">
              <Server size={48} class="mx-auto mb-4 opacity-40" />
              <p class="text-lg font-medium">No virtual machines</p>
              <p class="text-sm mt-1">Create your first VM to get started</p>
            </div>
          {:else}
            <div class="table-container">
              <table>
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Status</th>
                    <th>vCPUs</th>
                    <th>Memory</th>
                    <th>IP Address</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {#each vms as vm}
                    <tr>
                      <td class="font-medium">{vm.name}</td>
                      <td><StateBadge label={vm.actual_state} /></td>
                      <td>{vm.vcpu}</td>
                      <td>{vm.memory_mb} MB</td>
                      <td>{vm.ip_address || '—'}</td>
                      <td>
                        <a href="/vms/{vm.id}" class="text-orange-600 hover:text-orange-700 text-sm font-medium">
                          View
                        </a>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        {:else if activeTab === 'images'}
          {#if images.length === 0}
            <div class="text-center py-12 text-slate-500">
              <CheckCircle size={48} class="mx-auto mb-4 opacity-40" />
              <p class="text-lg font-medium">No images</p>
              <p class="text-sm mt-1">Import images to use as VM templates</p>
            </div>
          {:else}
            <div class="table-container">
              <table>
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>OS Family</th>
                    <th>Architecture</th>
                    <th>Status</th>
                    <th>Cloud-Init</th>
                  </tr>
                </thead>
                <tbody>
                  {#each images as image}
                    <tr>
                      <td class="font-medium">{image.name}</td>
                      <td class="capitalize">{image.os_family}</td>
                      <td>{image.architecture}</td>
                      <td><StateBadge label={image.status} /></td>
                      <td>
                        {#if image.cloud_init_supported}
                          <span class="badge badge-success">Supported</span>
                        {:else}
                          <span class="badge badge-warning">Not Supported</span>
                        {/if}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        {:else if activeTab === 'storage'}
          {#if pools.length === 0}
            <div class="text-center py-12 text-slate-500">
              <HardDrive size={48} class="mx-auto mb-4 opacity-40" />
              <p class="text-lg font-medium">No storage pools</p>
              <p class="text-sm mt-1">Configure storage pools for VM disks</p>
            </div>
          {:else}
            <div class="table-container">
              <table>
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Type</th>
                    <th>Path</th>
                    <th>Capacity</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {#each pools as pool}
                    <tr>
                      <td class="font-medium">{pool.name}</td>
                      <td class="uppercase text-xs">{pool.pool_type}</td>
                      <td class="mono text-xs">{pool.path}</td>
                      <td>
                        {#if pool.capacity_bytes}
                          {(pool.capacity_bytes / (1024 ** 3)).toFixed(1)} GB
                        {:else}
                          —
                        {/if}
                      </td>
                      <td><StateBadge label={pool.status} /></td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        {:else if activeTab === 'networks'}
          {#if networks.length === 0}
            <div class="text-center py-12 text-slate-500">
              <Activity size={48} class="mx-auto mb-4 opacity-40" />
              <p class="text-lg font-medium">No networks</p>
              <p class="text-sm mt-1">Configure networks for VM connectivity</p>
            </div>
          {:else}
            <div class="table-container">
              <table>
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Mode</th>
                    <th>Bridge</th>
                    <th>CIDR</th>
                    <th>Gateway</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {#each networks as network}
                    <tr>
                      <td class="font-medium">{network.name}</td>
                      <td class="capitalize">{network.mode}</td>
                      <td>{network.bridge_name}</td>
                      <td>{network.cidr}</td>
                      <td>{network.gateway_ip}</td>
                      <td><StateBadge label={network.status} /></td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}
