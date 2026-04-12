<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Server, Copy, Trash2, Plus, FileCode, Box, LayoutTemplate } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/DataTable.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import CreateFromTemplate from '$lib/components/CreateFromTemplate.svelte';
  import CloudInitViewer from '$lib/components/CloudInitViewer.svelte';
  import CloudInitEditor from '$lib/components/CloudInitEditor.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import type { VMTemplate, CloudInitTemplate, Image, Network, StoragePool, VM } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });

  // State
  let vmTemplates: VMTemplate[] = $state([]);
  let cloudInitTemplates: CloudInitTemplate[] = $state([]);
  let images: Image[] = $state([]);
  let networks: Network[] = $state([]);
  let pools: StoragePool[] = $state([]);
  let vms: VM[] = $state([]);
  let loading = $state(true);
  let error = $state('');
  let activeTab = $state<'vm' | 'cloudinit'>('vm');

  // Modal states
  let createFromTemplateOpen = $state(false);
  let selectedTemplate: VMTemplate | null = $state(null);
  let cloudInitViewerOpen = $state(false);
  let cloudInitEditorOpen = $state(false);
  let selectedCloudInitTemplate: CloudInitTemplate | null = $state(null);
  let createVMTemplateOpen = $state(false);

  // Create VM Template form state
  let newTemplateName = $state('');
  let newTemplateDescription = $state('');
  let selectedVMId = $state('');
  let selectedCloudInitId = $state('');
  let creatingTemplate = $state(false);

  // Confirm dialog state
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

  // Lookup maps
  function getImage(id: string) { return images.find(i => i.id === id); }
  function getNetwork(id: string) { return networks.find(n => n.id === id); }
  function getPool(id: string) { return pools.find(p => p.id === id); }
  function getVM(id: string) { return vms.find(v => v.id === id); }

  // VM Template columns
  const vmTemplateColumns = [
    {
      key: 'name',
      title: 'Name',
      sortable: true,
      render: (t: VMTemplate) => t.name
    },
    {
      key: 'description',
      title: 'Description',
      render: (t: VMTemplate) => t.description || '—'
    },
    {
      key: 'resources',
      title: 'Resources',
      render: (t: VMTemplate) => `${t.vcpu} vCPU, ${t.memory_mb} MB`
    },
    {
      key: 'image_id',
      title: 'Image',
      render: (t: VMTemplate) => {
        const img = getImage(t.image_id);
        return img?.name || t.image_id;
      }
    },
    {
      key: 'tags',
      title: 'Tags',
      render: (t: VMTemplate) => {
        if (!t.tags || t.tags.length === 0) return '—';
        return t.tags.slice(0, 3).join(', ') + (t.tags.length > 3 ? ` +${t.tags.length - 3}` : '');
      }
    }
  ];

  // Cloud-init Template columns
  const cloudInitColumns = [
    {
      key: 'name',
      title: 'Name',
      sortable: true,
      render: (t: CloudInitTemplate) => t.name
    },
    {
      key: 'description',
      title: 'Description',
      render: (t: CloudInitTemplate) => t.description || '—'
    },
    {
      key: 'variables',
      title: 'Variables',
      render: (t: CloudInitTemplate) => {
        if (!t.variables || t.variables.length === 0) return '—';
        return t.variables.join(', ');
      }
    }
  ];

  async function loadData() {
    loading = true;
    error = '';
    try {
      const [vmTemps, cloudTemps, imgs, nets, ps, vmList] = await Promise.all([
        client.listVMTemplates(),
        client.listCloudInitTemplates(),
        client.listImages(),
        client.listNetworks(),
        client.listStoragePools(),
        client.listVMs()
      ]);
      vmTemplates = vmTemps ?? [];
      cloudInitTemplates = cloudTemps ?? [];
      images = imgs ?? [];
      networks = nets ?? [];
      pools = ps ?? [];
      vms = vmList ?? [];
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load templates';
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

  function cloneTemplate(template: VMTemplate) {
    selectedTemplate = template;
    createFromTemplateOpen = true;
  }

  function viewCloudInit(template: CloudInitTemplate) {
    selectedCloudInitTemplate = template;
    cloudInitViewerOpen = true;
  }

  function createCloudInitTemplate() {
    cloudInitEditorOpen = true;
  }

  function openCreateVMTemplate() {
    newTemplateName = '';
    newTemplateDescription = '';
    selectedVMId = '';
    selectedCloudInitId = '';
    createVMTemplateOpen = true;
  }

  async function handleCreateVMTemplate() {
    if (!newTemplateName.trim()) {
      toast.error('Template name is required');
      return;
    }
    if (!selectedVMId) {
      toast.error('Please select a source VM');
      return;
    }

    creatingTemplate = true;
    try {
      const template = await client.createVMTemplate({
        source_vm_id: selectedVMId,
        name: newTemplateName.trim(),
        description: newTemplateDescription.trim() || undefined,
        cloud_init_config: selectedCloudInitId ? 
          cloudInitTemplates.find(t => t.id === selectedCloudInitId)?.content : 
          undefined
      });
      
      toast.success(`VM Template "${template.name}" created successfully`);
      createVMTemplateOpen = false;
      loadData();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to create template';
      toast.error(message);
    } finally {
      creatingTemplate = false;
    }
  }

  function deleteVMTemplate(template: VMTemplate) {
    confirmDialog = {
      open: true,
      title: 'Delete VM Template',
      description: `Are you sure you want to delete "${template.name}"? This action cannot be undone.`,
      action: async () => {
        try {
          await client.deleteVMTemplate(template.id);
          toast.success(`Template "${template.name}" deleted`);
          loadData();
        } catch (err) {
          toast.error(`Failed to delete template: ${err instanceof Error ? err.message : 'Unknown error'}`);
        }
      }
    };
  }

  function deleteCloudInitTemplate(template: CloudInitTemplate) {
    confirmDialog = {
      open: true,
      title: 'Delete Cloud-init Template',
      description: `Are you sure you want to delete "${template.name}"? This action cannot be undone.`,
      action: async () => {
        try {
          await client.deleteCloudInitTemplate(template.id);
          toast.success(`Template "${template.name}" deleted`);
          loadData();
        } catch (err) {
          toast.error(`Failed to delete template: ${err instanceof Error ? err.message : 'Unknown error'}`);
        }
      }
    };
  }
</script>

<div class="flex justify-between items-center mb-6">
  <div>
    <h1 class="text-2xl font-bold text-ink">Templates</h1>
    <p class="text-muted text-sm mt-1">VM templates and cloud-init configurations for rapid provisioning</p>
  </div>
  <div class="flex gap-2">
    {#if activeTab === 'vm'}
      <button
        type="button"
        onclick={openCreateVMTemplate}
        class="inline-flex items-center gap-2 px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors"
      >
        <LayoutTemplate size={16} />
        Create VM Template
      </button>
    {:else}
      <button
        type="button"
        onclick={createCloudInitTemplate}
        class="inline-flex items-center gap-2 px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors"
      >
        <FileCode size={16} />
        Create Cloud-init Template
      </button>
    {/if}
  </div>
</div>

<!-- Tabs -->
<div class="border-b border-line mb-6">
  <div class="flex gap-6">
    <button
      class="pb-3 text-sm font-medium border-b-2 transition-colors {activeTab === 'vm' ? 'border-primary text-primary' : 'border-transparent text-muted hover:text-ink'}"
      onclick={() => activeTab = 'vm'}
    >
      <span class="flex items-center gap-2">
        <Box size={16} />
        VM Templates
        <span class="bg-chrome text-muted text-xs px-2 py-0.5 rounded-full">{vmTemplates.length}</span>
      </span>
    </button>
    <button
      class="pb-3 text-sm font-medium border-b-2 transition-colors {activeTab === 'cloudinit' ? 'border-primary text-primary' : 'border-transparent text-muted hover:text-ink'}"
      onclick={() => activeTab = 'cloudinit'}
    >
      <span class="flex items-center gap-2">
        <FileCode size={16} />
        Cloud-init Templates
        <span class="bg-chrome text-muted text-xs px-2 py-0.5 rounded-full">{cloudInitTemplates.length}</span>
      </span>
    </button>
  </div>
</div>

{#if error}
  <div class="mb-4 border border-danger bg-red-50 px-4 py-3 text-danger">
    {error}
  </div>
{/if}

{#if activeTab === 'vm'}
  <section class="table-card">
    <DataTable
      data={vmTemplates}
      columns={vmTemplateColumns}
      {loading}
      selectable={false}
      emptyIcon={Box as unknown as typeof import('svelte').SvelteComponent}
      emptyTitle="No VM templates yet"
      emptyDescription="Create a VM template from an existing VM to enable rapid provisioning"
      rowId={(t: VMTemplate) => t.id}
    >
      {#snippet children(template: VMTemplate)}
        <div class="flex items-center gap-1">
          <button
            type="button"
            class="action-btn start"
            onclick={() => cloneTemplate(template)}
            title="Clone VM from template"
          >
            <Copy size={14} />
          </button>
          <button
            type="button"
            class="action-btn danger"
            onclick={() => deleteVMTemplate(template)}
            title="Delete template"
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
      data={cloudInitTemplates}
      columns={cloudInitColumns}
      {loading}
      selectable={false}
      emptyIcon={FileCode as unknown as typeof import('svelte').SvelteComponent}
      emptyTitle="No cloud-init templates"
      emptyDescription="Cloud-init templates define VM initialization configurations"
      rowId={(t: CloudInitTemplate) => t.id}
    >
      {#snippet children(template: CloudInitTemplate)}
        <div class="flex items-center gap-1">
          <button
            type="button"
            class="action-btn"
            onclick={() => viewCloudInit(template)}
            title="View template"
          >
            <FileCode size={14} />
          </button>
          {#if !template.id.startsWith('cit-')}
            <button
              type="button"
              class="action-btn danger"
              onclick={() => deleteCloudInitTemplate(template)}
              title="Delete template"
            >
              <Trash2 size={14} />
            </button>
          {/if}
        </div>
      {/snippet}
    </DataTable>
  </section>
{/if}

<!-- Create From Template Modal -->
<CreateFromTemplate
  bind:open={createFromTemplateOpen}
  template={selectedTemplate}
  {images}
  {networks}
  {pools}
  onSuccess={loadData}
/>

<!-- Cloud-init Viewer Modal -->
<CloudInitViewer
  bind:open={cloudInitViewerOpen}
  template={selectedCloudInitTemplate}
/>

<!-- Cloud-init Editor Modal -->
<CloudInitEditor
  bind:open={cloudInitEditorOpen}
  onSuccess={loadData}
/>

<!-- Create VM Template Modal -->
{#if createVMTemplateOpen}
  <div 
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" 
    role="dialog"
    aria-modal="true"
    aria-labelledby="create-template-title"
    onclick={(e) => {
      if (e.target === e.currentTarget) createVMTemplateOpen = false;
    }}
    onkeydown={(e) => {
      if (e.key === 'Escape') createVMTemplateOpen = false;
    }}
  >
    <div class="bg-white rounded-lg shadow-lg w-full max-w-lg mx-4">
      <div class="flex items-center justify-between px-6 py-4 border-b border-line">
        <h2 class="text-lg font-semibold text-ink">Create VM Template</h2>
        <button
          type="button"
          onclick={() => createVMTemplateOpen = false}
          class="text-muted hover:text-ink"
          aria-label="Close dialog"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
        </button>
      </div>
      
      <div class="p-6 space-y-4">
        <div>
          <label for="template-name" class="block text-sm font-medium text-ink mb-1">
            Template Name <span class="text-danger">*</span>
          </label>
          <input
            id="template-name"
            type="text"
            bind:value={newTemplateName}
            placeholder="e.g., Ubuntu Web Server"
            class="w-full h-9 rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
        </div>

        <div>
          <label for="template-description" class="block text-sm font-medium text-ink mb-1">
            Description
          </label>
          <input
            id="template-description"
            type="text"
            bind:value={newTemplateDescription}
            placeholder="Brief description of this template"
            class="w-full h-9 rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
        </div>

        <div>
          <label for="source-vm" class="block text-sm font-medium text-ink mb-1">
            Source VM <span class="text-danger">*</span>
          </label>
          <select
            id="source-vm"
            bind:value={selectedVMId}
            class="w-full h-9 rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          >
            <option value="">Select a VM...</option>
            {#each vms as vm}
              <option value={vm.id}>{vm.name} ({vm.vcpu} vCPU, {vm.memory_mb} MB)</option>
            {/each}
          </select>
          {#if vms.length === 0}
            <p class="text-xs text-muted mt-1">No VMs available. Create a VM first to use as a template.</p>
          {/if}
        </div>

        <div>
          <label for="cloud-init-template" class="block text-sm font-medium text-ink mb-1">
            Default Cloud-init Template (Optional)
          </label>
          <select
            id="cloud-init-template"
            bind:value={selectedCloudInitId}
            class="w-full h-9 rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          >
            <option value="">None</option>
            {#each cloudInitTemplates as cit}
              <option value={cit.id}>{cit.name}</option>
            {/each}
          </select>
          <p class="text-xs text-muted mt-1">
            This cloud-init config will be used by default when cloning from this template.
          </p>
        </div>
      </div>

      <div class="flex items-center justify-end gap-2 px-6 py-4 border-t border-line">
        <button
          type="button"
          onclick={() => createVMTemplateOpen = false}
          disabled={creatingTemplate}
          class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          type="button"
          onclick={handleCreateVMTemplate}
          disabled={creatingTemplate || !newTemplateName.trim() || !selectedVMId}
          class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2"
        >
          {#if creatingTemplate}
            <svg class="animate-spin h-4 w-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            Creating...
          {:else}
            Create Template
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
</style>
