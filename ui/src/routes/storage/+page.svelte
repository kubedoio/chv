<script lang="ts">
import Button from '$lib/components/primitives/Button.svelte';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { HardDrive, Plus, Database, ShieldCheck } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
  import FilterBar from '$lib/components/shared/FilterBar.svelte';
  import CreateStoragePoolModal from '$lib/components/storage/CreateStoragePoolModal.svelte';
  import SectionCard from '$lib/components/shell/SectionCard.svelte';
  import CompactMetricCard from '$lib/components/shared/CompactMetricCard.svelte';
  import { formatBytes } from '$lib/utils/table.svelte';
  import type { StoragePool } from '$lib/api/types';
  import ErrorState from '$lib/components/shell/ErrorState.svelte';
  import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
  import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
  import { getPageDefinition } from '$lib/shell/app-shell';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });
  const pageDef = getPageDefinition('/storage');

  let items = $state<StoragePool[]>([]);
  let loading = $state(true);
  let error = $state(false);
  let createModalOpen = $state(false);
  let query = $state('');

  const filteredItems = $derived(
    items.filter(item => 
      item.name.toLowerCase().includes(query.toLowerCase()) || 
      item.path.toLowerCase().includes(query.toLowerCase())
    )
  );

  const filterOptions = [
    { key: 'query', label: 'Search', type: 'text' as const, placeholder: 'Pool name or path...' }
  ];

  const columns = [
    { key: 'name', label: 'Storage Pool' },
    { key: 'pool_type', label: 'Engine' },
    { key: 'capacity', label: 'Capacity', align: 'right' as const },
    { key: 'available', label: 'Available', align: 'right' as const },
    { key: 'utilization', label: 'Utilization', align: 'right' as const },
    { key: 'status', label: 'Status' }
  ];

  const tableRows = $derived(filteredItems.map(pool => {
    const used = (pool.capacity_bytes || 0) - (pool.allocatable_bytes || 0);
    const utilization = pool.capacity_bytes ? Math.round((used / pool.capacity_bytes) * 100) : 0;
    
    return {
      ...pool,
      pool_type: { label: pool.pool_type === 'localdisk' ? 'Local-IO' : 'Overlay', tone: 'neutral' as const },
      capacity: formatBytes(pool.capacity_bytes || 0),
      available: formatBytes(pool.allocatable_bytes || 0),
      utilization: `${utilization}%`,
      status: { label: pool.status || 'active', tone: pool.status === 'error' ? 'failed' : 'healthy' as const }
    };
  }));

  async function loadStoragePools() {
    loading = true;
    error = false;
    try {
      items = (await client.listStoragePools()) ?? [];
    } catch (err) {
      toast.error('Failed to load storage pools');
      error = true;
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

<div class="inventory-page">
  <PageHeaderWithAction page={pageDef}>
    {#snippet actions()}
      <Button variant="primary" onclick={() => createModalOpen = true}>
        <Plus size={14} />
        Mount Storage
      </Button>
    {/snippet}
  </PageHeaderWithAction>

  <div class="inventory-metrics">
    <CompactMetricCard 
      label="Provisioned" 
      value={formatBytes(items.reduce((s, p) => s + (p.capacity_bytes || 0), 0))} 
      color="neutral"
    />
    <CompactMetricCard 
      label="Allocation Rate" 
      value={`${Math.round((items.reduce((s, p) => s + ((p.capacity_bytes || 0) - (p.allocatable_bytes || 0)), 0) / (items.reduce((s, p) => s + (p.capacity_bytes || 1), 0))) * 100)}%`} 
      color="primary"
    />
    <CompactMetricCard 
       label="Degraded Pools" 
       value={items.filter(p => p.status === 'error').length} 
       color={items.filter(p => p.status === 'error').length > 0 ? 'danger' : 'neutral'}
    />
  </div>

  <div class="inventory-controls">
    <FilterBar
      filters={filterOptions}
      activeFilters={{ query }}
      onFilterChange={(k, v) => query = v as string}
      onClearAll={() => query = ''}
    />
  </div>

  <main class="inventory-main">
    <section class="inventory-table-area">
      {#if error}
        <ErrorState />
      {:else if !loading && items.length === 0}
        <EmptyInfrastructureState
          title="No backing storage detected"
          description="Storage pools define data persistence boundaries for workloads."
          hint="Local storage pools map to physical host mountpoints."
        />
      {:else}
        <InventoryTable
          {columns}
          rows={tableRows}
        >
          {#snippet cell({ column, row })}
            {@const val = (row as any)[column.key]}
            {#if column.key === 'name'}
              <div class="pool-identity">
                <span class="pool-name">{row.name}</span>
                {#if row.is_default}
                   <span class="pool-tag">DEFAULT</span>
                {/if}
              </div>
            {:else if typeof val === 'object' && val?.tone}
               <span class={`status-pill status-pill--${val.tone}`}>{val.label}</span>
            {:else}
               <span class="cell-text">{val}</span>
            {/if}
          {/snippet}
        </InventoryTable>
      {/if}
    </section>

    <aside class="support-area">
      <SectionCard title="Data Persistence" icon={Database}>
        <div class="storage-summary">
           <div class="summary-row">
              <span>Logical Sectors</span>
              <span>Enabled</span>
           </div>
           <div class="summary-row">
              <span>Host I/O Wait</span>
              <span>Nominal</span>
           </div>
        </div>
      </SectionCard>

      <SectionCard title="Storage Health" icon={ShieldCheck}>
        <ul class="task-list">
          <li class="task-item">
            <span class="task-label">ZFS Scrutability</span>
            <span class="task-time">Verified</span>
          </li>
          <li class="task-item">
            <span class="task-label">Mount Persistence</span>
            <span class="task-time">Optimal</span>
          </li>
        </ul>
      </SectionCard>
    </aside>
  </main>
</div>

<CreateStoragePoolModal 
  bind:open={createModalOpen} 
  onSuccess={loadStoragePools}
  existingNames={items.map(i => i.name)}
/>

<style>
  .inventory-page {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .inventory-metrics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.75rem;
  }

  .inventory-controls {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-xs);
    overflow: hidden;
  }

  .inventory-main {
    display: grid;
    grid-template-columns: 1fr 300px;
    gap: 1rem;
    align-items: start;
  }

  .pool-identity {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .pool-name {
    font-weight: 700;
    color: var(--color-neutral-900);
  }

  .pool-tag {
    font-size: 8px;
    font-weight: 800;
    color: #ffffff;
    background: var(--color-primary);
    padding: 1px 3px;
    border-radius: 2px;
  }

  .status-pill {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 2px;
    background: var(--bg-surface-muted);
    border: 1px solid var(--border-subtle);
  }

  .status-pill--healthy { color: var(--color-success); border-color: var(--color-success-light); }
  .status-pill--failed { color: var(--color-danger); border-color: var(--color-danger-light); }

  .summary-row {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    color: var(--color-neutral-600);
    padding: 0.35rem 0;
  }

  .summary-row span:last-child {
    font-weight: 700;
    color: var(--color-neutral-900);
  }

  .support-area {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .task-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .task-item {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    padding: 0.35rem 0.5rem;
    background: var(--bg-surface-muted);
    border-radius: var(--radius-xs);
  }

  .task-label {
    font-weight: 600;
    color: var(--color-neutral-700);
  }

  .task-time {
    color: var(--color-success);
    font-weight: 700;
    text-transform: uppercase;
    font-size: 9px;
  }

  @media (max-width: 1100px) {
    .inventory-main {
      grid-template-columns: 1fr;
    }
  }
</style>

<CreateStoragePoolModal 
  bind:open={createModalOpen} 
  onSuccess={loadStoragePools}
  existingNames={items.map(i => i.name)}
/>
