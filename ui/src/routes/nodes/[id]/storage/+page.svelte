<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { HardDrive, ArrowLeft, Plus } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/DataTable.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import { useTable, formatBytes } from '$lib/utils/table.svelte';
  import { getDefaultNode } from '$lib/api/nodes';
  import type { StoragePool } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  const nodeId = $derived($page.params.id);
  const node = $derived(getDefaultNode());
  
  let items: StoragePool[] = $state([]);
  let loading = $state(true);

  // Table state management
  let table = useTable<StoragePool>({
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
      render: (pool: StoragePool) => pool.name
    },
    {
      key: 'pool_type',
      title: 'Type',
      sortable: true,
      width: '100px',
      render: (pool: StoragePool) => pool.pool_type.toUpperCase()
    },
    {
      key: 'path',
      title: 'Path',
      render: (pool: StoragePool) => pool.path
    },
    {
      key: 'capacity_bytes',
      title: 'Capacity',
      sortable: true,
      width: '120px',
      render: (pool: StoragePool) => pool.capacity_bytes ? formatBytes(pool.capacity_bytes) : '—'
    },
    {
      key: 'allocatable_bytes',
      title: 'Available',
      sortable: true,
      width: '120px',
      render: (pool: StoragePool) => pool.allocatable_bytes ? formatBytes(pool.allocatable_bytes) : '—'
    },
    {
      key: 'status',
      title: 'Status',
      sortable: true,
      width: '100px',
      render: (pool: StoragePool) => pool.status
    }
  ];

  async function loadData() {
    loading = true;
    try {
      // Use node-scoped API to get storage pools for this node
      const response = await client.listNodeStoragePools(nodeId);
      items = response.resources;
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to load storage pools');
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

  // Calculate totals
  const totalCapacity = $derived(items.reduce((acc, p) => acc + (p.capacity_bytes || 0), 0));
  const totalAvailable = $derived(items.reduce((acc, p) => acc + (p.allocatable_bytes || 0), 0));
</script>

<svelte:head>
  <title>Storage | {node.name}</title>
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
        <h1 class="text-2xl font-bold text-slate-900">Storage Pools</h1>
        <p class="text-sm text-slate-500">Node: {node.name}</p>
      </div>
    </div>
  </div>

  <!-- Stats -->
  <div class="grid gap-4 md:grid-cols-4">
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-amber-50 rounded-lg">
          <HardDrive size={20} class="text-amber-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Total Pools</p>
          <p class="text-xl font-bold text-slate-900">{items.length}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-blue-50 rounded-lg">
          <HardDrive size={20} class="text-blue-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Total Capacity</p>
          <p class="text-xl font-bold text-slate-900">{formatBytes(totalCapacity)}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-green-50 rounded-lg">
          <HardDrive size={20} class="text-green-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Available</p>
          <p class="text-xl font-bold text-slate-900">{formatBytes(totalAvailable)}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-purple-50 rounded-lg">
          <HardDrive size={20} class="text-purple-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Used</p>
          <p class="text-xl font-bold text-slate-900">
            {totalCapacity > 0 ? ((totalCapacity - totalAvailable) / totalCapacity * 100).toFixed(1) : 0}%
          </p>
        </div>
      </div>
    </div>
  </div>

  <!-- Storage Table -->
  <div class="bg-white rounded-lg shadow-sm border border-slate-200">
    <DataTable
      data={table.paginatedData}
      {columns}
      {loading}
      sortColumn={table.sortColumn}
      sortDirection={table.sortDirection}
      onSort={handleSort}
      rowId={pool => pool.id}
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
