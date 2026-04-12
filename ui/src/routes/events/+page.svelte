<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Activity, Filter, RefreshCw, Clock } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import SkeletonRow from '$lib/components/SkeletonRow.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { toast } from '$lib/stores/toast';
  import type { Event } from '$lib/api/types';

  const client = createAPIClient({ token: getStoredToken() ?? undefined });

  let events: Event[] = $state([]);
  let loading = $state(true);
  let autoRefresh = $state(true);
  let sinceAppLoad = $state(true);
  let refreshInterval: ReturnType<typeof setInterval> | null = $state(null);
  let newEventCount = $state(0);
  let lastEventCount = $state(0);

  // Filters
  let filterOperation = $state('');
  let filterStatus = $state('');
  let filterResource = $state('');

  const appStartTime = new Date();

  async function loadEvents() {
    try {
      const params = new URLSearchParams();
      if (filterOperation) params.set('operation', filterOperation);
      if (filterStatus) params.set('status', filterStatus);
      if (filterResource) params.set('resource', filterResource);

      const query = params.toString();
      let data = (await client.listEvents(query ? `?${query}` : '')) ?? [];

      if (sinceAppLoad) {
        data = data.filter((e) => new Date(e.timestamp) >= appStartTime);
      }
      
      // Track new events
      if (events.length > 0 && data.length > lastEventCount) {
        newEventCount = data.length - lastEventCount;
      }
      lastEventCount = data.length;
      events = data;
    } catch {
      toast.error('Failed to load events');
    } finally {
      loading = false;
    }
  }

  function startAutoRefresh() {
    refreshInterval = setInterval(loadEvents, 10000); // 10 seconds for faster updates
  }

  function stopAutoRefresh() {
    if (refreshInterval) {
      clearInterval(refreshInterval);
      refreshInterval = null;
    }
  }

  onMount(() => {
    loadEvents();
    if (autoRefresh) startAutoRefresh();
  });

  onDestroy(() => {
    stopAutoRefresh();
  });

  $effect(() => {
    if (autoRefresh && !refreshInterval) {
      startAutoRefresh();
    } else if (!autoRefresh && refreshInterval) {
      stopAutoRefresh();
    }
  });

  // Unique values for filters
  const operations = $derived([...new Set(events.map((e) => e.operation))].sort());
  const statuses = $derived([...new Set(events.map((e) => e.status))].sort());
  const resources = $derived([...new Set(events.map((e) => e.resource))].sort());

  function formatTime(ts: string) {
    return new Date(ts).toLocaleString();
  }

  function clearFilters() {
    filterOperation = '';
    filterStatus = '';
    filterResource = '';
    loadEvents();
  }
</script>

<svelte:head>
  <title>Events | chv</title>
</svelte:head>

<section class="table-card">
  <div class="card-header px-4 py-3">
    <div class="flex items-center justify-between">
      <div>
        <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Operations</div>
        <div class="mt-1 text-lg font-semibold">Event Log</div>
      </div>
      <div class="flex items-center gap-3">
        <label class="flex items-center gap-2 text-sm cursor-pointer">
          <input type="checkbox" bind:checked={autoRefresh} />
          <span class="text-muted">Auto-refresh (10s)</span>
          {#if newEventCount > 0}
            <span class="bg-red-500 text-white text-xs px-2 py-0.5 rounded-full">{newEventCount} new</span>
          {/if}
        </label>
        <label class="flex items-center gap-2 text-sm cursor-pointer">
          <input type="checkbox" bind:checked={sinceAppLoad} />
          <span class="text-muted">Since app load</span>
        </label>
        <button onclick={() => { newEventCount = 0; loadEvents(); }} class="p-2 hover:bg-chrome rounded">
          <RefreshCw size={16} />
        </button>
      </div>
    </div>

    <!-- Filters -->
    <div class="flex items-center gap-3 mt-4">
      <Filter size={16} class="text-muted" />

      <select
        bind:value={filterOperation}
        onchange={loadEvents}
        class="border border-line rounded px-3 py-1.5 text-sm"
      >
        <option value="">All Operations</option>
        {#each operations as op}
          <option value={op}>{op}</option>
        {/each}
      </select>

      <select
        bind:value={filterStatus}
        onchange={loadEvents}
        class="border border-line rounded px-3 py-1.5 text-sm"
      >
        <option value="">All Statuses</option>
        <option value="success">Success</option>
        <option value="failed">Failed</option>
        <option value="pending">Pending</option>
      </select>

      <select
        bind:value={filterResource}
        onchange={loadEvents}
        class="border border-line rounded px-3 py-1.5 text-sm"
      >
        <option value="">All Resources</option>
        {#each resources as res}
          <option value={res}>{res}</option>
        {/each}
      </select>

      {#if filterOperation || filterStatus || filterResource}
        <button onclick={clearFilters} class="text-sm text-accent hover:underline">
          Clear filters
        </button>
      {/if}
    </div>
  </div>

  {#if loading}
    <table class="w-full border-collapse text-sm">
      <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
        <tr>
          <th class="border-b border-line px-4 py-3">Timestamp</th>
          <th class="border-b border-line px-4 py-3">Operation</th>
          <th class="border-b border-line px-4 py-3">Status</th>
          <th class="border-b border-line px-4 py-3">Resource</th>
          <th class="border-b border-line px-4 py-3">Message</th>
        </tr>
      </thead>
      <tbody>
        {#each Array(5) as _}
          <SkeletonRow columns={5} />
        {/each}
      </tbody>
    </table>
  {:else if events.length === 0}
    <EmptyState
      icon={Activity}
      title="No events"
      description={sinceAppLoad
        ? 'No events since app loaded. Events will appear here as operations complete.'
        : 'No events found in the system.'}
    />
  {:else}
    <table class="w-full border-collapse text-sm">
      <thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
        <tr>
          <th class="border-b border-line px-4 py-3">Timestamp</th>
          <th class="border-b border-line px-4 py-3">Operation</th>
          <th class="border-b border-line px-4 py-3">Status</th>
          <th class="border-b border-line px-4 py-3">Resource</th>
          <th class="border-b border-line px-4 py-3">Message</th>
        </tr>
      </thead>
      <tbody>
        {#each events as event}
          <tr class="odd:bg-white even:bg-[#f8f8f8]">
            <td class="border-b border-line px-4 py-3 text-muted whitespace-nowrap">
              <div class="flex items-center gap-2">
                <Clock size={14} />
                {formatTime(event.timestamp)}
              </div>
            </td>
            <td class="border-b border-line px-4 py-3 font-medium">
              {event.operation}
            </td>
            <td class="border-b border-line px-4 py-3">
              <StateBadge label={event.status} />
            </td>
            <td class="border-b border-line px-4 py-3">
              <span class="text-xs bg-chrome px-2 py-1 rounded">
                {event.resource}
              </span>
            </td>
            <td class="border-b border-line px-4 py-3 text-muted">
              {event.message || '-'}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  .table-card {
    @apply bg-white border border-line rounded;
  }
  .card-header {
    @apply border-b border-line;
  }
</style>
