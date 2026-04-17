<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { Server, ArrowLeft, Plus } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/data-display/DataTable.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import FilterBar from '$lib/components/FilterBar.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import CreateVMModal from '$lib/components/modals/CreateVMModal.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { useTable, formatBytes } from '$lib/utils/table.svelte';
  import type { VM, Image, StoragePool, Network, Node } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  const nodeId = $derived($page.params.id);
  
  let node = $state<Node | null>(null);
  
  let items: VM[] = $state([]);
  let loading = $state(true);
  let error = $state('');
  let createModalOpen = $state(false);
  let images = $state<Image[]>([]);
  let pools = $state<StoragePool[]>([]);
  let networks = $state<Network[]>([]);
  
  // Confirm dialog state
  let confirmDialog = $state<{
    open: boolean;
    title: string;
    description: string;
    action: () => Promise<void>;
  }>({
    open: false,
    title: '',
    description: '',
    action: async () => {}
  });

  // Table state management
  let table = useTable<VM>({
    data: [],
    pageSize: 10
  });

  $effect(() => {
    table.data = items;
  });

  // Lookup functions for related data
  function getImage(id: string) { return images.find(i => i.id === id); }
  function getPool(id: string) { return pools.find(p => p.id === id); }
  function getNetwork(id: string) { return networks.find(n => n.id === id); }

  // Filter options
  const filterOptions = [
    {
      key: 'actual_state',
      label: 'State',
      type: 'select' as const,
      options: [
        { value: 'running', label: 'Running' },
        { value: 'stopped', label: 'Stopped' },
        { value: 'creating', label: 'Creating' },
        { value: 'error', label: 'Error' }
      ]
    }
  ];

  // Table columns definition
  const columns = [
    {
      key: 'name',
      title: 'Name',
      sortable: true,
      render: (vm: VM) => vm.name
    },
    {
      key: 'actual_state',
      title: 'State',
      sortable: true,
      width: '140px',
      render: (vm: VM) => {
        if (vm.desired_state === vm.actual_state) {
          return vm.actual_state;
        }
        return `${vm.actual_state} → ${vm.desired_state}`;
      }
    },
    {
      key: 'image_id',
      title: 'Image',
      render: (vm: VM) => {
        const img = getImage(vm.image_id);
        return img?.name ?? vm.image_id;
      }
    },
    {
      key: 'storage_pool_id',
      title: 'Pool',
      render: (vm: VM) => {
        const pool = getPool(vm.storage_pool_id);
        return pool?.name ?? vm.storage_pool_id;
      }
    },
    {
      key: 'vcpu',
      title: 'vCPU',
      sortable: true,
      align: 'center' as const,
      width: '80px'
    },
    {
      key: 'memory_mb',
      title: 'Memory',
      sortable: true,
      align: 'right' as const,
      width: '100px',
      render: (vm: VM) => `${vm.memory_mb} MB`
    },
    {
      key: 'ip_address',
      title: 'IP Address',
      width: '130px',
      render: (vm: VM) => vm.ip_address || '—'
    }
  ];

  async function loadData() {
    loading = true;
    error = '';
    try {
      // Fetch node details and VMs for this node
      const [nodeData, vmsResponse, imgs, ps, nets] = await Promise.all([
        client.getNode(nodeId),
        client.listNodeVMs(nodeId),
        client.listImages(),
        client.listStoragePools(),
        client.listNetworks()
      ]);
      node = nodeData;
      items = vmsResponse.resources;
      images = imgs;
      pools = ps;
      networks = nets;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load VMs';
      toast.error(error);
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

  async function startVM(vm: VM) {
    try {
      await client.startVM(vm.id);
      toast.success(`VM "${vm.name}" started`);
      loadData();
    } catch (err) {
      toast.error(`Failed to start VM: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  }

  async function stopVM(vm: VM) {
    try {
      await client.stopVM(vm.id);
      toast.success(`VM "${vm.name}" stopped`);
      loadData();
    } catch (err) {
      toast.error(`Failed to stop VM: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  }

  async function deleteVM(vm: VM) {
    confirmDialog = {
      open: true,
      title: 'Delete VM',
      description: `Are you sure you want to delete "${vm.name}"? This action cannot be undone.`,
      action: async () => {
        try {
          await client.deleteVM(vm.id);
          toast.success(`VM "${vm.name}" deleted`);
          loadData();
        } catch (err) {
          toast.error(`Failed to delete VM: ${err instanceof Error ? err.message : 'Unknown error'}`);
        }
      }
    };
  }
</script>

<svelte:head>
  <title>Virtual Machines | {node?.name ?? 'Node'}</title>
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
        <h1 class="text-2xl font-bold text-slate-900">Virtual Machines</h1>
        <p class="text-sm text-slate-500">Node: {node?.name ?? 'Loading...'}</p>
      </div>
    </div>
    <button
      onclick={() => createModalOpen = true}
      class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 hover:bg-orange-600 text-white text-sm font-medium rounded-lg transition-colors"
    >
      <Plus size={16} />
      Create VM
    </button>
  </div>

  <!-- Stats -->
  <div class="grid gap-4 md:grid-cols-4">
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-blue-50 rounded-lg">
          <Server size={20} class="text-blue-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Total VMs</p>
          <p class="text-xl font-bold text-slate-900">{items.length}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-green-50 rounded-lg">
          <Server size={20} class="text-green-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Running</p>
          <p class="text-xl font-bold text-slate-900">{items.filter(v => v.actual_state === 'running').length}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-slate-100 rounded-lg">
          <Server size={20} class="text-slate-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Stopped</p>
          <p class="text-xl font-bold text-slate-900">{items.filter(v => v.actual_state === 'stopped').length}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-red-50 rounded-lg">
          <Server size={20} class="text-red-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Error</p>
          <p class="text-xl font-bold text-slate-900">{items.filter(v => v.actual_state === 'error').length}</p>
        </div>
      </div>
    </div>
  </div>

  <!-- VM Table -->
  <div class="bg-white rounded-lg shadow-sm border border-slate-200">
    <FilterBar
      filters={filterOptions}
      activeFilters={{ actual_state: table.filters.actual_state }}
      onFilterChange={(key, value) => table.setFilter(key, value)}
      onClearAll={table.clearAllFilters}
    />
    
    <DataTable
      data={table.paginatedData}
      {columns}
      {loading}
      selectable={true}
      selectedIds={Array.from(table.selectedIds)}
      sortColumn={table.sortColumn}
      sortDirection={table.sortDirection}
      onSort={handleSort}
      onSelect={ids => {
        table.selectNone();
        ids.forEach(id => table.select(id));
      }}
      rowId={vm => vm.id}
      getRowHref={vm => `/vms/${vm.id}`}
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

<CreateVMModal
  bind:open={createModalOpen}
  {images}
  {pools}
  {networks}
  onSuccess={loadData}
/>

<ConfirmDialog
  bind:open={confirmDialog.open}
  title={confirmDialog.title}
  description={confirmDialog.description}
  confirmLabel="Delete"
  onConfirm={confirmDialog.action}
/>
