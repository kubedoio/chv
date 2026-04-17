<script lang="ts">
  import Modal from '$lib/components/modals/Modal.svelte';
  import FormField from '$lib/components/forms/FormField.svelte';
  import Input from '$lib/components/Input.svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import type { VM, VMTemplate } from '$lib/api/types';

  interface Props {
    open?: boolean;
    vms?: VM[];
    onSuccess?: (template: VMTemplate) => void;
  }

  let {
    open = $bindable(false),
    vms = [],
    onSuccess
  }: Props = $props();

  const client = createAPIClient({ token: getStoredToken() ?? undefined });

  // Form state
  let sourceVMId = $state('');
  let name = $state('');
  let description = $state('');
  let vcpu = $state(2);
  let memoryMB = $state(2048);
  let cloudInitConfig = $state('');
  let tagInput = $state('');
  let tags = $state<string[]>([]);

  let submitting = $state(false);
  let formError = $state('');
  let nameError = $state('');

  const nameRegex = /^[a-z0-9-]+$/;

  function resetForm() {
    sourceVMId = '';
    name = '';
    description = '';
    vcpu = 2;
    memoryMB = 2048;
    cloudInitConfig = '';
    tagInput = '';
    tags = [];
    formError = '';
    nameError = '';
  }

  function validateName(): boolean {
    if (!name.trim()) {
      nameError = 'Name is required';
      return false;
    }
    if (!nameRegex.test(name)) {
      nameError = 'Name must contain only lowercase letters, numbers, and hyphens';
      return false;
    }
    if (name.startsWith('-') || name.endsWith('-')) {
      nameError = 'Name cannot start or end with a hyphen';
      return false;
    }
    nameError = '';
    return true;
  }

  function addTag() {
    const tag = tagInput.trim().toLowerCase();
    if (tag && !tags.includes(tag)) {
      tags = [...tags, tag];
    }
    tagInput = '';
  }

  function removeTag(tag: string) {
    tags = tags.filter(t => t !== tag);
  }

  function handleTagKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      addTag();
    }
  }

  async function handleSubmit() {
    if (!validateName()) return;

    submitting = true;
    formError = '';

    try {
      const template = await client.createVMTemplate({
        source_vm_id: sourceVMId || undefined,
        name: name.trim(),
        description: description.trim() || undefined,
        vcpu: vcpu || undefined,
        memory_mb: memoryMB || undefined,
        cloud_init_config: cloudInitConfig.trim() || undefined,
        tags: tags.length > 0 ? tags : undefined
      });

      toast.success(`Template "${template.name}" created successfully`);
      open = false;
      onSuccess?.(template);
      resetForm();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to create template';
      formError = message;
      toast.error(message);
    } finally {
      submitting = false;
    }
  }

  // Auto-fill from selected VM
  $effect(() => {
    if (sourceVMId) {
      const vm = vms.find(v => v.id === sourceVMId);
      if (vm) {
        vcpu = vm.vcpu;
        memoryMB = vm.memory_mb;
      }
    }
  });

  // Reset form when modal closes
  $effect(() => {
    if (!open) {
      resetForm();
    }
  });
</script>

<Modal bind:open title="Create VM Template" closeOnBackdrop={!submitting} width="wide">
  <form class="space-y-5" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
    {#if formError}
      <div class="rounded border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">
        {formError}
      </div>
    {/if}

    <!-- Source VM (Optional) -->
    <FormField label="Source VM (Optional)" helper="Copy configuration from an existing VM" labelFor="source-vm">
      <select
        id="source-vm"
        bind:value={sourceVMId}
        class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
        disabled={submitting}
      >
        <option value="">None (manual configuration)</option>
        {#each vms as vm}
          <option value={vm.id}>{vm.name} ({vm.vcpu} vCPU, {vm.memory_mb} MB)</option>
        {/each}
      </select>
    </FormField>

    <!-- Template Name -->
    <FormField label="Template Name" error={nameError} required labelFor="template-name">
      <Input
        id="template-name"
        bind:value={name}
        placeholder="web-server-template"
        disabled={submitting}
        onblur={validateName}
      />
    </FormField>

    <!-- Description -->
    <FormField label="Description" labelFor="template-description">
      <Input
        id="template-description"
        bind:value={description}
        placeholder="Template for web servers with nginx pre-installed"
        disabled={submitting}
      />
    </FormField>

    <!-- Resources -->
    <div class="grid grid-cols-2 gap-4">
      <FormField label="vCPUs" labelFor="template-vcpu">
        <input
          id="template-vcpu"
          type="number"
          bind:value={vcpu}
          min={1}
          max={32}
          disabled={submitting}
          class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
        />
      </FormField>
      <FormField label="Memory (MB)" labelFor="template-memory">
        <input
          id="template-memory"
          type="number"
          bind:value={memoryMB}
          min={512}
          step={512}
          disabled={submitting}
          class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
        />
      </FormField>
    </div>

    <!-- Cloud-init Config -->
    <FormField label="Default Cloud-init Config" helper="Optional default cloud-init configuration" labelFor="template-cloudinit">
      <textarea
        id="template-cloudinit"
        bind:value={cloudInitConfig}
        placeholder="#cloud-config\npackages:\n  - nginx"
        class="w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 font-mono text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
        rows={5}
        disabled={submitting}
      ></textarea>
    </FormField>

    <!-- Tags -->
    <FormField label="Tags" helper="Press Enter to add a tag" labelFor="template-tags">
      <div class="space-y-2">
        <div class="flex gap-2">
          <Input
            id="template-tags"
            bind:value={tagInput}
            placeholder="web-server"
            disabled={submitting}
            onkeydown={handleTagKeydown}
          />
          <button
            type="button"
            onclick={addTag}
            disabled={submitting || !tagInput.trim()}
            class="px-3 py-2 rounded bg-chrome text-ink font-medium hover:bg-neutral-200 transition-colors disabled:opacity-50"
          >
            Add
          </button>
        </div>
        {#if tags.length > 0}
          <div class="flex flex-wrap gap-2">
            {#each tags as tag}
              <span class="inline-flex items-center gap-1 px-2 py-1 rounded bg-primary/10 text-primary text-xs">
                {tag}
                <button
                  type="button"
                  onclick={() => removeTag(tag)}
                  class="hover:text-primary/70"
                  disabled={submitting}
                >
                  ×
                </button>
              </span>
            {/each}
          </div>
        {/if}
      </div>
    </FormField>
  </form>

  {#snippet footer()}
    <button
      type="button"
      onclick={() => open = false}
      disabled={submitting}
      class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    >
      Cancel
    </button>
    <button
      type="button"
      onclick={handleSubmit}
      disabled={submitting || !name.trim()}
      class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2"
    >
      {#if submitting}
        <svg class="animate-spin h-4 w-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" aria-hidden="true">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
        </svg>
      {/if}
      {submitting ? 'Creating...' : 'Create Template'}
    </button>
  {/snippet}
</Modal>
