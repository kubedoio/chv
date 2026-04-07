<script lang="ts">
  import { onMount } from 'svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import type { StoragePool } from '$lib/api/types';

  const client = createAPIClient({ token: getStoredToken() ?? undefined });
  let items: StoragePool[] = [];

  onMount(async () => {
    items = await client.listStoragePools().catch(() => []);
  });
</script>

<section class="table-card">
  <div class="card-header px-4 py-3">
    <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Storage</div>
    <div class="mt-1 text-lg font-semibold">Localdisk Pools</div>
  </div>

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
        <tr class="odd:bg-white even:bg-[#f8f8f8]">
          <td class="border-b border-line px-4 py-3">{item.name}</td>
          <td class="border-b border-line px-4 py-3">{item.pool_type}</td>
          <td class="border-b border-line px-4 py-3 mono">{item.path}</td>
          <td class="border-b border-line px-4 py-3">{item.is_default ? 'yes' : 'no'}</td>
          <td class="border-b border-line px-4 py-3"><StateBadge label={item.status} /></td>
        </tr>
      {/each}
    </tbody>
  </table>
</section>

