<script lang="ts">
  import { onMount } from 'svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import type { Operation } from '$lib/api/types';

  const client = createAPIClient({ token: getStoredToken() ?? undefined });
  let items: Operation[] = [];

  onMount(async () => {
    items = await client.listOperations().catch(() => []);
  });
</script>

<section class="table-card">
  <div class="card-header px-4 py-3">
    <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Operations</div>
    <div class="mt-1 text-lg font-semibold">Auditable Change Log</div>
  </div>

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
</section>

