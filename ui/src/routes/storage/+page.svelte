<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { HardDrive, Plus } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import SkeletonRow from '$lib/components/SkeletonRow.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import CreateStoragePoolModal from '$lib/components/CreateStoragePoolModal.svelte';
  import type { StoragePool } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  let items: StoragePool[] = $state([]);
  let loading = $state(true);
  let createModalOpen = $state(false);

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
</script>

<section class="table-card">
  <div class="card-header px-4 py-3 flex items-center justify-between">
    <div>
      <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Storage</div>
      <div class="mt-1 text-lg font-semibold">Localdisk Pools</div>
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
          <th class="border-b border-line px-4 py-3">Type</th>
          <th class="border-b border-line px-4 py-3">Path</th>
          <th class="border-b border-line px-4 py-3">Default</th>
          <th class="border-b border-line px-4 py-3">Status</th>
        </tr>
      </thead>
      <tbody>
        {#each Array(5) as _}
          <SkeletonRow columns={5} />
        {/each}
      </tbody>
    </table>
  {:else if items.length === 0}
    <EmptyState
      icon={HardDrive}
      title="No storage pools yet"
      description="Create a storage pool to store VM disks"
    >
      <button
        onclick={() => createModalOpen = true}
        class="px-4 py-2 rounded bg-primary text-white font-medium text-sm hover:bg-primary/90 transition-colors flex items-center gap-2"
      >
        <Plus size={16} />
        Create Pool
      </button>
    </EmptyState>
  {:else}
    <table class="w-full border-collapse text-sm">
      <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
        <tr>
          <th class="border-b border-line px-4 py-3">Name</th>
          <th class="border-b border-line px-4 py-3">Type</th>
          <th class="border-b border-line px-4 py-3">Path</th>
          <th class="border-b border-line px-4 py-3">Default</th>
          <th class="border-b border-line px-4 py-3">Status</th>
        </tr>
      </thead>
      <tbody>
        {#each items as item}
          <tr class="odd:bg-white even:bg-[#f8f8f8] hover:bg-hover transition-colors">
            <td class="border-b border-line px-4 py-3 font-medium">{item.name}</td>
            <td class="border-b border-line px-4 py-3">{item.pool_type}</td>
            <td class="border-b border-line px-4 py-3 mono">{item.path}</td>
            <td class="border-b border-line px-4 py-3">{item.is_default ? 'yes' : 'no'}</td>
            <td class="border-b border-line px-4 py-3"><StateBadge label={item.status} /></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<CreateStoragePoolModal 
  bind:open={createModalOpen} 
  onSuccess={loadStoragePools} 
  existingNames={items.map(i => i.name)}
/>
