<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Network, Plus } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/DataTable.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import FilterBar from '$lib/components/FilterBar.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import CreateNetworkModal from '$lib/components/CreateNetworkModal.svelte';
  import { useTable } from '$lib/utils/table.svelte';
  import type { Network as NetworkType } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  let items: NetworkType[] = $state([]);
  let loading = $state(true);
  let createModalOpen = $state(false);

  // Table state management - reactive to items
  let table = $derived(useTable<NetworkType>({
    data: items,
    pageSize: 10
  }));

  // Filter options
  const filterOptions = [
    {
      key: 'mode',
      label: 'Mode',
      type: 'select' as const,
      options: [
        { value: 'bridge', label: 'Bridge' },
        { value: 'nat', label: 'NAT' },
        { value: 'macvtap', label: 'MacVTap' }
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
      key: 'mode',
      title: 'Mode',
      sortable: true,
      align: 'center' as const,
      width: '100px',
      render: (net: NetworkType) => net.mode || 'bridge'
    },
    {
      key: 'bridge_name',
      title: 'Bridge',
      width: '140px',
      render: (net: NetworkType) => net.bridge_name
    },
    {
      key: 'cidr',
      title: 'CIDR',
      width: '140px',
      render: (net: NetworkType) => net.cidr || '—'
    },
    {
      key: 'gateway_ip',
      title: 'Gateway',
      width: '130px',
      render: (net: NetworkType) => net.gateway_ip || '—'
    },
    {
      key: 'is_system_managed',
      title: 'Managed By',
      width: '120px',
      render: (net: NetworkType) => net.is_system_managed ? 'System' : 'User'
    }
  ];

  async function loadNetworks() {
    loading = true;
    try {
      items = await client.listNetworks();
    } catch (err) {
      toast.error('Failed to load networks');
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
    loadNetworks();
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
      <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Networks</div>
      <div class="mt-1 text-lg font-semibold">Bridge-backed Networks</div>
    </div>
    <button
      onclick={() => createModalOpen = true}
      class="px-4 py-2 rounded bg-primary text-white font-medium text-sm hover:bg-primary/90 transition-colors flex items-center gap-2"
    >
      <Plus size={16} />
      Create Network
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
    emptyIcon={Network as unknown as typeof import('svelte').SvelteComponent}
    emptyTitle="No networks yet"
    emptyDescription="Create a network to connect your VMs"
    onSort={handleSort}
    rowId={(net: NetworkType) => net.id}
  >
    {#snippet children(net: NetworkType)}
      <StateBadge label={net.status} />
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

<CreateNetworkModal bind:open={createModalOpen} onSuccess={loadNetworks} />
