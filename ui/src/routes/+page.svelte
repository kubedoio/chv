<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { 
    Server, 
    Image as ImageIcon, 
    HardDrive, 
    Network, 
    Activity,
    CheckCircle,
    AlertCircle,
    Loader2
  } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import StatsCard from '$lib/components/StatsCard.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import { toast } from '$lib/stores/toast';
  import type { VM, Image, StoragePool, Network as NetworkType, Event } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });

  let vms = $state<VM[]>([]);
  let images = $state<Image[]>([]);
  let pools = $state<StoragePool[]>([]);
  let networks = $state<NetworkType[]>([]);
  let events = $state<Event[]>([]);
  let installState = $state<string>('unknown');
  let loading = $state(true);
  let pollInterval: ReturnType<typeof setInterval> | null = $state(null);

  // Derived stats
  const runningVMs = $derived(vms.filter(v => v.actual_state === 'running').length);
  const stoppedVMs = $derived(vms.filter(v => v.actual_state === 'stopped').length);
  const importingImages = $derived(images.filter(i => i.status === 'importing').length);
  const readyImages = $derived(images.filter(i => i.status === 'ready').length);
  const recentEvents = $derived(events.slice(0, 5));

  async function loadData() {
    try {
      const [vmsData, imagesData, poolsData, networksData, eventsData, installData] = await Promise.all([
        client.listVMs(),
        client.listImages(),
        client.listStoragePools(),
        client.listNetworks(),
        client.listEvents(),
        client.getInstallStatus()
      ]);
      vms = vmsData;
      images = imagesData;
      pools = poolsData;
      networks = networksData;
      events = eventsData;
      installState = installData.overall_state;
    } catch (e) {
      console.error('Failed to load dashboard data:', e);
    } finally {
      loading = false;
    }
  }

  function startPolling() {
    pollInterval = setInterval(loadData, 10000); // 10 seconds
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  onMount(() => {
    if (!token) {
      goto('/login');
      return;
    }
    loadData();
    startPolling();
  });

  onDestroy(() => {
    stopPolling();
  });

  function formatTime(ts: string) {
    return new Date(ts).toLocaleTimeString();
  }
</script>

<svelte:head>
  <title>Dashboard | chv</title>
</svelte:head>

{#if loading}
  <div class="flex items-center justify-center h-64">
    <div class="text-muted">Loading...</div>
  </div>
{:else}
  <div class="space-y-6">
    <!-- System Status -->
    <section class="grid gap-4 lg:grid-cols-4">
      <StatsCard 
        title="Virtual Machines" 
        value={vms.length} 
        icon={Server}
        subtitle="{runningVMs} running, {stoppedVMs} stopped"
        href="/vms"
      />
      <StatsCard 
        title="Images" 
        value={images.length} 
        icon={ImageIcon}
        subtitle="{readyImages} ready, {importingImages} importing"
        href="/images"
      />
      <StatsCard 
        title="Storage Pools" 
        value={pools.length} 
        icon={HardDrive}
        subtitle="{pools.filter(p => p.status === 'ready').length} ready"
        href="/storage"
      />
      <StatsCard 
        title="Networks" 
        value={networks.length} 
        icon={Network}
        subtitle="{networks.filter(n => n.status === 'active').length} active"
        href="/networks"
      />
    </section>

    <!-- Quick Actions -->
    <section class="grid gap-4 lg:grid-cols-3">
      <a class="table-card block p-5 text-inherit no-underline hover:shadow-md transition-shadow" href="/install">
        <div class="flex items-center justify-between mb-2">
          <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Platform</div>
          {#if installState === 'ready'}
            <CheckCircle class="text-green-600" size={20} />
          {:else if installState === 'bootstrap_required'}
            <AlertCircle class="text-yellow-600" size={20} />
          {:else}
            <Loader2 class="text-blue-600 animate-spin" size={20} />
          {/if}
        </div>
        <div class="mt-2 text-lg font-semibold">Install and Repair</div>
        <p class="mt-2 text-sm text-muted">Status: <span class="capitalize">{installState.replace('_', ' ')}</span></p>
        <p class="mt-1 text-sm text-muted">Inspect `/var/lib/chv`, `chvbr0`, SQLite, and the default `localdisk` pool.</p>
      </a>

      <a class="table-card block p-5 text-inherit no-underline hover:shadow-md transition-shadow" href="/images">
        <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Templates</div>
        <div class="mt-2 text-lg font-semibold">QCOW2 Images</div>
        <p class="mt-2 text-sm text-muted">Track imported cloud images and their cloud-init readiness.</p>
      </a>

      <a class="table-card block p-5 text-inherit no-underline hover:shadow-md transition-shadow" href="/vms">
        <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Workloads</div>
        <div class="mt-2 text-lg font-semibold">Virtual Machines</div>
        <p class="mt-2 text-sm text-muted">View the desired and actual state for each VM workspace.</p>
      </a>
    </section>

    <!-- Recent Events -->
    <section class="table-card">
      <div class="card-header px-4 py-3 flex justify-between items-center">
        <div>
          <div class="text-[11px] uppercase tracking-[0.16em] text-muted">Activity</div>
          <div class="mt-1 text-lg font-semibold">Recent Events</div>
        </div>
        <a href="/events" class="text-sm text-accent hover:underline">View all</a>
      </div>
      
      {#if recentEvents.length === 0}
        <div class="p-8 text-center text-muted">
          <Activity size={32} class="mx-auto mb-2 opacity-50" />
          <p>No recent events</p>
        </div>
      {:else}
        <div class="divide-y divide-line">
          {#each recentEvents as event}
            <div class="px-4 py-3 flex items-center justify-between hover:bg-chrome/50">
              <div class="flex items-center gap-3">
                <StateBadge label={event.status} />
                <span class="text-sm font-medium capitalize">{event.operation}</span>
                <span class="text-xs text-muted">{event.resource}</span>
              </div>
              <span class="text-xs text-muted">{formatTime(event.timestamp)}</span>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </div>
{/if}

<style>
  .table-card {
    @apply bg-white border border-line rounded;
  }
  .card-header {
    @apply border-b border-line;
  }
</style>
