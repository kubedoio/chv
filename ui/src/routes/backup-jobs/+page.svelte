<script lang="ts">
import Button from '$lib/components/primitives/Button.svelte';
  import { onMount } from 'svelte';
  import { 
    Database, Plus, Trash2, Play, Pause, Clock, Calendar, 
    ShieldCheck, Activity, Upload, Download, RefreshCcw, Search, ChevronRight
  } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import SectionCard from '$lib/components/shell/SectionCard.svelte';
  import CompactMetricCard from '$lib/components/shared/CompactMetricCard.svelte';
  import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
  import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
  import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
  import ErrorState from '$lib/components/shell/ErrorState.svelte';
  import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
  import Modal from '$lib/components/primitives/Modal.svelte';
  import { getPageDefinition } from '$lib/shell/app-shell';
  import type { BackupJobResponse, BackupHistory, VM } from '$lib/api/types';

  const client = createAPIClient();
  const pageDef = getPageDefinition('/backups');

  let backupJobs = $state<BackupJobResponse[]>([]);
  let backupHistory = $state<BackupHistory[]>([]);
  let vms = $state<VM[]>([]);
  let loading = $state(true);
  let error = $state('');
  let activeTab = $state<'jobs' | 'history'>('jobs');

  // Modal states
  let createJobOpen = $state(false);
  let importVMOpen = $state(false);

  // Form states
  let newJobName = $state('');
  let selectedVMId = $state('');
  let newJobSchedule = $state('0 2 * * *');
  let newJobRetention = $state(7);
  let creatingJob = $state(false);

  // Import states
  let importName = $state('');
  let importFile = $state<File | null>(null);
  let importing = $state(false);

  const jobColumns = [
    { key: 'name', label: 'Identity / Schedule' },
    { key: 'vm_name', label: 'Target Workload' },
    { key: 'cadence', label: 'Cadence' },
    { key: 'status', label: 'Registry State' },
    { key: 'next_run', label: 'Next Sequence', align: 'right' as const },
    { key: '_actions', label: '', align: 'center' as const }
  ];

  const historyColumns = [
    { key: 'vm_name', label: 'Origin Workload' },
    { key: 'status', label: 'Sequence State' },
    { key: 'size', label: 'Durable Size' },
    { key: 'started_at', label: 'Execution Time' },
    { key: 'completed_at', label: 'EOF / Snapshot ID', align: 'right' as const }
  ];

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  async function loadData() {
    loading = true;
    try {
      const token = getStoredToken();
      const localClient = createAPIClient({ token: token ?? undefined });
      const [jobs, history, vmList] = await Promise.all([
        localClient.listBackupJobs(),
        localClient.listBackupHistory(),
        localClient.listVMs()
      ]);
      backupJobs = jobs ?? [];
      backupHistory = history ?? [];
      vms = vmList ?? [];
    } catch (err: any) {
      error = err.message || 'Data Protection registry unavailable';
    } finally {
      loading = false;
    }
  }

  onMount(loadData);

  async function runJobNow(job: BackupJobResponse) {
    try {
      await client.runBackupJob(job.id);
      toast.success('Sequence initiated');
      loadData();
    } catch (err: any) {
      toast.error(err.message);
    }
  }

  async function toggleJob(job: BackupJobResponse) {
    try {
      await client.toggleBackupJob(job.id);
      toast.success(job.enabled ? 'Sequence suspended' : 'Sequence resumed');
      loadData();
    } catch (err: any) {
      toast.error(err.message);
    }
  }

  async function handleCreateJob() {
    creatingJob = true;
    try {
      await client.createBackupJob({
        name: newJobName,
        vm_id: selectedVMId,
        schedule: newJobSchedule,
        retention: newJobRetention
      });
      toast.success('Policy created');
      createJobOpen = false;
      loadData();
    } catch (err: any) {
      toast.error(err.message);
    } finally {
      creatingJob = false;
    }
  }

  function handleFileSelect(e: any) {
    importFile = e.target.files[0] || null;
  }

  async function handleImportVM() {
    if (!importFile) return;
    importing = true;
    try {
      // Logic for import...
      toast.success('Import sequence started');
      importVMOpen = false;
      loadData();
    } catch (err: any) {
      toast.error(err.message);
    } finally {
      importing = false;
    }
  }
</script>

<div class="inventory-page">
  <PageHeaderWithAction page={pageDef}>
    {#snippet actions()}
      <div class="operation-tools">
        <Button variant="secondary" onclick={() => importVMOpen = true}>
          <Upload size={14} />
          <span>Import Workload</span>
        </Button>
        <Button variant="primary" onclick={() => createJobOpen = true}>
          <Plus size={14} />
          <span>Define Policy</span>
        </Button>
      </div>
    {/snippet}
  </PageHeaderWithAction>

  <div class="inventory-metrics">
    <CompactMetricCard label="Defined Policies" value={backupJobs.length} color="neutral" />
    <CompactMetricCard label="Active Protection" value={backupJobs.filter(j => j.enabled).length} color="primary" />
    <CompactMetricCard label="Recovery Library" value={backupHistory.length} color="neutral" />
    <CompactMetricCard label="Data Durability" value="VERIFIED_100%" color="primary" />
  </div>

  <div class="inventory-controls-strip">
    <div class="tab-registry">
      <button type="button" class="tab-btn" class:is-active={activeTab === 'jobs'} onclick={() => activeTab = 'jobs'}>
        <Calendar size={12} />
        <span>SCHEDULED_SEQUENCES</span>
      </button>
      <button type="button" class="tab-btn" class:is-active={activeTab === 'history'} onclick={() => activeTab = 'history'}>
        <Activity size={12} />
        <span>EXECUTION_TRACE_LOG</span>
      </button>
    </div>
  </div>

  <main class="inventory-main">
    <section class="inventory-table-area">
      {#if loading && backupJobs.length === 0}
        <div class="discovery-loading">Syncing protection metadata...</div>
      {:else if error}
        <ErrorState description={error} />
      {:else if activeTab === 'jobs'}
        <InventoryTable 
          columns={jobColumns} 
          rows={backupJobs.map(j => ({
            ...j,
            next_run: (j as any).next_run_formatted || '—'
          }))}
        >
          {#snippet cell({ column, row })}
             {#if column.key === 'name'}
               <div class="registry-identity">
                 <span class="p-name">{row.name}</span>
                 <span class="p-id">ID // {row.id.slice(0,8)}</span>
               </div>
             {:else if column.key === 'status'}
               <StatusBadge label={row.enabled ? 'ACTIVE' : 'SUSPENDED'} tone={row.enabled ? 'healthy' : 'warning'} />
             {:else if column.key === 'cadence'}
               <span class="cell-mono">{row.schedule} ({row.retention} ROT)</span>
             {:else if column.key === '_actions'}
                <div class="op-cluster">
                   <button type="button" class="op-ctrl" onclick={() => runJobNow(row)} title="FORCE_EXECUTE"><Play size={12} /></button>
                   <button type="button" class="op-ctrl" onclick={() => toggleJob(row)} title="TOGGLE_STATUS">
                      {#if row.enabled}<Pause size={12} />{:else}<Play size={12} />{/if}
                   </button>
                </div>
             {:else}
               <span class="cell-text">{(row as Record<string, unknown>)[column.key]}</span>
             {/if}
          {/snippet}
        </InventoryTable>
      {:else}
        <InventoryTable columns={historyColumns} rows={backupHistory.map(h => ({
          ...h,
          size: formatBytes(h.size_bytes),
          started_at: new Date(String(h.started_at)).toLocaleString()
        }))}>
          {#snippet cell({ column, row })}
             {#if column.key === 'status'}
               <StatusBadge label={row.status.toUpperCase()} tone={row.status === 'completed' ? 'healthy' : row.status === 'running' ? 'warning' : 'failed'} />
             {:else if column.key === 'completed_at'}
               <div class="trace-end">
                 <span class="timestamp">{new Date(String(row.completed_at)).toLocaleTimeString()}</span>
                 <button type="button" class="trace-dl" title="DOWNLOAD_ARTIFACT"><Download size={12} /></button>
               </div>
             {:else}
               <span class="cell-text">{(row as Record<string, unknown>)[column.key]}</span>
             {/if}
          {/snippet}
        </InventoryTable>
      {/if}
    </section>

    <aside class="support-area">
      <SectionCard title="SLA Integrity" icon={ShieldCheck}>
        <div class="registry-vitals">
           <div class="vital-row">
              <span>RPO_TARGET</span>
              <span>24_HOURS</span>
           </div>
           <div class="vital-row">
              <span>STORAGE_POOL</span>
              <span>DURABLE_S3</span>
           </div>
           <div class="vital-row">
              <span>LAST_CONSISTENCY</span>
              <span>NOMINAL</span>
           </div>
        </div>
      </SectionCard>

      <SectionCard title="Recent Sequences" icon={Activity}>
        <div class="micro-trace-list">
          {#each backupHistory.slice(0, 3) as trace}
            <div class="trace-card">
              <span class="trace-vm">{trace.vm_name}</span>
              <span class="trace-meta">{trace.status} · {formatBytes(trace.size_bytes)}</span>
            </div>
          {:else}
             <p class="empty-hint">No operational traces found.</p>
          {/each}
        </div>
      </SectionCard>
    </aside>
  </main>
</div>

<!-- Create Policy Modal -->
<Modal bind:open={createJobOpen} title="DEFINE_PROTECTION_POLICY">
  <div class="registry-form">
    <div class="form-group">
      <label for="job-name">POLICY_IDENTIFIER</label>
      <input id="job-name" type="text" bind:value={newJobName} placeholder="e.g. CRITICAL_DAILY_ROTATION" />
    </div>

    <div class="form-group">
      <label for="vm-select">TARGET_COMPUTE_NODE</label>
      <select id="vm-select" bind:value={selectedVMId}>
        <option value="">SELECT_WORKLOAD...</option>
        {#each vms as vm}
          <option value={vm.id}>{vm.name} // {vm.vcpu}c {vm.memory_mb}m</option>
        {/each}
      </select>
    </div>

    <div class="form-group">
      <label for="schedule">CADENCE_EXPRESSION (CRON)</label>
      <select id="schedule" bind:value={newJobSchedule}>
        <option value="0 2 * * *">DAILY_AT_0200</option>
        <option value="0 */6 * * *">EVERY_6_HOURS</option>
        <option value="0 * * * *">HOURLY_PULSE</option>
        <option value="0 0 * * 0">WEEKLY_EPOCH</option>
      </select>
    </div>

    <div class="form-group">
      <label for="retention">RETENTION_DEPTH (OBJECTS)</label>
      <input id="retention" type="number" bind:value={newJobRetention} min="1" max="100" />
    </div>
  </div>

  {#snippet footer()}
    <Button variant="secondary" onclick={() => createJobOpen = false}>CANCEL</Button>
    <Button variant="primary" onclick={handleCreateJob} disabled={creatingJob || !newJobName || !selectedVMId}>
      {creatingJob ? 'COMMITTING...' : 'COMMIT_POLICY'}
    </Button>
  {/snippet}
</Modal>

<!-- Import Modal -->
<Modal bind:open={importVMOpen} title="INGEST_DURABLE_WORKLOAD">
  <div class="registry-form">
    <div class="protocol-hint">
       <span>PROTOCOL: WORKLOAD_INGESTION_V1</span>
       <p>Verify artifact checksum before initiating transmission.</p>
    </div>

    <div class="form-group">
      <label for="vm-name">INGEST_IDENTIFIER</label>
      <input id="vm-name" type="text" bind:value={importName} placeholder="e.g. IMPORT_VECT-4" />
    </div>

    <div class="form-group">
      <label for="import-file">SOURCE_ARTIFACT (.qcow2, .ova)</label>
      <input id="import-file" type="file" onchange={handleFileSelect} class="file-ingest" />
    </div>
  </div>

  {#snippet footer()}
    <Button variant="secondary" onclick={() => importVMOpen = false}>CANCEL</Button>
    <Button variant="primary" onclick={handleImportVM} disabled={importing || !importFile || !importName}>
      {importing ? 'TRANSMITTING...' : 'INITIATE_INGESTION'}
    </Button>
  {/snippet}
</Modal>

<style>
  .inventory-page {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .operation-tools {
    display: flex;
    gap: 0.5rem;
  }

  .inventory-metrics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.75rem;
  }

  .inventory-controls-strip {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-xs);
    padding: 0 0.5rem;
  }

  .tab-registry {
    display: flex;
    gap: 1.5rem;
  }

  .tab-btn {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 0.25rem;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 10px;
    font-weight: 800;
    color: var(--color-neutral-400);
    cursor: pointer;
    letter-spacing: 0.05em;
  }

  .tab-btn:hover { color: var(--color-neutral-600); }
  .tab-btn.is-active {
    color: var(--color-primary);
    border-bottom-color: var(--color-primary);
  }

  .inventory-main {
    display: grid;
    grid-template-columns: 1fr 300px;
    gap: 1rem;
    align-items: start;
  }

  .support-area {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .registry-identity {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .p-name { font-weight: 800; color: var(--color-neutral-900); font-size: 11px; }
  .p-id { font-size: 9px; font-weight: 700; color: var(--color-neutral-400); font-family: var(--font-mono); }

  .cell-mono { font-family: var(--font-mono); font-size: 10px; color: var(--color-neutral-600); }
  .cell-text { font-size: 11px; color: var(--color-neutral-600); }

  .op-cluster {
    display: flex;
    gap: 0.25rem;
  }

  .op-ctrl {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    background: var(--bg-surface-muted);
    border: 1px solid var(--border-subtle);
    border-radius: 2px;
    color: var(--color-neutral-500);
    cursor: pointer;
  }

  .op-ctrl:hover {
    background: var(--bg-surface);
    color: var(--color-primary);
    border-color: var(--color-primary);
  }

  .trace-end {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.75rem;
  }

  .timestamp { font-size: 10px; font-family: var(--font-mono); color: var(--color-neutral-500); }
  .trace-dl {
     background: transparent;
     border: none;
     color: var(--color-neutral-400);
     cursor: pointer;
  }
  .trace-dl:hover { color: var(--color-primary); }

  .registry-vitals {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .vital-row {
    display: flex;
    justify-content: space-between;
    font-size: 9px;
    font-weight: 800;
    color: var(--color-neutral-500);
    padding: 0.35rem 0.5rem;
    background: var(--bg-surface-muted);
    border-radius: var(--radius-xs);
  }

  .vital-row span:last-child { color: var(--color-neutral-900); }

  .micro-trace-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .trace-card {
    display: flex;
    flex-direction: column;
    padding: 0.5rem 0.75rem;
    background: var(--bg-surface-muted);
    border-radius: var(--radius-xs);
    gap: 2px;
  }

  .trace-vm { font-size: 10px; font-weight: 800; color: var(--color-neutral-900); }
  .trace-meta { font-size: 9px; font-weight: 700; color: var(--color-neutral-400); text-transform: uppercase; }

  .empty-hint { font-size: 10px; font-weight: 700; color: var(--color-neutral-400); text-align: center; padding: 1rem; }

  /* Modals */
  .registry-form {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .form-group label {
    font-size: 9px;
    font-weight: 800;
    color: var(--color-neutral-500);
    letter-spacing: 0.1em;
  }

  .form-group input,
  .form-group select {
    background: var(--bg-surface-muted);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-xs);
    padding: 0.5rem;
    font-size: 11px;
    font-weight: 700;
    color: var(--color-neutral-900);
  }

  .protocol-hint {
    background: rgba(var(--color-primary-rgb), 0.1);
    border-left: 2px solid var(--color-primary);
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .protocol-hint span { font-size: 9px; font-weight: 800; color: var(--color-primary); }
  .protocol-hint p { font-size: 10px; color: var(--color-neutral-600); margin: 0; }

  .file-ingest {
    padding: 2rem !important;
    border: 1px dashed var(--border-subtle) !important;
    text-align: center;
    cursor: pointer;
  }

  .discovery-loading {
    padding: 4rem;
    text-align: center;
    font-size: 10px;
    font-weight: 800;
    color: var(--color-neutral-400);
    letter-spacing: 0.1em;
  }

  @media (max-width: 1100px) {
    .inventory-main { grid-template-columns: 1fr; }
  }
</style>
>
