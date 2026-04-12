<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { History } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import SkeletonRow from '$lib/components/SkeletonRow.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import type { Operation } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  let items: Operation[] = $state([]);
  let loading = $state(true);

  async function loadOperations() {
    loading = true;
    try {
      items = (await client.listOperations()) ?? [];
    } catch (err) {
      toast.error('Failed to load operations');
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
    loadOperations();
  });
</script>

<section class="table-card">
  <div class="card-header px-4 py-3">
    <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Operations</div>
    <div class="mt-1 text-lg font-semibold">Auditable Change Log</div>
  </div>

  {#if loading}
    <table class="w-full border-collapse text-sm">
      <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
        <tr>
          <th class="border-b border-line px-4 py-3">Resource</th>
          <th class="border-b border-line px-4 py-3">Operation</th>
          <th class="border-b border-line px-4 py-3">State</th>
          <th class="border-b border-line px-4 py-3">Created</th>
        </tr>
      </thead>
      <tbody>
        {#each Array(5) as _}
          <SkeletonRow columns={4} />
        {/each}
      </tbody>
    </table>
  {:else if items.length === 0}
    <EmptyState
      icon={History}
      title="No operations yet"
      description="Recent operations will appear here"
    />
  {:else}
    <table class="w-full border-collapse text-sm">
      <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
        <tr>
          <th class="border-b border-line px-4 py-3">Resource</th>
          <th class="border-b border-line px-4 py-3">Operation</th>
          <th class="border-b border-line px-4 py-3">State</th>
          <th class="border-b border-line px-4 py-3">Created</th>
        </tr>
      </thead>
      <tbody>
        {#each items as item}
          <tr class="odd:bg-white even:bg-[#f8f8f8]">
            <td class="border-b border-line px-4 py-3">{item.resource_type}:{item.resource_id}</td>
            <td class="border-b border-line px-4 py-3">{item.operation_type}</td>
            <td class="border-b border-line px-4 py-3"><StateBadge label={item.state} /></td>
            <td class="border-b border-line px-4 py-3 mono">{item.created_at}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>
