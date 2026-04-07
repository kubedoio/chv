<script lang="ts">
  import { onMount } from 'svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import type { VM } from '$lib/api/types';

  const client = createAPIClient({ token: getStoredToken() ?? undefined });
  let items: VM[] = [];

  onMount(async () => {
    items = await client.listVMs().catch(() => []);
  });
</script>

<section class="table-card">
  <div class="card-header px-4 py-3">
    <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Virtual Machines</div>
    <div class="mt-1 text-lg font-semibold">Desired vs Actual Runtime State</div>
  </div>

  <table class="w-full border-collapse text-sm">
    <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
      <tr>
        <th class="border-b border-line px-4 py-3">Name</th>
        <th class="border-b border-line px-4 py-3">Desired</th>
        <th class="border-b border-line px-4 py-3">Actual</th>
        <th class="border-b border-line px-4 py-3">CPU</th>
        <th class="border-b border-line px-4 py-3">Memory</th>
        <th class="border-b border-line px-4 py-3">IP</th>
        <th class="border-b border-line px-4 py-3">Error</th>
      </tr>
    </thead>
    <tbody>
      {#each items as item}
        <tr class="odd:bg-white even:bg-[#f8f8f8]">
          <td class="border-b border-line px-4 py-3">
            <a class="text-primary no-underline" href={`/vms/${item.id}`}>{item.name}</a>
          </td>
          <td class="border-b border-line px-4 py-3"><StateBadge label={item.desired_state} /></td>
          <td class="border-b border-line px-4 py-3"><StateBadge label={item.actual_state} /></td>
          <td class="border-b border-line px-4 py-3 mono">{item.vcpu}</td>
          <td class="border-b border-line px-4 py-3 mono">{item.memory_mb} MB</td>
          <td class="border-b border-line px-4 py-3 mono">{item.ip_address || 'pending'}</td>
          <td class="border-b border-line px-4 py-3">{item.last_error || '—'}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</section>

