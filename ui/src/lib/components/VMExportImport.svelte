<script lang="ts">
  import { Download, Upload, FileArchive, HardDrive, FileImage, Loader2, CheckCircle, AlertCircle } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import { goto } from '$app/navigation';

  interface Props {
    vmId?: string;
    vmName?: string;
  }

  let { vmId = '', vmName = '' }: Props = $props();

  const client = createAPIClient({ token: getStoredToken() ?? undefined });

  // Export state
  let exportFormat = $state<'qcow2' | 'ova' | 'raw'>('qcow2');
  let exporting = $state(false);
  let exportStatus = $state<'idle' | 'processing' | 'ready' | 'error'>('idle');
  let exportDownloadUrl = $state('');
  let exportId = $state('');

  // Import state
  let importFile = $state<File | null>(null);
  let importName = $state('');
  let importing = $state(false);
  let importProgress = $state(0);

  const formats = [
    { id: 'qcow2', name: 'QCOW2', description: 'Native QEMU disk format', icon: HardDrive },
    { id: 'ova', name: 'OVA', description: 'Open Virtualization Archive', icon: FileArchive },
    { id: 'raw', name: 'Raw', description: 'Raw disk image', icon: FileImage }
  ] as const;

  async function exportVM() {
    if (!vmId) {
      toast.error('No VM selected for export');
      return;
    }

    exporting = true;
    exportStatus = 'processing';
    
    try {
      const result = await client.exportVM(vmId);
      exportId = result.export_id;
      exportDownloadUrl = result.download_url;
      exportStatus = 'ready';
      toast.success('VM export ready for download');
    } catch (e: any) {
      exportStatus = 'error';
      toast.error(`Export failed: ${e.message || 'Unknown error'}`);
    } finally {
      exporting = false;
    }
  }

  async function downloadExport() {
    if (!exportDownloadUrl) {
      toast.error('No export available for download');
      return;
    }

    try {
      const response = await client.downloadExport(exportId);
      if (!response.ok) {
        throw new Error('Download failed');
      }
      
      const blob = await response.blob();
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${vmName || 'vm'}-export.tar.gz`;
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      document.body.removeChild(a);
      
      toast.success('Download started');
    } catch (e: any) {
      toast.error(`Download failed: ${e.message || 'Unknown error'}`);
    }
  }

  function handleFileSelect(event: Event) {
    const input = event.target as HTMLInputElement;
    if (input.files && input.files[0]) {
      importFile = input.files[0];
      if (!importName) {
        // Extract name from filename
        importName = importFile.name
          .replace(/\.tar\.gz$/, '')
          .replace(/\.ova$/, '')
          .replace(/\.qcow2$/, '')
          .replace(/\.raw$/, '')
          .replace(/\.img$/, '');
      }
    }
  }

  async function importVM() {
    if (!importFile) {
      toast.error('Please select a file to import');
      return;
    }
    if (!importName.trim()) {
      toast.error('VM name is required');
      return;
    }

    importing = true;
    importProgress = 0;
    
    try {
      const vm = await client.importVM(importFile, importName.trim());
      toast.success(`VM "${importName}" imported successfully`);
      importFile = null;
      importName = '';
      goto('/vms');
    } catch (e: any) {
      toast.error(`Import failed: ${e.message || 'Unknown error'}`);
    } finally {
      importing = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }
</script>

<div class="space-y-8">
  <!-- Export Section -->
  <section class="bg-white border border-line rounded-lg overflow-hidden">
    <div class="px-6 py-4 border-b border-line bg-gray-50">
      <h3 class="text-lg font-semibold text-ink flex items-center gap-2">
        <Download size={20} />
        Export VM
      </h3>
      <p class="text-sm text-muted mt-1">
        Export VM disk to a downloadable archive for backup or migration.
      </p>
    </div>

    <div class="p-6 space-y-6">
      {#if vmId}
        <!-- Format Selection -->
        <div>
          <label class="block text-sm font-medium text-ink mb-3">Export Format</label>
          <div class="grid grid-cols-3 gap-4">
            {#each formats as format}
              <button
                type="button"
                onclick={() => exportFormat = format.id}
                class="p-4 border-2 rounded-lg text-left transition-all {exportFormat === format.id ? 'border-primary bg-primary/5' : 'border-line hover:border-primary/30'}"
              >
                <format.icon size={24} class="mb-2 {exportFormat === format.id ? 'text-primary' : 'text-muted'}" />
                <div class="font-medium text-ink">{format.name}</div>
                <div class="text-xs text-muted">{format.description}</div>
              </button>
            {/each}
          </div>
        </div>

        <!-- Export Actions -->
        <div class="flex items-center gap-4">
          <button
            type="button"
            onclick={exportVM}
            disabled={exporting || exportStatus === 'processing'}
            class="px-4 py-2 bg-primary text-white font-medium rounded hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            {#if exporting || exportStatus === 'processing'}
              <Loader2 size={16} class="animate-spin" />
              Processing...
            {:else}
              <Download size={16} />
              Export VM
            {/if}
          </button>

          {#if exportStatus === 'ready'}
            <button
              type="button"
              onclick={downloadExport}
              class="px-4 py-2 bg-success text-white font-medium rounded hover:bg-success/90 transition-colors flex items-center gap-2"
            >
              <CheckCircle size={16} />
              Download Export
            </button>
          {/if}
        </div>

        <!-- Status Messages -->
        {#if exportStatus === 'processing'}
          <div class="bg-blue-50 border border-blue-200 rounded p-4 text-sm text-blue-800 flex items-center gap-2">
            <Loader2 size={16} class="animate-spin" />
            Creating export archive... This may take a few minutes for large VMs.
          </div>
        {:else if exportStatus === 'ready'}
          <div class="bg-emerald-50 border border-emerald-200 rounded p-4 text-sm text-emerald-800 flex items-center gap-2">
            <CheckCircle size={16} />
            Export ready! Click the download button to save the file.
          </div>
        {:else if exportStatus === 'error'}
          <div class="bg-red-50 border border-red-200 rounded p-4 text-sm text-red-800 flex items-center gap-2">
            <AlertCircle size={16} />
            Export failed. Please try again.
          </div>
        {/if}
      {:else}
        <div class="text-center py-8 text-muted">
          <Download size={48} class="mx-auto mb-4 opacity-20" />
          <p>Select a VM to export from the VM details page.</p>
        </div>
      {/if}
    </div>
  </section>

  <!-- Import Section -->
  <section class="bg-white border border-line rounded-lg overflow-hidden">
    <div class="px-6 py-4 border-b border-line bg-gray-50">
      <h3 class="text-lg font-semibold text-ink flex items-center gap-2">
        <Upload size={20} />
        Import VM
      </h3>
      <p class="text-sm text-muted mt-1">
        Import a VM from a backup file or exported disk image.
      </p>
    </div>

    <div class="p-6 space-y-6">
      <!-- Supported Formats -->
      <div class="bg-blue-50 border border-blue-200 rounded p-4 text-sm">
        <p class="font-medium text-blue-900 mb-2">Supported Formats</p>
        <ul class="list-disc list-inside space-y-1 text-blue-800">
          <li><strong>QCOW2</strong> - QEMU disk images (.qcow2)</li>
          <li><strong>OVA</strong> - Open Virtualization Archives (.ova)</li>
          <li><strong>Raw</strong> - Raw disk images (.raw, .img)</li>
          <li><strong>CHV Export</strong> - CHV backup archives (.tar.gz)</li>
        </ul>
      </div>

      <!-- Import Form -->
      <div class="space-y-4">
        <div>
          <label for="import-vm-name" class="block text-sm font-medium text-ink mb-1">
            New VM Name <span class="text-danger">*</span>
          </label>
          <input
            id="import-vm-name"
            type="text"
            bind:value={importName}
            placeholder="e.g., imported-vm"
            class="w-full h-9 rounded border border-line bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
        </div>

        <div>
          <label for="import-file" class="block text-sm font-medium text-ink mb-1">
            Backup File <span class="text-danger">*</span>
          </label>
          <div class="flex items-center gap-4">
            <label class="flex-1 cursor-pointer">
              <input
                id="import-file"
                type="file"
                accept=".tar.gz,.ova,.qcow2,.raw,.img"
                onchange={handleFileSelect}
                class="sr-only"
              />
              <div class="flex items-center justify-center w-full h-32 border-2 border-dashed border-line rounded-lg hover:border-primary/50 hover:bg-primary/5 transition-colors">
                <div class="text-center">
                  <Upload size={24} class="mx-auto mb-2 text-muted" />
                  <span class="text-sm text-muted">
                    {#if importFile}
                      {importFile.name}
                    {:else}
                      Click to select a file
                    {/if}
                  </span>
                  {#if importFile}
                    <p class="text-xs text-muted mt-1">
                      Size: {formatBytes(importFile.size)}
                    </p>
                  {/if}
                </div>
              </div>
            </label>
          </div>
        </div>

        <!-- Import Process Info -->
        <div class="text-sm text-muted bg-chrome rounded p-4">
          <p class="font-medium text-ink mb-2">Import Process:</p>
          <ol class="list-decimal list-inside space-y-1">
            <li>File will be uploaded to the server</li>
            <li>Disk image will be extracted and validated</li>
            <li>New VM will be created with imported disk</li>
            <li>VM will be in "prepared" state - ready to start</li>
          </ol>
        </div>

        <!-- Import Button -->
        <button
          type="button"
          onclick={importVM}
          disabled={importing || !importFile || !importName.trim()}
          class="w-full px-4 py-3 bg-primary text-white font-medium rounded hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          {#if importing}
            <Loader2 size={18} class="animate-spin" />
            Importing VM...
          {:else}
            <Upload size={18} />
            Import VM
          {/if}
        </button>

        <!-- Progress Bar -->
        {#if importing}
          <div class="space-y-2">
            <div class="h-2 bg-gray-200 rounded-full overflow-hidden">
              <div class="h-full bg-primary animate-pulse" style="width: 100%"></div>
            </div>
            <p class="text-xs text-center text-muted">Uploading and processing... Please wait.</p>
          </div>
        {/if}
      </div>
    </div>
  </section>
</div>

<style>
  .text-danger {
    color: var(--color-danger);
  }

  .bg-success {
    background-color: var(--color-success);
  }

  .bg-chrome {
    background-color: var(--color-chrome);
  }
</style>
