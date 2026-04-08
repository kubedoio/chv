<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Server, Play, Square, AlertCircle, Plus } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import StatsCard from '$lib/components/StatsCard.svelte';
  import SkeletonRow from '$lib/components/SkeletonRow.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import CreateVMModal from '$lib/components/CreateVMModal.svelte';
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

  // Computed stats
  const total = $derived(items.length);
  const running = $derived(items.filter(vm => vm.actual_state === 'running').length);
  const stopped = $derived(items.filter(vm => vm.actual_state === 'stopped').length);
  const other = $derived(items.filter(vm => !['running', 'stopped'].includes(vm.actual_state)).length);

  async function loadVMs() {
    loading = true;
    error = '';
    try {
      items = await client.listVMs();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load VMs';
      toast.error(error);
      items = [];
    }
    loading = false;
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
    // Check if user is logged in
    if (!token) {
      goto('/login');
      return;
    }
    loadVMs();
    loadDependencies();
  });
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
    class="ml-4 px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors flex items-center gap-2"
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

{#if loading}
  <!-- Loading state with skeleton rows -->
  <section class="table-card">
    <div class="card-header px-4 py-3">
      <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Virtual Machines</div>
      <div class="mt-1 text-lg font-semibold">VM List</div>
    </div>

    <table class="w-full border-collapse text-sm">
      <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
        <tr>
          <th class="border-b border-line px-4 py-3">Name</th>
          <th class="border-b border-line px-4 py-3">State</th>
          <th class="border-b border-line px-4 py-3">Image</th>
          <th class="border-b border-line px-4 py-3">Pool</th>
          <th class="border-b border-line px-4 py-3">Network</th>
          <th class="border-b border-line px-4 py-3">vCPU</th>
          <th class="border-b border-line px-4 py-3">Memory</th>
          <th class="border-b border-line px-4 py-3">IP</th>
          <th class="border-b border-line px-4 py-3">Last Error</th>
        </tr>
      </thead>
      <tbody>
        {#each Array(5) as _}
          <SkeletonRow columns={9} />
        {/each}
      </tbody>
    </table>
  </section>
{:else if items.length === 0}
  <!-- Empty state when no VMs -->
  <section class="table-card">
    <div class="card-header px-4 py-3">
      <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Virtual Machines</div>
      <div class="mt-1 text-lg font-semibold">VM List</div>
    </div>
    <EmptyState
      icon={Server}
      title="No VMs yet"
      description="Create a virtual machine to get started"
    >
      <button 
        onclick={() => createModalOpen = true}
        class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors flex items-center gap-2"
      >
        <Plus size={16} />
        Create VM
      </button>
    </EmptyState>
  </section>
{:else}
  <!-- Table with data -->
  <section class="table-card">
    <div class="card-header px-4 py-3">
      <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Virtual Machines</div>
      <div class="mt-1 text-lg font-semibold">VM List</div>
    </div>

    <table class="w-full border-collapse text-sm">
      <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
        <tr>
          <th class="border-b border-line px-4 py-3">Name</th>
          <th class="border-b border-line px-4 py-3">State</th>
          <th class="border-b border-line px-4 py-3">Image</th>
          <th class="border-b border-line px-4 py-3">Pool</th>
          <th class="border-b border-line px-4 py-3">Network</th>
          <th class="border-b border-line px-4 py-3">vCPU</th>
          <th class="border-b border-line px-4 py-3">Memory</th>
          <th class="border-b border-line px-4 py-3">IP</th>
          <th class="border-b border-line px-4 py-3">Last Error</th>
        </tr>
      </thead>
      <tbody>
        {#each items as item}
          <tr class="odd:bg-white even:bg-[#f8f8f8]">
            <td class="border-b border-line px-4 py-3">
              <a class="text-primary no-underline hover:underline" href={`/vms/${item.id}`}>{item.name}</a>
            </td>
            <td class="border-b border-line px-4 py-3">
              {#if item.desired_state === item.actual_state}
                <StateBadge label={item.actual_state} />
              {:else}
                <div class="flex flex-col gap-1">
                  <span class="text-xs text-muted">desired: {item.desired_state}</span>
                  <StateBadge label={item.actual_state} />
                </div>
              {/if}
            </td>
            <td class="border-b border-line px-4 py-3">{item.image_id}</td>
            <td class="border-b border-line px-4 py-3">{item.storage_pool_id}</td>
            <td class="border-b border-line px-4 py-3">{item.network_id}</td>
            <td class="border-b border-line px-4 py-3 mono">{item.vcpu}</td>
            <td class="border-b border-line px-4 py-3 mono">{item.memory_mb} MB</td>
            <td class="border-b border-line px-4 py-3 mono">{item.ip_address || 'pending'}</td>
            <td class="border-b border-line px-4 py-3 text-danger text-xs max-w-[200px] truncate" title={item.last_error}>
              {item.last_error || '—'}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}

<CreateVMModal 
  bind:open={createModalOpen} 
  {images} 
  {pools} 
  {networks}
  onSuccess={loadVMs}
/>
