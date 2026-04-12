<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Server, Copy, Trash2, Plus, FileCode, Box } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import DataTable from '$lib/components/DataTable.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import CreateFromTemplate from '$lib/components/CreateFromTemplate.svelte';
  import CloudInitViewer from '$lib/components/CloudInitViewer.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import type { VMTemplate, CloudInitTemplate, Image, Network, StoragePool } from '$lib/api/types';

  const token = getStoredToken();
  const client = createAPIClient({ token: token ?? undefined });

  // State
  let vmTemplates: VMTemplate[] = $state([]);
  let cloudInitTemplates: CloudInitTemplate[] = $state([]);
  let images: Image[] = $state([]);
  let networks: Network[] = $state([]);
  let pools: StoragePool[] = $state([]);
  let loading = $state(true);
  let error = $state('');
  let activeTab = $state<'vm' | 'cloudinit'>('vm');

  // Modal states
  let createFromTemplateOpen = $state(false);
  let selectedTemplate: VMTemplate | null = $state(null);
  let cloudInitViewerOpen = $state(false);
  let selectedCloudInitTemplate: CloudInitTemplate | null = $state(null);

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

  // Lookup maps - use functions to avoid creating new Maps on every render
  function getImage(id: string) { return images.find(i => i.id === id); }
  function getNetwork(id: string) { return networks.find(n => n.id === id); }
  function getPool(id: string) { return pools.find(p => p.id === id); }

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
      const [vmTemps, cloudTemps, imgs, nets, ps] = await Promise.all([
        client.listVMTemplates(),
        client.listCloudInitTemplates(),
        client.listImages(),
        client.listNetworks(),
        client.listStoragePools()
      ]);
      vmTemplates = vmTemps ?? [];
      cloudInitTemplates = cloudTemps ?? [];
      images = imgs ?? [];
      networks = nets ?? [];
      pools = ps ?? [];
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

<CreateFromTemplate
  bind:open={createFromTemplateOpen}
  template={selectedTemplate}
  {images}
  {networks}
  {pools}
  onSuccess={loadData}
/>

<CloudInitViewer
  bind:open={cloudInitViewerOpen}
  template={selectedCloudInitTemplate}
/>

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
