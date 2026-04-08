<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Network, Plus } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import SkeletonRow from '$lib/components/SkeletonRow.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import CreateNetworkModal from '$lib/components/CreateNetworkModal.svelte';
  import type { Network as NetworkType } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  let items: NetworkType[] = $state([]);
  let loading = $state(true);
  let createModalOpen = $state(false);

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
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M5 12h14"/>
        <path d="M12 5v14"/>
      </svg>
      Create
    </button>
  </div>

  {#if loading}
    <table class="w-full border-collapse text-sm">
      <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
        <tr>
          <th class="border-b border-line px-4 py-3">Name</th>
          <th class="border-b border-line px-4 py-3">Bridge</th>
          <th class="border-b border-line px-4 py-3">CIDR</th>
          <th class="border-b border-line px-4 py-3">Gateway</th>
          <th class="border-b border-line px-4 py-3">Managed</th>
          <th class="border-b border-line px-4 py-3">Status</th>
        </tr>
      </thead>
      <tbody>
        {#each Array(5) as _}
          <SkeletonRow columns={6} />
        {/each}
      </tbody>
    </table>
  {:else if items.length === 0}
    <EmptyState
      icon={Network}
      title="No networks yet"
      description="Create a network to connect your VMs"
    >
      <button
        onclick={() => createModalOpen = true}
        class="px-4 py-2 rounded bg-primary text-white font-medium text-sm hover:bg-primary/90 transition-colors flex items-center gap-2"
      >
        <Plus size={16} />
        Create Network
      </button>
    </EmptyState>
  {:else}
    <table class="w-full border-collapse text-sm">
      <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
        <tr>
          <th class="border-b border-line px-4 py-3">Name</th>
          <th class="border-b border-line px-4 py-3">Bridge</th>
          <th class="border-b border-line px-4 py-3">CIDR</th>
          <th class="border-b border-line px-4 py-3">Gateway</th>
          <th class="border-b border-line px-4 py-3">Managed</th>
          <th class="border-b border-line px-4 py-3">Status</th>
        </tr>
      </thead>
      <tbody>
        {#each items as item}
          <tr class="odd:bg-white even:bg-[#f8f8f8] hover:bg-hover transition-colors">
            <td class="border-b border-line px-4 py-3 font-medium">{item.name}</td>
            <td class="border-b border-line px-4 py-3 mono">{item.bridge_name}</td>
            <td class="border-b border-line px-4 py-3 mono">{item.cidr}</td>
            <td class="border-b border-line px-4 py-3 mono">{item.gateway_ip}</td>
            <td class="border-b border-line px-4 py-3">{item.is_system_managed ? 'system' : 'manual'}</td>
            <td class="border-b border-line px-4 py-3"><StateBadge label={item.status} /></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<CreateNetworkModal bind:open={createModalOpen} onSuccess={loadNetworks} />
