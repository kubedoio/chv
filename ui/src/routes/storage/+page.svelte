<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { HardDrive, Plus } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/DataTable.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import FilterBar from '$lib/components/FilterBar.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import CreateStoragePoolModal from '$lib/components/CreateStoragePoolModal.svelte';
  import { useTable, formatBytes } from '$lib/utils/table.svelte';
  import type { StoragePool } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  let items: StoragePool[] = $state([]);
  let loading = $state(true);
  let createModalOpen = $state(false);

  // Table state management - reactive to items
  let table = $derived(useTable<StoragePool>({
    data: items,
    pageSize: 10
  }));

  // Filter options
  const filterOptions = [
    {
      key: 'pool_type',
      label: 'Type',
      type: 'select' as const,
      options: [
        { value: 'localdisk', label: 'Local Disk' }
      ]
    },
    {
      key: 'status',
      label: 'Status',
      type: 'select' as const,
      options: [
        { value: 'active', label: 'Active' },
        { value: 'inactive', label: 'Inactive' },
        { value: 'error', label: 'Error' }
      ]
    }
  ];

  // Table columns definition
  const columns = [
    {
      key: 'name',
      title: 'Name',
      sortable: true
    },
    {
      key: 'pool_type',
      title: 'Type',
      align: 'center' as const,
      width: '100px',
      render: (pool: StoragePool) => pool.pool_type === 'localdisk' ? 'Local' : pool.pool_type
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
      align: 'right' as const,
      width: '120px',
      render: (pool: StoragePool) => {
        if (!pool.capacity_bytes) return '—';
        return formatBytes(pool.capacity_bytes);
      }
    },
    {
      key: 'allocatable_bytes',
      title: 'Available',
      align: 'right' as const,
      width: '120px',
      render: (pool: StoragePool) => {
        if (!pool.allocatable_bytes) return '—';
        return formatBytes(pool.allocatable_bytes);
      }
    },
    {
      key: 'used',
      title: 'Used',
      align: 'right' as const,
      width: '120px',
      render: (pool: StoragePool) => {
        if (!pool.capacity_bytes || !pool.allocatable_bytes) return '—';
        const used = pool.capacity_bytes - pool.allocatable_bytes;
        return formatBytes(used);
      }
    },
    {
      key: 'is_default',
      title: 'Default',
      align: 'center' as const,
      width: '80px',
      render: (pool: StoragePool) => pool.is_default ? 'Yes' : 'No'
    }
  ];

  async function loadStoragePools() {
    loading = true;
    try {
      items = await client.listStoragePools();
    } catch (err) {
      toast.error('Failed to load storage pools');
      items = [];
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (!token) {
      goto('/login');
      return;
    }
    loadStoragePools();
  });

  function handleSort(column: string, direction: 'asc' | 'desc' | null) {
    if (direction) {
      table.setSort(column, direction);
    } else {
      table.clearSort();
    }
  }
</script>

<section class="table-card">
  <div class="card-header px-4 py-3 flex items-center justify-between">
    <div>
      <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Storage</div>
      <div class="mt-1 text-lg font-semibold">Storage Pools</div>
    </div>
    <button
      onclick={() => createModalOpen = true}
      class="px-4 py-2 rounded bg-primary text-white font-medium text-sm hover:bg-primary/90 transition-colors flex items-center gap-2"
    >
      <Plus size={16} />
      Create Pool
    </button>
  </div>

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
    sortColumn={table.sortColumn ?? undefined}
    sortDirection={table.sortDirection}
    emptyIcon={HardDrive as unknown as typeof import('svelte').SvelteComponent}
    emptyTitle="No storage pools yet"
    emptyDescription="Create a storage pool to store VM disks"
    onSort={handleSort}
    rowId={(pool: StoragePool) => pool.id}
  >
    {#snippet children(pool: StoragePool)}
      <StateBadge label={pool.status} />
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

<CreateStoragePoolModal 
  bind:open={createModalOpen} 
  onSuccess={loadStoragePools}
  existingNames={items.map(i => i.name)}
/>
