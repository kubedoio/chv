<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    Server,
    Circle,
    Cpu,
    HardDrive,
    Network,
    CheckCircle,
    ArrowRight,
    Plus,
    Settings,
    Trash2,
    Activity
  } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import AddNodeModal from '$lib/components/AddNodeModal.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import type { NodeWithResources, CreateNodeInput, CreateNodeResponse } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });

  let nodes = $state<NodeWithResources[]>([]);
  let loading = $state(true);
  let showAddModal = $state(false);
  let selectedNodeId = $state<string | null>(null);

  async function loadNodes() {
    loading = true;
    try {
      const data = (await client.listNodes()) ?? [];
      nodes = data;
    } catch (e) {
      console.error('Failed to load nodes:', e);
      toast.error('Failed to load nodes');
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (!token) {
      goto('/login');
      return;
    }
    loadNodes();
  });

  async function handleCreateNode(data: CreateNodeInput): Promise<CreateNodeResponse> {
    const result = await client.createNode(data);
    await loadNodes(); // Refresh the list
    return result;
  }

  async function handleDeleteNode(nodeId: string) {
    if (!confirm('Are you sure you want to delete this node? This action cannot be undone.')) {
      return;
    }

    try {
      await client.deleteNode(nodeId);
      toast.success('Node deleted successfully');
      await loadNodes();
    } catch (e) {
      console.error('Failed to delete node:', e);
      toast.error('Failed to delete node');
    }
  }

  async function handleToggleMaintenance(node: NodeWithResources) {
    const newStatus = node.status === 'maintenance' ? 'online' : 'maintenance';
    try {
      await client.setNodeMaintenance(node.id, newStatus === 'maintenance');
      toast.success(`Node ${newStatus === 'maintenance' ? 'set to maintenance' : 'brought online'}`);
      await loadNodes();
    } catch (e) {
      console.error('Failed to update node status:', e);
      toast.error('Failed to update node status');
    }
  }

  function getStatusColor(status: string): string {
    switch (status) {
      case 'online': return 'text-green-500';
      case 'offline': return 'text-red-500';
      case 'maintenance': return 'text-orange-500';
      case 'error': return 'text-red-600';
      default: return 'text-slate-400';
    }
  }

  function getStatusBg(status: string): string {
    switch (status) {
      case 'online': return 'bg-green-100';
      case 'offline': return 'bg-red-100';
      case 'maintenance': return 'bg-orange-100';
      case 'error': return 'bg-red-100';
      default: return 'bg-slate-100';
    }
  }

  const columns = [
    { key: 'name', title: 'Name', sortable: true },
    { key: 'hostname', title: 'Hostname', sortable: true },
    { key: 'ip_address', title: 'IP Address', sortable: true },
    { key: 'status', title: 'Status', sortable: true },
    { key: 'resources', title: 'Resources' },
    { key: 'last_seen', title: 'Last Seen' },
  ];

  function formatLastSeen(lastSeenAt?: string): string {
    if (!lastSeenAt) return 'Never';
    
    const date = new Date(lastSeenAt);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffSec = Math.floor(diffMs / 1000);
    const diffMin = Math.floor(diffSec / 60);
    const diffHour = Math.floor(diffMin / 60);
    const diffDay = Math.floor(diffHour / 24);

    if (diffSec < 60) return 'Just now';
    if (diffMin < 60) return `${diffMin}m ago`;
    if (diffHour < 24) return `${diffHour}h ago`;
    if (diffDay < 7) return `${diffDay}d ago`;
    return date.toLocaleDateString();
  }
</script>

<svelte:head>
  <title>Nodes | CHV</title>
</svelte:head>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-bold text-slate-900">Nodes</h1>
      <p class="text-sm text-slate-500 mt-1">Manage compute nodes in your datacenter</p>
    </div>
    <button
      type="button"
      onclick={() => showAddModal = true}
      class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
    >
      <Plus size={18} />
      Add Node
    </button>
  </div>

  <!-- Nodes List -->
  {#if loading}
    <div class="flex items-center justify-center h-64">
      <div class="flex items-center gap-3 text-slate-500">
        <div class="w-5 h-5 border-2 border-slate-300 border-t-orange-500 rounded-full animate-spin"></div>
        <span>Loading nodes...</span>
      </div>
    </div>
  {:else if nodes.length === 0}
    <!-- Empty State -->
    <div class="text-center py-16 bg-white rounded-lg border border-slate-200">
      <Server size={48} class="mx-auto mb-4 text-slate-300" />
      <h3 class="text-lg font-medium text-slate-900">No nodes configured</h3>
      <p class="text-sm text-slate-500 mt-1 mb-6">Add your first compute node to get started</p>
      <button
        type="button"
        onclick={() => showAddModal = true}
        class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
      >
        <Plus size={18} />
        Add Node
      </button>
    </div>
  {:else}
    <!-- Grid View -->
    <div class="grid gap-4 lg:grid-cols-2 xl:grid-cols-3">
      {#each nodes as node (node.id)}
        <div class="group bg-white rounded-lg shadow-sm border border-slate-200 hover:shadow-md hover:border-orange-200 transition-all">
          <a href="/nodes/{node.id}" class="block p-5">
            <div class="flex items-start justify-between">
              <div class="flex items-center gap-3">
                <div class="w-10 h-10 rounded-lg bg-gradient-to-br from-orange-500 to-red-600 flex items-center justify-center">
                  <Server class="text-white" size={20} />
                </div>
                <div>
                  <h3 class="font-semibold text-slate-900 group-hover:text-orange-600 transition-colors">{node.name}</h3>
                  <div class="flex items-center gap-2 mt-0.5">
                    <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium {getStatusBg(node.status)}">
                      <Circle size={6} class={getStatusColor(node.status)} fill="currentColor" />
                      <span class="capitalize text-slate-700">{node.status}</span>
                    </span>
                    {#if node.is_local}
                      <span class="text-xs px-1.5 py-0.5 bg-slate-100 text-slate-600 rounded">Local</span>
                    {/if}
                  </div>
                </div>
              </div>
              <ArrowRight size={18} class="text-slate-400 group-hover:text-orange-500 transition-colors" />
            </div>

            <div class="mt-4 pt-4 border-t border-slate-100">
              <div class="grid grid-cols-2 gap-4">
                <div class="flex items-center gap-2">
                  <Cpu size={14} class="text-slate-400" />
                  <span class="text-sm text-slate-600">{node.resources?.vms ?? 0} VMs</span>
                </div>
                <div class="flex items-center gap-2">
                  <CheckCircle size={14} class="text-slate-400" />
                  <span class="text-sm text-slate-600">{node.resources?.images ?? 0} Images</span>
                </div>
                <div class="flex items-center gap-2">
                  <HardDrive size={14} class="text-slate-400" />
                  <span class="text-sm text-slate-600">{node.resources?.storage_pools ?? 0} Pools</span>
                </div>
                <div class="flex items-center gap-2">
                  <Network size={14} class="text-slate-400" />
                  <span class="text-sm text-slate-600">{node.resources?.networks ?? 0} Networks</span>
                </div>
              </div>
            </div>

            <div class="mt-4 flex items-center justify-between text-xs text-slate-500">
              <span>{node.hostname}</span>
              <span>{node.ip_address}</span>
            </div>

            {#if !node.is_local}
              <div class="mt-3 pt-3 border-t border-slate-100 flex items-center justify-between text-xs">
                <span class="text-slate-500 flex items-center gap-1">
                  <Activity size={12} />
                  Last seen: {formatLastSeen(node.last_seen_at)}
                </span>
              </div>
            {/if}
          </a>

          <!-- Action Buttons (only for non-local nodes) -->
          {#if !node.is_local}
            <div class="px-5 pb-4 flex items-center gap-2">
              <button
                type="button"
                onclick={() => handleToggleMaintenance(node)}
                class="flex-1 inline-flex items-center justify-center gap-1.5 px-3 py-1.5 text-sm font-medium text-slate-700 bg-slate-50 border border-slate-200 rounded-md hover:bg-slate-100 transition-colors"
                title={node.status === 'maintenance' ? 'Bring node online' : 'Set to maintenance mode'}
              >
                <Settings size={14} />
                {node.status === 'maintenance' ? 'Online' : 'Maintenance'}
              </button>
              <button
                type="button"
                onclick={() => handleDeleteNode(node.id)}
                class="inline-flex items-center justify-center gap-1.5 px-3 py-1.5 text-sm font-medium text-red-600 bg-red-50 border border-red-200 rounded-md hover:bg-red-100 transition-colors"
                title="Delete node"
              >
                <Trash2 size={14} />
              </button>
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <!-- List View Alternative (DataTable) -->
    <div class="hidden">
      <DataTable
        data={nodes}
        {columns}
        rowId={(node) => node.id}
        loading={loading}
        emptyTitle="No nodes found"
        emptyDescription="Add a new node to get started"
      >
        {#snippet children(row: NodeWithResources)}
          <div class="flex items-center gap-2">
            <a
              href="/nodes/{row.id}"
              class="text-orange-600 hover:text-orange-700 font-medium text-sm"
            >
              View
            </a>
          </div>
        {/snippet}
      </DataTable>
    </div>
  {/if}
</div>

<!-- Add Node Modal -->
<AddNodeModal
  bind:open={showAddModal}
  onClose={() => showAddModal = false}
  onSubmit={handleCreateNode}
/>
