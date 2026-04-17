<script lang="ts">
  import { Camera, RotateCcw, Trash2, Clock, CheckCircle, AlertCircle } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import type { VMSnapshot } from '$lib/api/types';

  interface Props {
    vmId: string;
    vmState: string;
  }

  let { vmId, vmState }: Props = $props();

  const client = createAPIClient({ token: getStoredToken() ?? undefined });

  let snapshots = $state<VMSnapshot[]>([]);
  let loading = $state(true);
  let snapshotLoading = $state(false);

  const canManageSnapshots = $derived(vmState === 'stopped' || vmState === 'prepared');

  async function loadSnapshots() {
    loading = true;
    try {
      snapshots = await client.listVMSnapshots(vmId);
    } catch (e: any) {
      toast.error('Failed to load snapshots');
    } finally {
      loading = false;
    }
  }

  async function createSnapshot() {
    if (!canManageSnapshots) {
      toast.error('VM must be stopped to create a snapshot');
      return;
    }

    snapshotLoading = true;
    try {
      await client.createVMSnapshot(vmId);
      toast.success('Snapshot created successfully');
      await loadSnapshots();
    } catch (e: any) {
      toast.error(e.message || 'Failed to create snapshot');
    } finally {
      snapshotLoading = false;
    }
  }

  async function restoreSnapshot(snapId: string) {
    if (!canManageSnapshots) {
      toast.error('VM must be stopped to restore a snapshot');
      return;
    }

    if (!confirm('Are you sure you want to restore this snapshot? Current disk state will be lost.')) {
      return;
    }

    snapshotLoading = true;
    try {
      await client.restoreVMSnapshot(vmId, snapId);
      toast.success('Snapshot restored successfully');
    } catch (e: any) {
      toast.error(e.message || 'Failed to restore snapshot');
    } finally {
      snapshotLoading = false;
    }
  }

  async function deleteSnapshot(snapId: string) {
    if (!confirm('Are you sure you want to delete this snapshot?')) {
      return;
    }

    snapshotLoading = true;
    try {
      await client.deleteVMSnapshot(vmId, snapId);
      toast.success('Snapshot deleted');
      await loadSnapshots();
    } catch (e: any) {
      toast.error(e.message || 'Failed to delete snapshot');
    } finally {
      snapshotLoading = false;
    }
  }

  function formatDate(dateStr: string): string {
    const date = new Date(dateStr);
    return date.toLocaleString();
  }

  // Load snapshots when component mounts
  $effect(() => {
    loadSnapshots();
  });
</script>

<div class="space-y-4">
  <!-- Header -->
  <div class="flex justify-between items-center bg-white border border-line p-4 rounded">
    <div>
      <h3 class="font-semibold text-gray-800 flex items-center gap-2">
        <Camera size={18} />
        VM Snapshots
      </h3>
      <p class="text-xs text-muted mt-1">
        Internal snapshots stored within the qcow2 disk file.
        {#if !canManageSnapshots}
          <span class="text-amber-600">VM must be stopped to manage snapshots.</span>
        {/if}
      </p>
    </div>
    <button 
      onclick={createSnapshot} 
      disabled={snapshotLoading || !canManageSnapshots} 
      class="button-primary flex items-center gap-2"
    >
      <Camera size={16} />
      {snapshotLoading ? 'Creating...' : 'Take Snapshot'}
    </button>
  </div>

  <!-- Snapshots List -->
  {#if loading}
    <div class="card p-8 text-center text-muted">
      <div class="animate-spin h-8 w-8 border-2 border-primary border-t-transparent rounded-full mx-auto mb-4"></div>
      <p>Loading snapshots...</p>
    </div>
  {:else if snapshots.length === 0}
    <div class="card p-12 text-center text-muted border-dashed border-2">
      <Camera size={48} class="mx-auto mb-4 opacity-20" />
      <p>No snapshots found for this VM.</p>
      <p class="text-xs mt-1">Snapshots allow you to save the disk state and revert back to it later.</p>
    </div>
  {:else}
    <div class="card overflow-hidden">
      <table class="w-full text-left border-collapse">
        <thead>
          <tr class="bg-gray-50 text-xs uppercase tracking-wider text-muted border-b border-line">
            <th class="px-4 py-3 font-semibold">Name</th>
            <th class="px-4 py-3 font-semibold">Created</th>
            <th class="px-4 py-3 font-semibold">Status</th>
            <th class="px-4 py-3 font-semibold text-right">Actions</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-line text-sm bg-white">
          {#each snapshots as snap (snap.id)}
            <tr class="hover:bg-gray-50 transition-colors">
              <td class="px-4 py-3 font-medium text-gray-800">{snap.name}</td>
              <td class="px-4 py-3 text-muted">
                <span class="flex items-center gap-1">
                  <Clock size={14} />
                  {formatDate(snap.created_at)}
                </span>
              </td>
              <td class="px-4 py-3">
                <span class="px-2 py-0.5 rounded-full text-xs font-semibold bg-emerald-50 text-emerald-700 border border-emerald-200">
                  {snap.status}
                </span>
              </td>
              <td class="px-4 py-3 text-right">
                <div class="flex justify-end gap-1">
                  <button 
                    onclick={() => restoreSnapshot(snap.id)} 
                    disabled={snapshotLoading || !canManageSnapshots}
                    class="p-2 hover:bg-emerald-50 rounded text-emerald-600 border border-transparent hover:border-emerald-200 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
                    title="Restore Snapshot"
                  >
                    <RotateCcw size={16} />
                  </button>
                  <button 
                    onclick={() => deleteSnapshot(snap.id)} 
                    disabled={snapshotLoading}
                    class="p-2 hover:bg-rose-50 rounded text-rose-600 border border-transparent hover:border-rose-200 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
                    title="Delete Snapshot"
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <!-- Snapshot Timeline -->
    <div class="bg-white border border-line rounded p-4">
      <h4 class="text-sm font-medium text-ink mb-4">Snapshot Timeline</h4>
      <div class="relative">
        <div class="absolute left-3 top-0 bottom-0 w-0.5 bg-line"></div>
        <div class="space-y-4">
          {#each snapshots.slice().reverse() as snap, i (snap.id)}
            <div class="relative flex items-start gap-4 pl-8">
              <div class="absolute left-0 top-1 w-6 h-6 rounded-full bg-white border-2 border-primary flex items-center justify-center">
                <Camera size={12} class="text-primary" />
              </div>
              <div class="flex-1 bg-chrome rounded p-3">
                <div class="flex items-center justify-between">
                  <span class="font-medium text-sm">{snap.name}</span>
                  <span class="text-xs text-muted">{formatDate(snap.created_at)}</span>
                </div>
                <div class="text-xs text-muted mt-1">
                  Status: <span class="text-emerald-600">{snap.status}</span>
                </div>
              </div>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .card {
    background: white;
    border: 1px solid var(--color-line);
    border-radius: 0.5rem;
  }

  .button-primary {
    padding: 0.5rem 1rem;
    border-radius: 0.25rem;
    background: var(--color-primary);
    color: white;
    border: none;
    cursor: pointer;
    transition: background 0.15s;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .button-primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-primary) 90%, black);
  }

  .button-primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
