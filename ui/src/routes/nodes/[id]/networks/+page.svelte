<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { Network, ArrowLeft, Plus } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/data-display/DataTable.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import CreateNetworkModal from '$lib/components/modals/CreateNetworkModal.svelte';
  import { useTable } from '$lib/utils/table.svelte';
  import { getDefaultNode } from '$lib/api/nodes';
  import type { Network as NetworkType } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  const nodeId = $derived($page.params.id);
  const node = $derived(getDefaultNode());
  
  let items: NetworkType[] = $state([]);
  let loading = $state(true);
  let createModalOpen = $state(false);

  // Table state management
  let table = useTable<NetworkType>({
    data: [],
    pageSize: 10
  });

  $effect(() => {
    table.data = items;
  });

  // Table columns definition
  const columns = [
    {
      key: 'name',
      title: 'Name',
      sortable: true,
      render: (net: NetworkType) => net.name
    },
    {
      key: 'mode',
      title: 'Mode',
      sortable: true,
      width: '100px',
      render: (net: NetworkType) => net.mode
    },
    {
      key: 'bridge_name',
      title: 'Bridge',
      width: '120px',
      render: (net: NetworkType) => net.bridge_name
    },
    {
      key: 'cidr',
      title: 'CIDR',
      width: '140px',
      render: (net: NetworkType) => net.cidr
    },
    {
      key: 'gateway_ip',
      title: 'Gateway',
      width: '130px',
      render: (net: NetworkType) => net.gateway_ip || '—'
    },
    {
      key: 'status',
      title: 'Status',
      sortable: true,
      width: '100px',
      render: (net: NetworkType) => net.status
    }
  ];

  async function loadData() {
    loading = true;
    try {
      // Use node-scoped API to get networks for this node
      const response = await client.listNodeNetworks(nodeId);
      items = response.resources;
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to load networks');
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (!token) {
      goto('/login');
      return;
    }
    loadData();
  });

  function handleSort(column: string, direction: 'asc' | 'desc' | null) {
    if (direction) {
      table.setSort(column, direction);
    } else {
      table.clearSort();
    }
  }
</script>

<svelte:head>
  <title>Networks | {node.name}</title>
</svelte:head>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-4">
      <a 
        href={`/nodes/${nodeId}`} 
        class="p-2 text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors"
        aria-label="Back to node"
      >
        <ArrowLeft size={20} />
      </a>
      <div>
        <h1 class="text-2xl font-bold text-slate-900">Networks</h1>
        <p class="text-sm text-slate-500">Node: {node.name}</p>
      </div>
    </div>
    <button
      onclick={() => createModalOpen = true}
      class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 hover:bg-orange-600 text-white text-sm font-medium rounded-lg transition-colors"
    >
      <Plus size={16} />
      Create Network
    </button>
  </div>

  <!-- Stats -->
  <div class="grid gap-4 md:grid-cols-4">
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-green-50 rounded-lg">
          <Network size={20} class="text-green-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Total Networks</p>
          <p class="text-xl font-bold text-slate-900">{items.length}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-blue-50 rounded-lg">
          <Network size={20} class="text-blue-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Bridge Mode</p>
          <p class="text-xl font-bold text-slate-900">{items.filter(n => n.mode === 'bridge').length}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-purple-50 rounded-lg">
          <Network size={20} class="text-purple-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">NAT Mode</p>
          <p class="text-xl font-bold text-slate-900">{items.filter(n => n.mode === 'nat').length}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-green-50 rounded-lg">
          <Network size={20} class="text-green-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Active</p>
          <p class="text-xl font-bold text-slate-900">{items.filter(n => n.status === 'active').length}</p>
        </div>
      </div>
    </div>
  </div>

  <!-- Networks Table -->
  <div class="bg-white rounded-lg shadow-sm border border-slate-200">
    <DataTable
      data={table.paginatedData}
      {columns}
      {loading}
      sortColumn={table.sortColumn}
      sortDirection={table.sortDirection}
      onSort={handleSort}
      rowId={net => net.id}
    />
    
    <Pagination
      page={table.page}
      pageSize={table.pageSize}
      totalItems={table.totalItems}
      onPageChange={p => table.setPage(p)}
      onPageSizeChange={size => table.setPageSize(size)}
    />
  </div>
</div>

<CreateNetworkModal
  bind:open={createModalOpen}
  onSuccess={loadData}
/>
