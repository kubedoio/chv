<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Server, Play, Square, AlertCircle, Plus, Trash2, Settings } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/DataTable.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import FilterBar from '$lib/components/FilterBar.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import StatsCard from '$lib/components/StatsCard.svelte';
  import CreateVMModal from '$lib/components/CreateVMModal.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { useTable, formatBytes } from '$lib/utils/table.svelte';
  import type { VM, Image, StoragePool, Network } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  
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

  // Table state management - create new table when items change
  let table = $derived(useTable<VM>({
    data: items,
    pageSize: 10
  }));

  // Lookup maps for related data
  const imageMap = $derived(
    new Map(images.map(i => [i.id, i]))
  );
  const poolMap = $derived(
    new Map(pools.map(p => [p.id, p]))
  );
  const networkMap = $derived(
    new Map(networks.map(n => [n.id, n]))
  );

  // Computed stats
  const total = $derived(items.length);
  const running = $derived(items.filter(vm => vm.actual_state === 'running').length);
  const stopped = $derived(items.filter(vm => vm.actual_state === 'stopped').length);
  const other = $derived(items.filter(vm => !['running', 'stopped'].includes(vm.actual_state)).length);

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
        const img = imageMap.get(vm.image_id);
        return img?.name ?? vm.image_id;
      }
    },
    {
      key: 'storage_pool_id',
      title: 'Pool',
      render: (vm: VM) => {
        const pool = poolMap.get(vm.storage_pool_id);
        return pool?.name ?? vm.storage_pool_id;
      }
    },
    {
      key: 'network_id',
      title: 'Network',
      render: (vm: VM) => {
        const net = networkMap.get(vm.network_id);
        return net?.name ?? vm.network_id;
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

  async function loadVMs() {
    loading = true;
    error = '';
    try {
      items = await client.listVMs();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load VMs';
      toast.error(error);
      items = [];
    } finally {
      loading = false;
    }
  }

  async function loadDependencies() {
    try {
      const [imgs, ps, nets] = await Promise.all([
        client.listImages(),
        client.listStoragePools(),
        client.listNetworks()
      ]);
      images = imgs;
      pools = ps;
      networks = nets;
    } catch (e) {
      console.error('Failed to load dependencies:', e);
    }
  }

  onMount(() => {
    if (!token) {
      goto('/login');
      return;
    }
    loadVMs();
    loadDependencies();
  });

  function handleSort(column: string, direction: 'asc' | 'desc' | null) {
    if (direction) {
      table.setSort(column, direction);
    } else {
      table.clearSort();
    }
  }

  function handleSelect(ids: string[]) {
    const newSet = new Set(ids);
    table.selectedIds.forEach(id => {
      if (!newSet.has(id)) table.deselect(id);
    });
    ids.forEach(id => {
      if (!table.selectedIds.has(id)) table.select(id);
    });
  }

  async function startVM(vm: VM) {
    try {
      await client.startVM(vm.id);
      toast.success(`VM "${vm.name}" started`);
      loadVMs();
    } catch (err) {
      toast.error(`Failed to start VM: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  }

  async function stopVM(vm: VM) {
    try {
      await client.stopVM(vm.id);
      toast.success(`VM "${vm.name}" stopped`);
      loadVMs();
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
          loadVMs();
        } catch (err) {
          toast.error(`Failed to delete VM: ${err instanceof Error ? err.message : 'Unknown error'}`);
        }
      }
    };
  }

  async function handleBulkAction(action: 'start' | 'stop' | 'delete') {
    const selectedIds = Array.from(table.selectedIds);
    if (selectedIds.length === 0) return;
    
    const count = selectedIds.length;
    
    if (action === 'delete') {
      confirmDialog = {
        open: true,
        title: 'Delete VMs',
        description: `Are you sure you want to delete ${count} VM${count > 1 ? 's' : ''}? This action cannot be undone.`,
        action: async () => {
          try {
            await client.bulkDeleteVMs(selectedIds);
            toast.success(`${count} VM${count > 1 ? 's' : ''} deleted`);
            table.selectNone();
            loadVMs();
          } catch (err) {
            toast.error(`Bulk delete failed: ${err instanceof Error ? err.message : 'Unknown error'}`);
          }
        }
      };
    } else if (action === 'start') {
      try {
        await client.bulkStartVMs(selectedIds);
        toast.success(`${count} VM${count > 1 ? 's' : ''} started`);
        table.selectNone();
        loadVMs();
      } catch (err) {
        toast.error(`Bulk start failed: ${err instanceof Error ? err.message : 'Unknown error'}`);
      }
    } else if (action === 'stop') {
      try {
        await client.bulkStopVMs(selectedIds);
        toast.success(`${count} VM${count > 1 ? 's' : ''} stopped`);
        table.selectNone();
        loadVMs();
      } catch (err) {
        toast.error(`Bulk stop failed: ${err instanceof Error ? err.message : 'Unknown error'}`);
      }
    }
  }

  function navigateToVM(vm: VM) {
    goto(`/vms/${vm.id}`);
  }
</script>

<!-- Header with stats cards and create button -->
<div class="flex justify-between items-start mb-6">
  <div class="grid grid-cols-2 lg:grid-cols-4 gap-4 flex-1">
    <StatsCard title="Total VMs" value={total} icon={Server} />
    <StatsCard title="Running" value={running} icon={Play} trend="up" />
    <StatsCard title="Stopped" value={stopped} icon={Square} />
    <StatsCard title="Other" value={other} icon={AlertCircle} />
  </div>
  <button 
    onclick={() => createModalOpen = true} 
    class="ml-4 button-primary flex items-center gap-2"
  >
    <Plus size={16} />
    Create VM
  </button>
</div>

{#if error}
  <div class="mb-4 border border-danger bg-red-50 px-4 py-3 text-danger">
    {error}
  </div>
{/if}

<section class="table-card">
  <!-- Filter bar -->
  <FilterBar
    filters={filterOptions}
    activeFilters={table.filters}
    onFilterChange={table.setFilter}
    onClearAll={table.clearAllFilters}
  />

  <!-- Data table -->
  <DataTable
    data={table.paginatedData}
    {columns}
    {loading}
    selectable={true}
    selectedIds={Array.from(table.selectedIds)}
    sortColumn={table.sortColumn ?? undefined}
    sortDirection={table.sortDirection}
    emptyIcon={Server as unknown as typeof import('svelte').SvelteComponent}
    emptyTitle="No VMs yet"
    emptyDescription="Create a virtual machine to get started"
    onSort={handleSort}
    onSelect={handleSelect}
    onRowClick={navigateToVM}
    rowId={(vm: VM) => vm.id}
  >
    {#snippet children(vm: VM)}
      <div class="flex items-center gap-1">
        {#if vm.actual_state === 'stopped'}
          <button
            type="button"
            class="action-btn start"
            onclick={(e) => { e.stopPropagation(); startVM(vm); }}
            title="Start VM"
          >
            <Play size={14} />
          </button>
        {:else if vm.actual_state === 'running'}
          <button
            type="button"
            class="action-btn stop"
            onclick={(e) => { e.stopPropagation(); stopVM(vm); }}
            title="Stop VM"
          >
            <Square size={14} />
          </button>
        {/if}
        <a
          href={`/vms/${vm.id}`}
          class="action-btn"
          onclick={(e) => e.stopPropagation()}
          title="Settings"
        >
          <Settings size={14} />
        </a>
        <button
          type="button"
          class="action-btn danger"
          onclick={(e) => { e.stopPropagation(); deleteVM(vm); }}
          title="Delete VM"
        >
          <Trash2 size={14} />
        </button>
      </div>
    {/snippet}
  </DataTable>

  <!-- Pagination -->
  {#if !loading && table.totalItems > 0}
    <Pagination
      page={table.page}
      pageSize={table.pageSize}
      totalItems={table.totalItems}
      onPageChange={table.setPage}
      onPageSizeChange={table.setPageSize}
    />
  {/if}
</section>

<!-- Bulk action bar -->
{#if table.selectedCount > 0}
  <div class="fixed bottom-6 left-1/2 -translate-x-1/2 bg-ink text-white px-6 py-3 rounded-full shadow-2xl flex items-center gap-6 z-50 animate-in fade-in slide-in-from-bottom-4 duration-300">
    <div class="flex items-center gap-2 border-r border-white/20 pr-6">
      <span class="bg-primary text-[10px] font-bold px-1.5 py-0.5 rounded uppercase tracking-wider">{table.selectedCount}</span>
      <span class="text-sm font-medium">Selected</span>
    </div>
    
    <div class="flex items-center gap-4">
      <button 
        onclick={() => handleBulkAction('start')}
        class="flex items-center gap-2 text-sm hover:text-primary transition-colors font-medium"
      >
        <Play size={14} fill="currentColor" />
        Start
      </button>
      
      <button 
        onclick={() => handleBulkAction('stop')}
        class="flex items-center gap-2 text-sm hover:text-primary transition-colors font-medium"
      >
        <Square size={14} fill="currentColor" />
        Stop
      </button>
      
      <button 
        onclick={() => handleBulkAction('delete')}
        class="flex items-center gap-2 text-sm text-danger hover:text-red-400 transition-colors font-medium"
      >
        <Trash2 size={14} />
        Delete
      </button>
    </div>
    
    <button 
      onclick={() => table.selectNone()}
      class="ml-2 text-white/50 hover:text-white transition-colors"
    >
      Cancel
    </button>
  </div>
{/if}

<CreateVMModal 
  bind:open={createModalOpen} 
  {images} 
  {pools} 
  {networks}
  onSuccess={loadVMs}
/>

<ConfirmDialog
  bind:open={confirmDialog.open}
  title={confirmDialog.title}
  description={confirmDialog.description}
  confirmText="Delete"
  variant="danger"
  onConfirm={() => { confirmDialog.action(); confirmDialog.open = false; }}
  onCancel={() => confirmDialog.open = false}
/>

<style>
  .action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    color: var(--color-neutral-500);
    background: transparent;
    border: none;
    cursor: pointer;
    transition: all var(--duration-fast);
  }

  .action-btn:hover {
    background: var(--color-neutral-100);
    color: var(--color-neutral-700);
  }

  .action-btn.start:hover {
    color: var(--color-success);
    background: var(--color-success-light);
  }

  .action-btn.stop:hover {
    color: var(--color-warning);
    background: var(--color-warning-light);
  }

  .action-btn.danger:hover {
    color: var(--color-danger);
    background: var(--color-danger-light);
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slide-in-from-bottom-4 {
    from { transform: translate(-50%, 1rem); }
    to { transform: translate(-50%, 0); }
  }

  .animate-in {
    animation-fill-mode: both;
  }

  .fade-in {
    animation-name: fade-in;
  }

  .slide-in-from-bottom-4 {
    animation-name: slide-in-from-bottom-4;
  }

  .duration-300 {
    animation-duration: 300ms;
  }
</style>
