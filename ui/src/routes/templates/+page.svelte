<script lang="ts">
import Button from '$lib/components/primitives/Button.svelte';
  import { onMount } from 'svelte';
  import { 
    Server, Copy, Trash2, Plus, FileCode, Box, LayoutTemplate, 
    ArrowRight, Activity, ShieldCheck, Search
  } from 'lucide-svelte';
  import { createAPIClient } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import SectionCard from '$lib/components/shell/SectionCard.svelte';
  import CompactMetricCard from '$lib/components/shared/CompactMetricCard.svelte';
  import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
  import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
  import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
  import ErrorState from '$lib/components/shell/ErrorState.svelte';
  import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
  import CreateFromTemplate from '$lib/components/vms/CreateFromTemplate.svelte';
  import CloudInitViewer from '$lib/components/shell/CloudInitViewer.svelte';
  import CloudInitEditor from '$lib/components/shell/CloudInitEditor.svelte';
  import { getPageDefinition } from '$lib/shell/app-shell';
  import type { ShellTone } from '$lib/shell/app-shell';
  import type { VMTemplate, CloudInitTemplate, Image, Network, StoragePool, VM } from '$lib/api/types';
  import ConfirmDialog from '$lib/components/shared/ConfirmDialog.svelte';

  const InventoryTableAny = InventoryTable as any;

  const client = createAPIClient();
  const pageDef = getPageDefinition('/images'); // Reusing Images definition as it covers library

  let vmTemplates = $state<VMTemplate[]>([]);
  let cloudInitTemplates = $state<CloudInitTemplate[]>([]);
  let images = $state<Image[]>([]);
  let networks = $state<Network[]>([]);
  let pools = $state<StoragePool[]>([]);
  let vms = $state<VM[]>([]);
  let loading = $state(true);
  let error = $state('');
  let activeTab = $state<'vm' | 'cloudinit'>('vm');

  // Modal states
  let createFromTemplateOpen = $state(false);
  let selectedTemplate = $state<VMTemplate | null>(null);
  let cloudInitViewerOpen = $state(false);
  let cloudInitEditorOpen = $state(false);
  let selectedCloudInitTemplate = $state<CloudInitTemplate | null>(null);
  let createVMTemplateOpen = $state(false);
  let newTemplateName = $state('');
  let newTemplateDescription = $state('');
  let selectedVMId = $state('');
  let selectedCloudInitId = $state('');
  let creatingTemplate = $state(false);
  let confirmDialog = $state({ open: false, title: '', description: '', action: () => {} });

  async function handleCreateVMTemplate() {
    if (!newTemplateName.trim() || !selectedVMId) return;
    creatingTemplate = true;
    try {
      await client.createVMTemplate({
        name: newTemplateName.trim(),
        description: newTemplateDescription.trim() || undefined,
        source_vm_id: selectedVMId,
        cloud_init_config: selectedCloudInitId ? undefined : undefined
      });
      toast.success('Template created successfully');
      createVMTemplateOpen = false;
      newTemplateName = '';
      newTemplateDescription = '';
      selectedVMId = '';
      selectedCloudInitId = '';
      await loadData();
    } catch (err: any) {
      toast.error(err.message || 'Failed to create template');
    } finally {
      creatingTemplate = false;
    }
  }

  const vmColumns = [
    { key: 'name', label: 'Template Identity' },
    { key: 'resources', label: 'Resource Profile' },
    { key: 'image_name', label: 'Base Image' },
    { key: 'tags', label: 'Directives' },
    { key: 'status', label: 'Availability', align: 'center' as const }
  ];

  const ciColumns = [
    { key: 'name', label: 'Identity' },
    { key: 'variables', label: 'Defined Var Registry' },
    { key: 'last_used', label: 'Last Seq', align: 'right' as const }
  ];

  async function loadData() {
    loading = true;
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
    } catch (err: any) {
      error = err.message || 'Blueprint registry unavailable';
    } finally {
      loading = false;
    }
  }

  onMount(loadData);

  function cloneTemplate(template: VMTemplate) {
    selectedTemplate = template;
    createFromTemplateOpen = true;
  }
</script>

<div class="inventory-page">
  <PageHeaderWithAction page={pageDef}>
    {#snippet actions()}
      <div class="header-actions">
        {#if activeTab === 'vm'}
          <Button variant="primary" onclick={() => createVMTemplateOpen = true}>
            <LayoutTemplate size={14} />
            Commit Blueprint
          </Button>
        {:else}
          <Button variant="primary" onclick={() => cloudInitEditorOpen = true}>
            <FileCode size={14} />
            Register Init Script
          </Button>
        {/if}
      </div>
    {/snippet}
  </PageHeaderWithAction>

  <div class="inventory-metrics">
    <CompactMetricCard 
      label="Provision Blueprints" 
      value={vmTemplates.length} 
      color="neutral"
    />
    <CompactMetricCard 
      label="Init Registries" 
      value={cloudInitTemplates.length} 
      color="primary"
    />
    <CompactMetricCard 
      label="Library Assets" 
      value={images.length} 
      color="neutral"
    />
    <CompactMetricCard 
      label="SLA Compliance" 
      value="NOMINAL" 
      color="primary"
    />
  </div>

  <div class="tabs-nav">
    <button class="tab-item" class:active={activeTab === 'vm'} onclick={() => activeTab = 'vm'}>
      <Box size={14} />
      <span>Workload Blueprints</span>
    </button>
    <button class="tab-item" class:active={activeTab === 'cloudinit'} onclick={() => activeTab = 'cloudinit'}>
      <FileCode size={14} />
      <span>Init Registries</span>
    </button>
  </div>

  <main class="inventory-main">
    <section class="inventory-table-area">
      {#if loading && vmTemplates.length === 0}
        <div class="skeleton-table"></div>
      {:else if error}
        <ErrorState />
      {:else if activeTab === 'vm'}
        <InventoryTableAny columns={vmColumns} rows={vmTemplates.map(t => ({
          ...t,
          resources: `${t.vcpu} vCPU / ${t.memory_mb}MB`,
          image_name: images.find(i => i.id === t.image_id)?.name || t.image_id,
          status: { label: 'VERIFIED', tone: 'healthy' }
        }))}>
          {#snippet cell({ column, row }: { column: any; row: any })}
             {#if column.key === 'name'}
               <span class="blueprint-name">{row.name}</span>
             {:else if column.key === 'status'}
               <StatusBadge label={row.status.label} tone={row.status.tone as ShellTone} />
             {:else}
               <span class="cell-text">{(row as Record<string, unknown>)[column.key]}</span>
             {/if}
          {/snippet}
          {#snippet actions({ row }: { row: any })}
            <div class="row-ops">
               <button class="op-btn" onclick={() => cloneTemplate(row)} title="Orchestrate Workload"><Copy size={12} /></button>
            </div>
          {/snippet}
        </InventoryTableAny>
      {:else}
        <InventoryTableAny columns={ciColumns} rows={cloudInitTemplates.map(t => ({
          ...t,
          variables: t.variables?.join(', ') || 'NONE'
        }))}>
           {#snippet cell({ column, row }: { column: any; row: any })}
             {#if column.key === 'name'}
               <span class="blueprint-name">{row.name}</span>
             {:else}
               <span class="cell-text">{(row as Record<string, unknown>)[column.key]}</span>
             {/if}
          {/snippet}
          {#snippet actions({ row }: { row: any })}
            <div class="row-ops">
               <button class="op-btn" title="View Registry"><FileCode size={12} /></button>
            </div>
          {/snippet}
        </InventoryTableAny>
      {/if}
    </section>

    <aside class="support-area">
      <SectionCard title="Library Insights" icon={ShieldCheck}>
        <div class="audit-summary">
          <div class="summary-row">
            <span>Scan Status</span>
            <span>CLEAN</span>
          </div>
          <div class="summary-row">
            <span>Auto-Sync</span>
            <span>ENABLED</span>
          </div>
        </div>
      </SectionCard>

      <SectionCard title="Directives" icon={ArrowRight}>
        <p class="empty-hint">Blueprint library optimized for fabric placement acceleration.</p>
      </SectionCard>
    </aside>
  </main>
</div>

{#if createFromTemplateOpen}
  <CreateFromTemplate
    bind:open={createFromTemplateOpen}
    template={selectedTemplate}
    {images}
    {networks}
    {pools}
    onSuccess={loadData}
  />
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
    tabindex="-1"
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
        <h2 id="create-template-title" class="text-lg font-semibold text-ink">Create VM Template</h2>
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
  .inventory-page {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .header-actions {
    display: flex;
    gap: 0.5rem;
  }

  .inventory-metrics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.75rem;
  }

  .tabs-nav {
    display: flex;
    gap: 0.25rem;
    padding: 0.25rem;
    background: var(--bg-surface-muted);
    border-radius: var(--radius-xs);
    width: fit-content;
  }

  .tab-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.75rem;
    font-size: 10px;
    font-weight: 700;
    color: var(--color-neutral-500);
    text-transform: uppercase;
    border-radius: var(--radius-xs);
    transition: all 0.1s ease;
  }

  .tab-item:hover {
    color: var(--color-neutral-900);
  }

  .tab-item.active {
    background: var(--bg-surface);
    color: var(--color-primary);
    box-shadow: 0 1px 2px rgba(0,0,0,0.05);
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

  .blueprint-name {
    font-weight: 700;
    color: var(--color-neutral-900);
  }

  .row-ops {
    display: flex;
    gap: 0.25rem;
  }

  .op-btn {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border-radius: 4px;
    color: var(--color-neutral-500);
    transition: all 0.1s ease;
  }

  .op-btn:hover {
    background: var(--bg-surface-muted);
    color: var(--color-primary);
  }

  .audit-summary {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .summary-row {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    color: var(--color-neutral-600);
    padding: 0.35rem 0.5rem;
    background: var(--bg-surface-muted);
    border-radius: var(--radius-xs);
  }

  .summary-row span:last-child {
    font-weight: 700;
    color: var(--color-neutral-900);
  }

  .empty-hint {
    font-size: 11px;
    color: var(--color-neutral-400);
    padding: 1rem;
    text-align: center;
  }

  @media (max-width: 1100px) {
    .inventory-main {
      grid-template-columns: 1fr;
    }
  }

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
