<script lang="ts">
  import { onMount } from 'svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import InstallStatusPanel from '$lib/components/InstallStatusPanel.svelte';
  import { toast } from '$lib/stores/toast';
  import type { InstallStatusResponse, InstallActionResponse } from '$lib/api/types';

  const client = createAPIClient({ token: getStoredToken() ?? undefined });

  let status: InstallStatusResponse | null = null;
  let loading = true;
  let actionLoading = false;
  let error = '';
  let lastActionResult: InstallActionResponse | null = null;

  async function loadStatus() {
    loading = true;
    error = '';
    try {
      status = await client.getInstallStatus();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load install status.';
      toast.error(error);
    } finally {
      loading = false;
    }
  }

  async function bootstrapInstall() {
    actionLoading = true;
    lastActionResult = null;
    try {
      const result = await client.bootstrapInstall();
      lastActionResult = result;
      
      if (result.errors.length > 0) {
        toast.error(`Bootstrap completed with ${result.errors.length} error(s)`);
      } else if (result.warnings.length > 0) {
        toast.success(`Bootstrap completed with ${result.warnings.length} warning(s)`);
      } else {
        toast.success(`Bootstrap completed successfully: ${result.actions_taken.join(', ')}`);
      }
      
      await loadStatus();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Bootstrap failed';
      toast.error(`Bootstrap failed: ${message}`);
    } finally {
      actionLoading = false;
    }
  }

  async function repairBridge() {
    await runRepair({ repair_bridge: true, repair_directories: false, repair_localdisk: false }, 'Bridge');
  }

  async function repairDirectories() {
    await runRepair({ repair_bridge: false, repair_directories: true, repair_localdisk: false }, 'Directories');
  }

  async function repairLocaldisk() {
    await runRepair({ repair_bridge: false, repair_directories: false, repair_localdisk: true }, 'Localdisk');
  }

  async function runRepair(body: Record<string, boolean>, name: string) {
    actionLoading = true;
    lastActionResult = null;
    try {
      const result = await client.repairInstall(body);
      lastActionResult = result;
      
      if (result.errors.length > 0) {
        toast.error(`${name} repair completed with ${result.errors.length} error(s)`);
      } else if (result.warnings.length > 0) {
        toast.success(`${name} repair completed with ${result.warnings.length} warning(s)`);
      } else {
        toast.success(`${name} repair completed: ${result.actions_taken.join(', ')}`);
      }
      
      await loadStatus();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Repair failed';
      toast.error(`${name} repair failed: ${message}`);
    } finally {
      actionLoading = false;
    }
  }

  onMount(loadStatus);
</script>

<InstallStatusPanel
  {status}
  {loading}
  {actionLoading}
  {error}
  {lastActionResult}
  handleBootstrap={bootstrapInstall}
  handleRefresh={loadStatus}
  handleRepairBridge={repairBridge}
  handleRepairDirectories={repairDirectories}
  handleRepairLocaldisk={repairLocaldisk}
/>
