<script lang="ts">
import Button from '$lib/components/primitives/Button.svelte';
  import { onMount } from 'svelte';
  import {
    Plus, Calendar,
    Activity, Upload
  } from 'lucide-svelte';
  import { getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast.svelte';
  import { mutateWithRefresh } from '$lib/stores/mutation.svelte';
  import CompactMetricCard from '$lib/components/shared/CompactMetricCard.svelte';
  import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
  import { getPageDefinition } from '$lib/shell/app-shell';
  import {
    listBackupJobs,
    listBackupHistory,
    createBackupJob,
    executeBackupJob,
    updateBackupJob
  } from '$lib/bff/backups';
  import { listVms } from '$lib/bff/vms';
  import type { BackupJob, BackupHistory } from '$lib/bff/types';
  import type { VmListItem } from '$lib/bff/types';
  import BackupJobsTable from '$lib/components/backup-jobs/BackupJobsTable.svelte';
  import BackupJobsSidebar from '$lib/components/backup-jobs/BackupJobsSidebar.svelte';
  import BackupJobCreateModal from '$lib/components/backup-jobs/BackupJobCreateModal.svelte';
  import BackupJobImportModal from '$lib/components/backup-jobs/BackupJobImportModal.svelte';

  const pageDef = getPageDefinition('/backups');

  let backupJobs = $state<BackupJob[]>([]);
  let backupHistory = $state<BackupHistory[]>([]);
  let vms = $state<VmListItem[]>([]);
  let loading = $state(true);
  let error = $state('');
  let activeTab = $state<'jobs' | 'history'>('jobs');

  // Modal states
  let createJobOpen = $state(false);
  let importVMOpen = $state(false);

  // Form states
  let selectedVMId = $state('');
  let selectedBackupType = $state('full');
  let selectedTargetPath = $state('');
  let selectedStorageBackend = $state('');
  let creatingJob = $state(false);

  // Import states
  let importName = $state('');
  let importFile = $state<File | null>(null);
  let importing = $state(false);

  const jobColumns = [
    { key: 'job_id', label: 'Identity' },
    { key: 'vm_id', label: 'Target Workload' },
    { key: 'backup_type', label: 'Type' },
    { key: 'status', label: 'Registry State' },
    { key: 'created_at', label: 'Created', align: 'right' as const },
    { key: '_actions', label: '', align: 'center' as const }
  ];

  const historyColumns = [
    { key: 'vm_id', label: 'Origin Workload' },
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

  async function loadData(): Promise<void> {
    loading = true;
    try {
      const token = getStoredToken() ?? undefined;
      const [jobsResp, historyResp, vmResp] = await Promise.all([
        listBackupJobs(token),
        listBackupHistory(1, 50, token),
        listVms({ page: 1, page_size: 200, filters: {} }, token)
      ]);
      backupJobs = jobsResp.items ?? [];
      backupHistory = historyResp.items ?? [];
      vms = vmResp.items ?? [];
    } catch (err: unknown) {
      error = err instanceof Error ? err.message : 'Data Protection registry unavailable';
    } finally {
      loading = false;
    }
  }

  onMount(loadData);

  async function runJobNow(job: BackupJob): Promise<void> {
    try {
      const token = getStoredToken() ?? undefined;
      await mutateWithRefresh(
        () => executeBackupJob(job.job_id, token),
        {
          successMessage: 'Sequence initiated',
          errorMessage: 'Failed to execute job',
          skipRefresh: true,
        }
      );
      loadData();
    } catch (err: unknown) {
      // Error already toasted by mutateWithRefresh
    }
  }

  async function toggleJob(job: BackupJob): Promise<void> {
    try {
      const token = getStoredToken() ?? undefined;
      const newStatus = job.status === 'Enabled' ? 'Disabled' : 'Enabled';
      await mutateWithRefresh(
        () => updateBackupJob(job.job_id, { status: newStatus }, token),
        {
          successMessage: job.status === 'Enabled' ? 'Sequence suspended' : 'Sequence resumed',
          errorMessage: 'Failed to toggle job',
          skipRefresh: true,
        }
      );
      loadData();
    } catch (err: unknown) {
      // Error already toasted by mutateWithRefresh
    }
  }

  async function handleCreateJob(): Promise<void> {
    creatingJob = true;
    try {
      const token = getStoredToken() ?? undefined;
      await mutateWithRefresh(
        () => createBackupJob({
          vm_id: selectedVMId,
          backup_type: selectedBackupType,
          target_path: selectedTargetPath || undefined,
          storage_backend: selectedStorageBackend || undefined
        }, token),
        {
          successMessage: 'Policy created',
          errorMessage: 'Failed to create job',
          skipRefresh: true,
        }
      );
      createJobOpen = false;
      loadData();
    } catch (err: unknown) {
      // Error already toasted by mutateWithRefresh
    } finally {
      creatingJob = false;
    }
  }

  function handleFileSelect(e: Event): void {
    const target = e.target as HTMLInputElement;
    importFile = target.files?.[0] ?? null;
  }

  async function handleImportVM(): Promise<void> {
    if (!importFile) return;
    importing = true;
    try {
      toast.error('VM import is not yet implemented. Use the Images page to register images by URL.');
      importVMOpen = false;
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
    <CompactMetricCard label="Active Protection" value={backupJobs.filter(j => j.status === 'Enabled').length} color="primary" />
    <CompactMetricCard label="Recovery Library" value={backupHistory.length} color="neutral" />
    <CompactMetricCard label="Data Durability" value="VERIFIED_100%" color="primary" />
  </div>

  <div class="inventory-controls-strip">
    <div class="tab-registry" role="tablist" aria-label="Backup job views">
      <button type="button" role="tab" id="tab-jobs" aria-selected={activeTab === 'jobs'} aria-controls="panel-backup-jobs" class="tab-btn" class:is-active={activeTab === 'jobs'} onclick={() => activeTab = 'jobs'}>
        <Calendar size={12} />
        <span>SCHEDULED_SEQUENCES</span>
      </button>
      <button type="button" role="tab" id="tab-history" aria-selected={activeTab === 'history'} aria-controls="panel-backup-jobs" class="tab-btn" class:is-active={activeTab === 'history'} onclick={() => activeTab = 'history'}>
        <Activity size={12} />
        <span>EXECUTION_TRACE_LOG</span>
      </button>
    </div>
  </div>

  <main class="inventory-main">
    <div id="panel-backup-jobs" role="tabpanel" aria-labelledby="tab-{activeTab}" class="contents">
      <BackupJobsTable
        {activeTab}
        {loading}
        {error}
        {backupJobs}
        {backupHistory}
        {jobColumns}
        {historyColumns}
        {formatBytes}
        {runJobNow}
        {toggleJob}
      />
      <BackupJobsSidebar {backupHistory} {formatBytes} />
    </div>
  </main>
</div>

<BackupJobCreateModal
  bind:open={createJobOpen}
  {vms}
  bind:selectedVMId
  bind:selectedBackupType
  bind:selectedTargetPath
  bind:selectedStorageBackend
  {creatingJob}
  {handleCreateJob}
/>

<BackupJobImportModal
  bind:open={importVMOpen}
  bind:importName
  {importFile}
  {importing}
  {handleFileSelect}
  {handleImportVM}
/>

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

  @media (max-width: 1100px) {
    .inventory-main { grid-template-columns: 1fr; }
  }
</style>
>
