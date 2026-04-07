<script lang="ts">
  import { onMount } from 'svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import InstallStatusPanel from '$lib/components/InstallStatusPanel.svelte';
  import type { InstallStatusResponse } from '$lib/api/types';

  const client = createAPIClient({ token: getStoredToken() ?? undefined });

  let status: InstallStatusResponse | null = null;
  let loading = true;
  let error = '';

  async function loadStatus() {
    loading = true;
    error = '';
    try {
      status = await client.getInstallStatus();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load install status.';
    } finally {
      loading = false;
    }
  }

  async function bootstrapInstall() {
    await client.bootstrapInstall();
    await loadStatus();
  }

  async function repairBridge() {
    await client.repairInstall({ repair_bridge: true, repair_directories: false, repair_localdisk: false });
    await loadStatus();
  }

  async function repairDirectories() {
    await client.repairInstall({ repair_bridge: false, repair_directories: true, repair_localdisk: false });
    await loadStatus();
  }

  async function repairLocaldisk() {
    await client.repairInstall({ repair_bridge: false, repair_directories: false, repair_localdisk: true });
    await loadStatus();
  }

  onMount(loadStatus);
</script>

<InstallStatusPanel
  {status}
  {loading}
  {error}
  handleBootstrap={bootstrapInstall}
  handleRefresh={loadStatus}
  handleRepairBridge={repairBridge}
  handleRepairDirectories={repairDirectories}
  handleRepairLocaldisk={repairLocaldisk}
/>

