<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { 
    Database, 
    Plus, 
    Trash2, 
    Play, 
    Pause, 
    Clock, 
    Calendar,
    HardDrive,
    Server,
    CheckCircle,
    XCircle,
    Loader2,
    Download,
    Upload,
    RefreshCw
  } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/DataTable.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import type { BackupJob, BackupJobResponse, BackupHistory, VM } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });

  // State
  let backupJobs = $state<BackupJobResponse[]>([]);
  let backupHistory = $state<BackupHistory[]>([]);
  let vms = $state<VM[]>([]);
  let loading = $state(true);
  let error = $state('');
  let activeTab = $state<'jobs' | 'history'>('jobs');

  // Modal states
  let createJobOpen = $state(false);
  let importVMOpen = $state(false);
  let confirmDialog = $state<{
    open: boolean;
    title: string;
    description: string;
    action: () => Promise<void>;
  }>({
    open: false,
    title: '',
    description: '',
    action: async () => {}
  });

  // Create job form state
  let newJobName = $state('');
  let selectedVMId = $state('');
  let newJobSchedule = $state('0 2 * * *'); // Daily at 2 AM
  let newJobRetention = $state(7);
  let creatingJob = $state(false);

  // Import form state
  let importFile = $state<File | null>(null);
  let importName = $state('');
  let importing = $state(false);

  // Job columns
  const jobColumns = [
    {
      key: 'name',
      title: 'Name',
      sortable: true,
      render: (job: BackupJobResponse) => job.name
    },
    {
      key: 'vm_name',
      title: 'VM',
      render: (job: BackupJobResponse) => job.vm_name || job.vm_id.slice(0, 8)
    },
    {
      key: 'schedule',
      title: 'Schedule',
      render: (job: BackupJobResponse) => formatCron(job.schedule)
    },
    {
      key: 'retention',
      title: 'Retention',
      render: (job: BackupJobResponse) => `${job.retention} backups`
    },
    {
      key: 'status',
      title: 'Status',
      render: (job: BackupJobResponse) => {
        if (job.enabled) {
          return 'Enabled';
        }
        return 'Disabled';
      }
    },
    {
      key: 'next_run',
      title: 'Next Run',
      render: (job: BackupJobResponse) => job.next_run_formatted || '—'
    },
    {
      key: 'last_run',
      title: 'Last Run',
      render: (job: BackupJobResponse) => job.last_run_formatted || 'Never'
    }
  ];

  // History columns
  const historyColumns = [
    {
      key: 'vm_name',
      title: 'VM',
      render: (h: BackupHistory) => h.vm_name || h.vm_id.slice(0, 8)
    },
    {
      key: 'status',
      title: 'Status',
      render: (h: BackupHistory) => {
        return h.status.charAt(0).toUpperCase() + h.status.slice(1);
      }
    },
    {
      key: 'size',
      title: 'Size',
      render: (h: BackupHistory) => formatBytes(h.size_bytes)
    },
    {
      key: 'started_at',
      title: 'Started',
      render: (h: BackupHistory) => new Date(h.started_at).toLocaleString()
    },
    {
      key: 'completed_at',
      title: 'Completed',
      render: (h: BackupHistory) => h.completed_at ? new Date(h.completed_at).toLocaleString() : '—'
    }
  ];

  function formatCron(cron: string): string {
    // Simple cron formatter - could be expanded
    const parts = cron.split(' ');
    if (parts.length === 5) {
      if (cron === '0 2 * * *') return 'Daily at 2:00 AM';
      if (cron === '0 */6 * * *') return 'Every 6 hours';
      if (cron === '0 * * * *') return 'Hourly';
      if (cron === '0 0 * * 0') return 'Weekly (Sunday)';
    }
    return cron;
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  async function loadData() {
    loading = true;
    error = '';
    try {
      const [jobs, history, vmList] = await Promise.all([
        client.listBackupJobs(),
        client.listBackupHistory(),
        client.listVMs()
      ]);
      backupJobs = jobs ?? [];
      backupHistory = history ?? [];
      vms = vmList ?? [];
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load backup data';
      toast.error(error);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (!token) {
      goto('/login');
      return;
    }
    loadData();
  });

  async function toggleJob(job: BackupJobResponse) {
    try {
      await client.toggleBackupJob(job.id);
      toast.success(`Backup job ${job.enabled ? 'disabled' : 'enabled'}`);
      loadData();
    } catch (err) {
      toast.error(`Failed to toggle job: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  }

  async function runJobNow(job: BackupJobResponse) {
    try {
      await client.runBackupJob(job.id);
      toast.success('Backup job started');
      setTimeout(loadData, 2000);
    } catch (err) {
      toast.error(`Failed to run job: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  }

  function deleteJob(job: BackupJobResponse) {
    confirmDialog = {
      open: true,
      title: 'Delete Backup Job',
      description: `Are you sure you want to delete "${job.name}"? This will not delete existing backups.`,
      action: async () => {
        try {
          await client.deleteBackupJob(job.id);
          toast.success(`Backup job "${job.name}" deleted`);
          loadData();
        } catch (err) {
          toast.error(`Failed to delete job: ${err instanceof Error ? err.message : 'Unknown error'}`);
        }
      }
    };
  }

  async function handleCreateJob() {
    if (!newJobName.trim()) {
      toast.error('Job name is required');
      return;
    }
    if (!selectedVMId) {
      toast.error('Please select a VM');
      return;
    }
    if (!newJobSchedule) {
      toast.error('Schedule is required');
      return;
    }

    creatingJob = true;
    try {
      await client.createBackupJob({
        vm_id: selectedVMId,
        name: newJobName.trim(),
        schedule: newJobSchedule,
        retention: newJobRetention
      });
      
      toast.success(`Backup job "${newJobName}" created successfully`);
      createJobOpen = false;
      resetJobForm();
      loadData();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to create backup job';
      toast.error(message);
    } finally {
      creatingJob = false;
    }
  }

  function resetJobForm() {
    newJobName = '';
    selectedVMId = '';
    newJobSchedule = '0 2 * * *';
    newJobRetention = 7;
  }

  function handleFileSelect(event: Event) {
    const input = event.target as HTMLInputElement;
    if (input.files && input.files[0]) {
      importFile = input.files[0];
      if (!importName) {
        // Extract name from filename
        importName = importFile.name.replace(/\.tar\.gz$/, '').replace(/\.ova$/, '').replace(/\.qcow2$/, '');
      }
    }
  }

  async function handleImportVM() {
    if (!importFile) {
      toast.error('Please select a file to import');
      return;
    }
    if (!importName.trim()) {
      toast.error('VM name is required');
      return;
    }

    importing = true;
    try {
      await client.importVM(importFile, importName.trim());
      toast.success(`VM "${importName}" imported successfully`);
      importVMOpen = false;
      importFile = null;
      importName = '';
      goto('/vms');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to import VM';
      toast.error(message);
    } finally {
      importing = false;
    }
  }

  async function exportVM(vmId: string) {
    try {
      const result = await client.exportVM(vmId);
      toast.success(`VM export started. Download will be available at ${result.download_url}`);
    } catch (err) {
      toast.error(`Failed to export VM: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  }
</script>

<div class="flex justify-between items-center mb-6">
  <div>
    <h1 class="text-2xl font-bold text-ink">Backup & Disaster Recovery</h1>
    <p class="text-muted text-sm mt-1">Scheduled backups, snapshots, and VM export/import</p>
  </div>
  <div class="flex gap-2">
    {#if activeTab === 'jobs'}
      <button
        type="button"
        onclick={() => importVMOpen = true}
        class="inline-flex items-center gap-2 px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors"
      >
        <Upload size={16} />
        Import VM
      </button>
      <button
        type="button"
        onclick={() => createJobOpen = true}
        class="inline-flex items-center gap-2 px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors"
      >
        <Plus size={16} />
        Create Backup Job
      </button>
    {/if}
  </div>
</div>

<!-- Tabs -->
<div class="border-b border-line mb-6">
  <div class="flex gap-6">
    <button
      class="pb-3 text-sm font-medium border-b-2 transition-colors {activeTab === 'jobs' ? 'border-primary text-primary' : 'border-transparent text-muted hover:text-ink'}"
      onclick={() => activeTab = 'jobs'}
    >
      <span class="flex items-center gap-2">
        <Calendar size={16} />
        Backup Jobs
        <span class="bg-chrome text-muted text-xs px-2 py-0.5 rounded-full">{backupJobs.length}</span>
      </span>
    </button>
    <button
      class="pb-3 text-sm font-medium border-b-2 transition-colors {activeTab === 'history' ? 'border-primary text-primary' : 'border-transparent text-muted hover:text-ink'}"
      onclick={() => activeTab = 'history'}
    >
      <span class="flex items-center gap-2">
        <Database size={16} />
        History
        <span class="bg-chrome text-muted text-xs px-2 py-0.5 rounded-full">{backupHistory.length}</span>
      </span>
    </button>
  </div>
</div>

{#if error}
  <div class="mb-4 border border-danger bg-red-50 px-4 py-3 text-danger">
    {error}
  </div>
{/if}

{#if activeTab === 'jobs'}
  <section class="table-card">
    <DataTable
      data={backupJobs}
      columns={jobColumns}
      {loading}
      selectable={false}
      emptyIcon={Calendar as unknown as typeof import('svelte').SvelteComponent}
      emptyTitle="No backup jobs yet"
      emptyDescription="Create scheduled backup jobs to automatically protect your VMs"
      rowId={(job: BackupJobResponse) => job.id}
    >
      {#snippet children(job: BackupJobResponse)}
        <div class="flex items-center gap-1">
          <button
            type="button"
            class="action-btn start"
            onclick={() => runJobNow(job)}
            title="Run now"
          >
            <Play size={14} />
          </button>
          <button
            type="button"
            class="action-btn"
            onclick={() => toggleJob(job)}
            title={job.enabled ? 'Disable' : 'Enable'}
          >
            {#if job.enabled}
              <Pause size={14} />
            {:else}
              <Play size={14} />
            {/if}
          </button>
          <button
            type="button"
            class="action-btn danger"
            onclick={() => deleteJob(job)}
            title="Delete job"
          >
            <Trash2 size={14} />
          </button>
        </div>
      {/snippet}
    </DataTable>
  </section>
{:else}
  <section class="table-card">
    <DataTable
      data={backupHistory}
      columns={historyColumns}
      {loading}
      selectable={false}
      emptyIcon={Database as unknown as typeof import('svelte').SvelteComponent}
      emptyTitle="No backup history"
      emptyDescription="Backup history will appear here when backups are run"
      rowId={(h: BackupHistory) => h.id}
    >
      {#snippet children(h: BackupHistory)}
        <div class="flex items-center gap-1">
          {#if h.status === 'running'}
            <Loader2 size={14} class="animate-spin text-primary" />
          {:else if h.status === 'completed'}
            <CheckCircle size={14} class="text-success" />
          {:else if h.status === 'failed'}
            <XCircle size={14} class="text-danger" />
          {/if}
        </div>
      {/snippet}
    </DataTable>
  </section>
{/if}

<!-- Create Backup Job Modal -->
{#if createJobOpen}
  <div 
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" 
    role="dialog"
    aria-modal="true"
    aria-labelledby="create-job-title"
    onclick={(e) => {
      if (e.target === e.currentTarget) createJobOpen = false;
    }}
    onkeydown={(e) => {
      if (e.key === 'Escape') createJobOpen = false;
    }}
  >
    <div class="bg-white rounded-lg shadow-lg w-full max-w-lg mx-4">
      <div class="flex items-center justify-between px-6 py-4 border-b border-line">
        <h2 id="create-job-title" class="text-lg font-semibold text-ink">Create Backup Job</h2>
        <button
          type="button"
          onclick={() => createJobOpen = false}
          class="text-muted hover:text-ink"
          aria-label="Close dialog"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
        </button>
      </div>
      
      <div class="p-6 space-y-4">
        <div>
          <label for="job-name" class="block text-sm font-medium text-ink mb-1">
            Job Name <span class="text-danger">*</span>
          </label>
          <input
            id="job-name"
            type="text"
            bind:value={newJobName}
            placeholder="e.g., Daily Backup"
            class="w-full h-9 rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
        </div>

        <div>
          <label for="vm-select" class="block text-sm font-medium text-ink mb-1">
            Virtual Machine <span class="text-danger">*</span>
          </label>
          <select
            id="vm-select"
            bind:value={selectedVMId}
            class="w-full h-9 rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          >
            <option value="">Select a VM...</option>
            {#each vms as vm}
              <option value={vm.id}>{vm.name} ({vm.vcpu} vCPU, {vm.memory_mb} MB)</option>
            {/each}
          </select>
          {#if vms.length === 0}
            <p class="text-xs text-muted mt-1">No VMs available. Create a VM first.</p>
          {/if}
        </div>

        <div>
          <label for="schedule" class="block text-sm font-medium text-ink mb-1">
            Schedule (Cron Expression) <span class="text-danger">*</span>
          </label>
          <select
            id="schedule"
            bind:value={newJobSchedule}
            class="w-full h-9 rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          >
            <option value="0 2 * * *">Daily at 2:00 AM</option>
            <option value="0 */6 * * *">Every 6 hours</option>
            <option value="0 * * * *">Hourly</option>
            <option value="0 0 * * 0">Weekly (Sunday)</option>
          </select>
          <p class="text-xs text-muted mt-1">
            Cron format: minute hour day month weekday
          </p>
        </div>

        <div>
          <label for="retention" class="block text-sm font-medium text-ink mb-1">
            Retention (number of backups to keep)
          </label>
          <input
            id="retention"
            type="number"
            bind:value={newJobRetention}
            min="1"
            max="100"
            class="w-full h-9 rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
          <p class="text-xs text-muted mt-1">
            Older backups will be automatically deleted when this limit is exceeded.
          </p>
        </div>
      </div>

      <div class="flex items-center justify-end gap-2 px-6 py-4 border-t border-line">
        <button
          type="button"
          onclick={() => createJobOpen = false}
          disabled={creatingJob}
          class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          type="button"
          onclick={handleCreateJob}
          disabled={creatingJob || !newJobName.trim() || !selectedVMId}
          class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2"
        >
          {#if creatingJob}
            <svg class="animate-spin h-4 w-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            Creating...
          {:else}
            Create Job
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Import VM Modal -->
{#if importVMOpen}
  <div 
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" 
    role="dialog"
    aria-modal="true"
    aria-labelledby="import-vm-title"
    onclick={(e) => {
      if (e.target === e.currentTarget) importVMOpen = false;
    }}
    onkeydown={(e) => {
      if (e.key === 'Escape') importVMOpen = false;
    }}
  >
    <div class="bg-white rounded-lg shadow-lg w-full max-w-lg mx-4">
      <div class="flex items-center justify-between px-6 py-4 border-b border-line">
        <h2 id="import-vm-title" class="text-lg font-semibold text-ink">Import VM</h2>
        <button
          type="button"
          onclick={() => importVMOpen = false}
          class="text-muted hover:text-ink"
          aria-label="Close dialog"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
        </button>
      </div>
      
      <div class="p-6 space-y-4">
        <div class="bg-blue-50 border border-blue-200 rounded p-4 text-sm text-blue-800">
          <p class="font-medium mb-1">Supported Formats</p>
          <ul class="list-disc list-inside space-y-1">
            <li>QCOW2 disk images (.qcow2)</li>
            <li>OVA archives (.ova)</li>
            <li>Raw disk images (.raw, .img)</li>
            <li>CHV export archives (.tar.gz)</li>
          </ul>
        </div>

        <div>
          <label for="vm-name" class="block text-sm font-medium text-ink mb-1">
            VM Name <span class="text-danger">*</span>
          </label>
          <input
            id="vm-name"
            type="text"
            bind:value={importName}
            placeholder="e.g., imported-vm"
            class="w-full h-9 rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
        </div>

        <div>
          <label for="import-file" class="block text-sm font-medium text-ink mb-1">
            Backup File <span class="text-danger">*</span>
          </label>
          <input
            id="import-file"
            type="file"
            accept=".tar.gz,.ova,.qcow2,.raw,.img"
            onchange={handleFileSelect}
            class="w-full text-sm file:mr-4 file:py-2 file:px-4 file:rounded file:border-0 file:text-sm file:font-medium file:bg-primary file:text-white hover:file:bg-primary/90"
          />
          {#if importFile}
            <p class="text-xs text-muted mt-1">
              Selected: {importFile.name} ({formatBytes(importFile.size)})
            </p>
          {/if}
        </div>

        <div class="text-sm text-muted">
          <p class="font-medium text-ink mb-1">Import Process:</p>
          <ol class="list-decimal list-inside space-y-1">
            <li>File will be uploaded to the server</li>
            <li>Disk image will be extracted and validated</li>
            <li>New VM will be created with imported disk</li>
            <li>VM will be in "prepared" state - ready to start</li>
          </ol>
        </div>
      </div>

      <div class="flex items-center justify-end gap-2 px-6 py-4 border-t border-line">
        <button
          type="button"
          onclick={() => importVMOpen = false}
          disabled={importing}
          class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          type="button"
          onclick={handleImportVM}
          disabled={importing || !importFile || !importName.trim()}
          class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2"
        >
          {#if importing}
            <svg class="animate-spin h-4 w-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            Importing...
          {:else}
            <Upload size={16} />
            Import VM
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<ConfirmDialog
  bind:open={confirmDialog.open}
  title={confirmDialog.title}
  description={confirmDialog.description}
  confirmText="Delete"
  variant="danger"
  onConfirm={() => { confirmDialog.action(); confirmDialog.open = false; }}
  onCancel={() => confirmDialog.open = false}
/>

<style>
  .action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    color: var(--color-neutral-500);
    background: transparent;
    border: none;
    cursor: pointer;
    transition: all var(--duration-fast);
  }

  .action-btn:hover {
    background: var(--color-neutral-100);
    color: var(--color-neutral-700);
  }

  .action-btn.start:hover {
    color: var(--color-success);
    background: var(--color-success-light);
  }

  .action-btn.danger:hover {
    color: var(--color-danger);
    background: var(--color-danger-light);
  }

  .table-card {
    background: white;
    border: 1px solid var(--color-line);
    border-radius: 0.5rem;
    overflow: hidden;
  }
</style>
