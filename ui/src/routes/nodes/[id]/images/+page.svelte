<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { Image as ImageIcon, ArrowLeft, Plus, Download } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/data-display/DataTable.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import ImportImageModal from '$lib/components/modals/ImportImageModal.svelte';
  import { useTable, formatBytes } from '$lib/utils/table.svelte';
  import { getDefaultNode } from '$lib/api/nodes';
  import type { Image } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  const nodeId = $derived($page.params.id as string);
  const node = $derived(getDefaultNode());
  
  let items: Image[] = $state([]);
  let loading = $state(true);
  let importModalOpen = $state(false);

  // Table state management
  let table = useTable<Image>({
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
      render: (img: Image) => img.name
    },
    {
      key: 'os_family',
      title: 'OS Family',
      sortable: true,
      render: (img: Image) => img.os_family
    },
    {
      key: 'architecture',
      title: 'Architecture',
      sortable: true,
      width: '120px',
      render: (img: Image) => img.architecture
    },
    {
      key: 'status',
      title: 'Status',
      sortable: true,
      width: '120px',
      render: (img: Image) => img.status
    },
    {
      key: 'cloud_init_supported',
      title: 'Cloud-Init',
      width: '120px',
      render: (img: Image) => img.cloud_init_supported ? 'Supported' : 'Not Supported'
    },
    {
      key: 'created_at',
      title: 'Created',
      sortable: true,
      width: '150px',
      render: (img: Image) => img.created_at ? new Date(img.created_at).toLocaleDateString() : 'N/A'
    }
  ];

  async function loadData() {
    loading = true;
    try {
      // Use node-scoped API to get images for this node
      const response = await client.listNodeImages(nodeId);
      items = response.resources;
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to load images');
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
  <title>Images | {node.name}</title>
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
        <h1 class="text-2xl font-bold text-slate-900">Images</h1>
        <p class="text-sm text-slate-500">Node: {node.name}</p>
      </div>
    </div>
    <button
      onclick={() => importModalOpen = true}
      class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 hover:bg-orange-600 text-white text-sm font-medium rounded-lg transition-colors"
    >
      <Download size={16} />
      Import Image
    </button>
  </div>

  <!-- Stats -->
  <div class="grid gap-4 md:grid-cols-4">
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-purple-50 rounded-lg">
          <ImageIcon size={20} class="text-purple-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Total Images</p>
          <p class="text-xl font-bold text-slate-900">{items.length}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-green-50 rounded-lg">
          <ImageIcon size={20} class="text-green-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">Ready</p>
          <p class="text-xl font-bold text-slate-900">{items.filter(i => i.status === 'ready').length}</p>
        </div>
      </div>
    </div>
    <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-blue-50 rounded-lg">
          <ImageIcon size={20} class="text-blue-600" />
        </div>
        <div>
          <p class="text-xs text-slate-500 uppercase">With Cloud-Init</p>
          <p class="text-xl font-bold text-slate-900">{items.filter(i => i.cloud_init_supported).length}</p>
        </div>
      </div>
    </div>
  </div>

  <!-- Images Table -->
  <div class="bg-white rounded-lg shadow-sm border border-slate-200">
    <DataTable
      data={table.paginatedData}
      {columns}
      {loading}
      sortColumn={table.sortColumn}
      sortDirection={table.sortDirection}
      onSort={handleSort}
      rowId={img => img.id}
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

<ImportImageModal
  bind:open={importModalOpen}
  onSuccess={loadData}
/>
