<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/stores';
  import { createAPIClient } from '$lib/api/client';

  export let currentPath = '/';

  const items = [
    { href: '/', label: 'Overview' },
    { href: '/install', label: 'Install' },
    { href: '/networks', label: 'Networks' },
    { href: '/storage', label: 'Storage' },
    { href: '/images', label: 'Images' },
    { href: '/vms', label: 'Virtual Machines' },
    { href: '/operations', label: 'Operations' },
    { href: '/events', label: 'Events' },
    { href: '/settings', label: 'Settings' }
  ];

  const client = createAPIClient();
  let newEvents = 0;
  let lastEventCheck = new Date();
  let pollInterval: ReturnType<typeof setInterval>;

  onMount(() => {
    // Check immediately
    checkNewEvents();

    // Poll every 30 seconds
    pollInterval = setInterval(checkNewEvents, 30000);

    return () => {
      if (pollInterval) clearInterval(pollInterval);
    };
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
  });

  async function checkNewEvents() {
    try {
      const events = await client.listEvents();
      // Count events newer than last check
      newEvents = events.filter(e => new Date(e.timestamp) > lastEventCheck).length;
    } catch (err) {
      console.error('Failed to check events:', err);
    }
  }

  function clearBadge() {
    newEvents = 0;
    lastEventCheck = new Date();
  }

  // Clear badge when navigating to events page
  $: if ($page?.url?.pathname === '/events') {
    clearBadge();
  }
</script>

<aside class="border-r border-line bg-chrome">
  <div class="border-b border-line px-5 py-4">
    <div class="text-[11px] uppercase tracking-[0.2em] text-muted">CHV</div>
    <div class="mt-2 text-lg font-semibold text-ink">Operator Console</div>
    <div class="mt-1 text-sm text-muted">Cloud Hypervisor MVP-1</div>
  </div>

  <nav class="p-3">
    {#each items as item}
      <a
        class={`mb-1 flex items-center border px-3 py-2 text-sm no-underline transition ${
          currentPath === item.href ? 'border-primary bg-selected text-ink' : 'border-transparent text-muted hover:border-line hover:bg-white hover:text-ink'
        }`}
        href={item.href}
        on:click={() => item.href === '/events' && clearBadge()}
      >
        <span>{item.label}</span>
        {#if item.href === '/events' && newEvents > 0}
          <span class="badge">{newEvents}</span>
        {/if}
      </a>
    {/each}
  </nav>
</aside>

<style>
  .badge {
    background-color: #E60000;
    color: white;
    border-radius: 50%;
    padding: 2px 6px;
    font-size: 11px;
    font-weight: bold;
    margin-left: auto;
    min-width: 18px;
    text-align: center;
  }
</style>
