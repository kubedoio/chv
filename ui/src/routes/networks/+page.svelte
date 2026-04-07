<script lang="ts">
  import { onMount } from 'svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import type { Network } from '$lib/api/types';

  const client = createAPIClient({ token: getStoredToken() ?? undefined });
  let items: Network[] = [];

  onMount(async () => {
    items = await client.listNetworks().catch(() => []);
  });
</script>

<section class="table-card">
  <div class="card-header px-4 py-3">
    <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Networks</div>
    <div class="mt-1 text-lg font-semibold">Bridge-backed Networks</div>
  </div>

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
        <tr class="odd:bg-white even:bg-[#f8f8f8]">
          <td class="border-b border-line px-4 py-3">{item.name}</td>
          <td class="border-b border-line px-4 py-3 mono">{item.bridge_name}</td>
          <td class="border-b border-line px-4 py-3 mono">{item.cidr}</td>
          <td class="border-b border-line px-4 py-3 mono">{item.gateway_ip}</td>
          <td class="border-b border-line px-4 py-3">{item.is_system_managed ? 'system' : 'manual'}</td>
          <td class="border-b border-line px-4 py-3"><StateBadge label={item.status} /></td>
        </tr>
      {/each}
    </tbody>
  </table>
</section>

