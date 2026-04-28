<script lang="ts">
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import Modal from '../primitives/Modal.svelte';
  import { toast } from '$lib/stores/toast';
  import { AlertTriangle } from 'lucide-svelte';
  import type { VM } from '$lib/api/types';
  
  interface Props {
    open?: boolean;
    vm?: VM | null;
    onSuccess?: () => void;
  }
  
  let { open = $bindable(false), vm = null, onSuccess }: Props = $props();
  
  const client = createAPIClient({ token: getStoredToken() ?? undefined });
  
  let confirming = $state(false);
  let confirmText = $state('');
  let deleting = $state(false);
  
  async function handleDelete() {
    if (confirmText !== vm?.name) {
      toast.error('Please type the VM name to confirm');
      return;
    }
    
    if (!vm) return;
    
    deleting = true;
    try {
      await client.deleteVM(vm.id);
      toast.success(`VM ${vm.name} deleted`);
      open = false;
      onSuccess?.();
    } catch (e: any) {
      toast.error(`Failed to delete VM: ${e.message}`);
    } finally {
      deleting = false;
    }
  }
  
  $effect(() => {
    if (!open) {
      confirming = false;
      confirmText = '';
    }
  });
</script>

<Modal bind:open title="Delete VM">
  {#snippet children()}
    <div>
      {#if !confirming}
        <div class="flex items-start gap-3 text-amber-700">
          <AlertTriangle size={24} />
          <div>
            <p class="font-medium">Are you sure you want to delete <strong>{vm?.name}</strong>?</p>
            <p class="text-sm text-muted mt-1">This action cannot be undone. The VM and all its data will be permanently removed.</p>
          </div>
        </div>
      {:else}
        <div class="mb-4">
          <p class="text-sm mb-2">Type <strong>{vm?.name}</strong> to confirm deletion:</p>
          <input 
            bind:value={confirmText}
            class="w-full border border-line rounded p-2"
            placeholder="Type VM name..."
          />
        </div>
      {/if}
    </div>
  {/snippet}
  
  {#snippet footer()}
    <button type="button" onclick={() => open = false} class="button-secondary">Cancel</button>
    {#if !confirming}
      <button type="button" onclick={() => confirming = true} class="button-danger">
        Delete VM
      </button>
    {:else}
      <button type="button" onclick={() => confirming = false} class="button-secondary">Back</button>
      <button type="button" onclick={handleDelete} disabled={deleting || confirmText !== vm?.name} class="button-danger">
        {deleting ? 'Deleting...' : 'Confirm Delete'}
      </button>
    {/if}
  {/snippet}
</Modal>

<style>
  .button-secondary {
    padding: 0.5rem 1rem;
    border-radius: 0.25rem;
    border: 1px solid var(--color-line);
    background: white;
    transition: background 0.15s;
  }
  .button-secondary:hover {
    background: #f5f5f5;
  }
  .button-danger {
    padding: 0.5rem 1rem;
    border-radius: 0.25rem;
    background: #E60000;
    color: white;
    border: none;
    cursor: pointer;
    transition: background 0.15s;
  }
  .button-danger:hover:not(:disabled) {
    background: #cc0000;
  }
  .button-danger:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
