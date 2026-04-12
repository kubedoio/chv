<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { onMount, onDestroy } from 'svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import StatusIndicator from '$lib/components/StatusIndicator.svelte';
  import CloudInitPreview from '$lib/components/CloudInitPreview.svelte';
  import DeleteVMModal from '$lib/components/DeleteVMModal.svelte';
  import MetricsChart from '$lib/components/MetricsChart.svelte';
  import VMMetricsHistory from '$lib/components/VMMetricsHistory.svelte';
  import Terminal from '$lib/components/Terminal.svelte';
  
  import VMPowerMenu from '$lib/components/VMPowerMenu.svelte';
  import BootLogViewer from '$lib/components/BootLogViewer.svelte';
  import FirewallRuleEditor from '$lib/components/FirewallRuleEditor.svelte';
  import { toast } from '$lib/stores/toast';
  import { registerShortcuts, createVMDetailShortcuts, setActiveContext } from '$lib/stores/keyboard.svelte.ts';
  import { Play, Square, Trash2, ArrowLeft, Cpu, HardDrive, Network, Image as ImageIcon, RefreshCw, Terminal as TerminalIcon, BarChart3, RotateCcw, Camera, Edit, FileText, Shield, Maximize2, Minimize2 } from 'lucide-svelte';
  import type { VM, Image, StoragePool, Network as NetworkType, VMMetrics, VMMetricsResponse, VMSnapshot } from '$lib/api/types';
  
  const client = createAPIClient({ token: getStoredToken() ?? undefined });
  const id = $derived($page.params.id ?? '');
  
  let vm = $state<VM | null>(null);
  let images = $state<Image[]>([]);
  let pools = $state<StoragePool[]>([]);
  let networks = $state<NetworkType[]>([]);
  let metricsData = $state<VMMetricsResponse | null>(null);
  let metrics = $derived(metricsData?.current);
  
  // Use a stable empty array reference
  const EMPTY_ARRAY: VMMetrics[] = [];
  let metricsHistory = $derived(metricsData?.history ?? EMPTY_ARRAY);
  let loading = $state(true);
  let actionLoading = $state(false);
  let deleteModalOpen = $state(false);
  let pollInterval = $state<number | null>(null);
  let metricsInterval = $state<number | null>(null);
  let lastUpdated = $state<Date | null>(null);
  let activeTab = $state<'overview' | 'metrics' | 'snapshots' | 'console' | 'boot-logs' | 'firewall'>('overview');
  let snapshots = $state<VMSnapshot[]>([]);
  let consoleWsUrl = $state<string>('');
  let showTerminal = $state(false);
  let consoleFullscreen = $state(false);
  
  let snapshotLoading = $state(false);
  let editModalOpen = $state(false);
  
  // Helper functions for display values (not derived to avoid re-renders)
  function getImageName() {
    if (!vm) return 'Unknown';
    const img = images.find(i => i.id === vm?.image_id);
    return img?.name || vm?.image_id?.slice(0, 8) || 'Unknown';
  }
  
  function getPoolName() {
    if (!vm) return 'Unknown';
    const pool = pools.find(p => p.id === vm?.storage_pool_id);
    return pool?.name || vm?.storage_pool_id?.slice(0, 8) || 'Unknown';
  }
  
  function getNetworkName() {
    if (!vm) return 'Unknown';
    const net = networks.find(n => n.id === vm?.network_id);
    return net?.name || vm?.network_id?.slice(0, 8) || 'Unknown';
  }
  
  const vmState = $derived(vm?.actual_state || 'unknown');
  
  // Determine if we should poll based on VM state
  const shouldPoll = $derived(
    ['running', 'starting', 'stopping'].includes(vmState.toLowerCase())
  );
  
  // Tab index mapping for keyboard shortcuts
  const tabOrder: ('overview' | 'metrics' | 'snapshots' | 'console' | 'boot-logs' | 'firewall')[] = ['overview', 'metrics', 'snapshots', 'boot-logs', 'firewall', 'console'];
  
  // Keyboard shortcut handlers
  const keyboardHandlers = {
    onEdit: () => editModalOpen = true,
    onStart: startVM,
    onStop: stopVM,
    onRestart: restartVM,
    onDelete: () => deleteModalOpen = true,
    onTabChange: (tabIndex: number) => {
      const tab = tabOrder[tabIndex];
      if (tab) {
        handleTabChange(tab);
      }
    }
  };
  
  onMount(() => {
    // Check auth
    if (!getStoredToken()) {
      goto('/login');
      return;
    }

    // Set active context for keyboard shortcuts
    setActiveContext('vm-detail');

    // Register VM detail shortcuts
    const unregister = registerShortcuts(createVMDetailShortcuts(keyboardHandlers));

    // Escape key to exit fullscreen console
    const handleKeydown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && consoleFullscreen) {
        consoleFullscreen = false;
      }
    };
    document.addEventListener('keydown', handleKeydown);

    loadVM();
    loadDependencies();
    startPolling();

    return () => {
      stopPolling();
      stopMetricsPolling();
      unregister();
      document.removeEventListener('keydown', handleKeydown);
    };
  });
  
  onDestroy(() => {
    stopPolling();
    stopMetricsPolling();
  });
  
  function startPolling() {
    // Poll every 3 seconds for transient states, every 10 seconds otherwise
    const interval = shouldPoll ? 3000 : 10000;
    pollInterval = window.setInterval(() => {
      refreshVM();
    }, interval);
  }
  
  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }
  
  function startMetricsPolling() {
    // Poll metrics every 30 seconds
    if (metricsInterval) {
      clearInterval(metricsInterval);
    }
    metricsInterval = window.setInterval(() => {
      loadMetrics();
    }, 30000);
  }
  
  function stopMetricsPolling() {
    if (metricsInterval) {
      clearInterval(metricsInterval);
      metricsInterval = null;
    }
  }

  // Helper for metrics charts
  function getMetricHistory(key: 'cpu' | 'memory' | 'disk_read' | 'disk_write') {
    return metricsHistory.map(m => {
      if (key === 'cpu') return m.cpu.usage_percent;
      if (key === 'memory') return m.memory.usage_percent;
      if (key === 'disk_read') return m.disk.read_bytes / 1024 / 1024;
      if (key === 'disk_write') return m.disk.write_bytes / 1024 / 1024;
      return 0;
    });
  }
  
  // Handle tab switching - start/stop metrics polling
  function handleTabChange(tab: 'overview' | 'metrics' | 'snapshots' | 'console' | 'firewall') {
    activeTab = tab;
    if (tab === 'metrics' && vmState === 'running') {
      loadMetrics();
      startMetricsPolling();
    } else {
      stopMetricsPolling();
    }

    if (tab === 'snapshots') {
      loadSnapshots();
    }
    
    if (tab === 'console') {
      openConsole();
    }
  }

  async function loadSnapshots() {
    try {
      snapshots = await client.listVMSnapshots(id);
    } catch (e: any) {
      console.error('Failed to load snapshots:', e);
    }
  }

  async function createSnapshot() {
    if (vmState !== 'stopped' && vmState !== 'prepared') {
      toast.error('VM must be stopped to create a snapshot');
      return;
    }

    snapshotLoading = true;
    try {
      await client.createVMSnapshot(id);
      toast.success('Snapshot created');
      await loadSnapshots();
    } catch (e: any) {
      toast.error(e.message || 'Failed to create snapshot');
    } finally {
      snapshotLoading = false;
    }
  }

  async function restoreSnapshot(snapID: string) {
    if (vmState !== 'stopped' && vmState !== 'prepared') {
      toast.error('VM must be stopped to restore a snapshot');
      return;
    }

    if (!confirm('Are you sure you want to restore this snapshot? Current disk state will be lost.')) {
      return;
    }

    snapshotLoading = true;
    try {
      await client.restoreVMSnapshot(id, snapID);
      toast.success('Snapshot restored');
      await refreshVM();
    } catch (e: any) {
      toast.error(e.message || 'Failed to restore snapshot');
    } finally {
      snapshotLoading = false;
    }
  }

  async function deleteSnapshot(snapID: string) {
    if (!confirm('Are you sure you want to delete this snapshot?')) {
      return;
    }

    snapshotLoading = true;
    try {
      await client.deleteVMSnapshot(id, snapID);
      toast.success('Snapshot deleted');
      await loadSnapshots();
    } catch (e: any) {
      toast.error(e.message || 'Failed to delete snapshot');
    } finally {
      snapshotLoading = false;
    }
  }
  
  async function loadVM() {
    loading = true;
    try {
      vm = await client.getVM(id);
      consoleType = vm?.console_type || 'pty';
      lastUpdated = new Date();
    } catch (e) {
      toast.error('Failed to load VM');
      goto('/vms');
    } finally {
      loading = false;
    }
  }
  
  async function refreshVM() {
    try {
      // Use lightweight status endpoint for polling
      const status = await client.getVMStatus(id);
      
      // Merge status into existing VM object
      if (vm) {
        vm = {
          ...vm,
          actual_state: status.actual_state,
          desired_state: status.desired_state,
          // Note: cloud_hypervisor_pid not in VM type, using extended merge
          last_error: status.last_error
          // Note: updated_at not in VM type
        };
      }
      lastUpdated = new Date();
    } catch (e) {
      console.error('Failed to refresh VM status:', e);
    }
  }
  
  async function loadDependencies() {
    try {
      const [imgs, ps, nets] = await Promise.all([
        client.listImages(),
        client.listStoragePools(),
        client.listNetworks()
      ]);
      images = imgs;
      pools = ps;
      networks = nets;
    } catch (e) {
      console.error('Failed to load dependencies:', e);
    }
  }
  
  async function loadMetrics() {
    if (vmState !== 'running') {
      metricsData = null;
      return;
    }
    try {
      metricsData = await client.getVMMetrics(id);
    } catch (e) {
      console.error('Failed to load metrics:', e);
    }
  }
  
  async function openConsole() {
    try {
      const resp = await client.getVMConsoleURL(id);
      consoleWsUrl = resp.ws_url;
      showTerminal = true;
    } catch (e: any) {
      toast.error(e.message || 'Failed to get console URL');
    }
  }

  function toggleConsoleFullscreen() {
    consoleFullscreen = !consoleFullscreen;
  }
  
  async function startVM() {
    actionLoading = true;
    try {
      await client.startVM(id);
      toast.success('VM starting...');
      await loadVM();
    } catch (e: any) {
      toast.error(e.message || 'Failed to start VM');
    } finally {
      actionLoading = false;
    }
  }
  
  async function stopVM() {
    actionLoading = true;
    try {
      await client.stopVM(id);
      toast.success('VM stopping...');
      await loadVM();
    } catch (e: any) {
      toast.error(e.message || 'Failed to stop VM');
    } finally {
      actionLoading = false;
    }
  }
  
  async function restartVM() {
    actionLoading = true;
    try {
      await client.restartVM(id);
      toast.success('VM restarting...');
      await loadVM();
    } catch (e: any) {
      toast.error(e.message || 'Failed to restart VM');
    } finally {
      actionLoading = false;
    }
  }

  async function handlePowerAction(action: string, options?: { graceful?: boolean; timeout?: number }) {
    actionLoading = true;
    try {
      switch (action) {
        case 'start':
          await client.startVM(id);
          toast.success('VM starting...');
          break;
        case 'shutdown':
          await client.shutdownVM(id, options?.timeout);
          toast.success('Shutdown signal sent');
          break;
        case 'force-stop':
          await client.forceStopVM(id);
          toast.success('VM force stopped');
          break;
        case 'reset':
          await client.resetVM(id);
          toast.success('VM reset initiated');
          break;
        case 'restart':
          if (options?.graceful) {
            await client.restartVMWithOptions(id, true, options.timeout);
          } else {
            await client.restartVM(id);
          }
          toast.success('VM restarting...');
          break;
      }
      await loadVM();
    } catch (e: any) {
      toast.error(e.message || `Failed to ${action} VM`);
    } finally {
      actionLoading = false;
    }
  }
</script>

{#if loading}
  <div class="flex items-center justify-center h-64">
    <div class="text-muted">Loading...</div>
  </div>
{:else if vm}
  <div class="vm-detail">
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <div class="flex items-center gap-4">
        <button onclick={() => goto('/vms')} class="p-2 hover:bg-chrome rounded" title="Back to VMs">
          <ArrowLeft size={20} />
        </button>
        <div>
          <h1 class="text-2xl font-bold">{vm.name}</h1>
          <p class="text-sm text-muted">{vm.id}</p>
        </div>
        <StateBadge label={vmState} />
        {#if shouldPoll}
          <StatusIndicator status={vmState} size="sm" showLabel={false} />
        {/if}
      </div>
      <div class="flex items-center gap-3">
        {#if lastUpdated}
          <span class="text-xs text-gray-500">
            Updated {lastUpdated.toLocaleTimeString()}
          </span>
        {/if}
        <button 
          onclick={refreshVM} 
          disabled={actionLoading}
          class="p-2 hover:bg-chrome rounded"
          title="Refresh (R)"
        >
          <RefreshCw size={16} class={actionLoading ? 'animate-spin' : ''} />
        </button>
      
        <VMPowerMenu 
          vmState={vmState} 
          disabled={actionLoading}
          onAction={handlePowerAction}
        />
        
        <button onclick={() => editModalOpen = true} class="button-secondary flex items-center gap-2" title="Edit (E)">
          <Edit size={16} />
          Edit
        </button>
        <button onclick={() => deleteModalOpen = true} class="button-danger flex items-center gap-2" title="Delete (Del)">
          <Trash2 size={16} />
          Delete
        </button>
      </div>
    </div>
    
    <!-- Info Cards -->
    <div class="grid grid-cols-4 gap-4 mb-6">
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <Cpu size={16} />
          <span class="text-xs uppercase tracking-wider">Resources</span>
        </div>
        <div class="text-lg font-semibold">{vm.vcpu} vCPU</div>
        <div class="text-sm text-muted">{vm.memory_mb} MB RAM</div>
        <!-- PID info removed - not available in VM type -->
      </div>
      
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <ImageIcon size={16} />
          <span class="text-xs uppercase tracking-wider">Image</span>
        </div>
        <div class="text-lg font-semibold truncate">{getImageName()}</div>
        <div class="text-sm text-muted">{vm.image_id?.slice(0, 8)}...</div>
      </div>
      
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <HardDrive size={16} />
          <span class="text-xs uppercase tracking-wider">Storage</span>
        </div>
        <div class="text-lg font-semibold">{getPoolName()}</div>
        <div class="text-sm text-muted">{vm.storage_pool_id?.slice(0, 8)}...</div>
      </div>
      
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <Network size={16} />
          <span class="text-xs uppercase tracking-wider">Network</span>
        </div>
        <div class="text-lg font-semibold">{getNetworkName()}</div>
        <div class="text-sm text-muted">{vm.ip_address || 'No IP'}</div>
      </div>
    </div>
    
    <!-- Last Error -->
    {#if vm.last_error}
      <div class="bg-[#FFF0F0] border border-[#E60000] text-[#E60000] p-4 rounded mb-6">
        <div class="font-medium mb-1">Last Error</div>
        <div class="text-sm">{vm.last_error}</div>
      </div>
    {/if}
    
    <!-- Tabs -->
    <div class="border-b border-line mb-6">
      <div class="flex gap-1">
        <button 
          onclick={() => handleTabChange('overview')}
          class="px-4 py-2 text-sm font-medium {activeTab === 'overview' ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
          title="Overview (1)"
        >
          Overview
        </button>
        <button 
          onclick={() => handleTabChange('metrics')}
          class="px-4 py-2 text-sm font-medium {activeTab === 'metrics' ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
          title="Metrics (2)"
        >
          <BarChart3 size={16} class="inline mr-1" />
          Metrics
        </button>
        <button 
          onclick={() => handleTabChange('snapshots')}
          class="px-4 py-2 text-sm font-medium {activeTab === 'snapshots' ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
          title="Snapshots (3)"
        >
          <Camera size={16} class="inline mr-1" />
          Snapshots
        </button>
        <button 
          onclick={() => handleTabChange('boot-logs')}
          class="px-4 py-2 text-sm font-medium {activeTab === 'boot-logs' ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
          title="Boot Logs (4)"
        >
          <FileText size={16} class="inline mr-1" />
          Boot Logs
        </button>
        <button 
          onclick={() => handleTabChange('firewall')}
          class="px-4 py-2 text-sm font-medium {activeTab === 'firewall' ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
          title="Firewall (5)"
        >
          <Shield size={16} class="inline mr-1" />
          Firewall
        </button>
        {#if vmState === 'running'}
          <button
            onclick={() => handleTabChange('console')}
            class="px-4 py-2 text-sm font-medium {activeTab === 'console' ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
            title="Console (6)"
          >
            <TerminalIcon size={16} class="inline mr-1" />
            Console
          </button>
        {/if}
      </div>
    </div>
    
    <!-- Tab Content -->
    {#if activeTab === 'overview'}
      <!-- Cloud-init Preview -->
      <div class="card p-4">
        <CloudInitPreview 
          userData={vm.user_data}
          metaData={vm.meta_data}
          networkConfig={vm.network_config}
        />
      </div>
    {:else if activeTab === 'metrics'}
      {#if vmState === 'running'}
        <VMMetricsHistory vmId={id} />
      {:else}
        <div class="text-center py-12 bg-white border border-line rounded-lg">
          <BarChart3 size={48} class="mx-auto text-slate-300 mb-4" />
          <h3 class="text-lg font-medium text-slate-900 mb-1">Metrics Unavailable</h3>
          <p class="text-sm text-slate-500">VM must be running to view metrics history</p>
          <button 
            onclick={startVM} 
            disabled={actionLoading}
            class="mt-4 px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50"
          >
            Start VM
          </button>
        </div>
      {/if}
    {:else if activeTab === 'snapshots'}
      <div class="space-y-4">
        <div class="flex justify-between items-center bg-white border border-line p-4 rounded">
          <div>
            <h3 class="font-semibold text-gray-800">Local Disk Snapshots</h3>
            <p class="text-xs text-muted">Internal snapshots stored within the qcow2 disk header.</p>
          </div>
          <button 
            onclick={createSnapshot} 
            disabled={snapshotLoading || (vmState !== 'stopped' && vmState !== 'prepared')} 
            class="button-primary flex items-center gap-2"
          >
            <Camera size={16} />
            {snapshotLoading ? 'Creating...' : 'Take Snapshot'}
          </button>
        </div>

        {#if snapshots.length === 0}
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
                {#each snapshots as snap}
                  <tr class="hover:bg-gray-50 transition-colors">
                    <td class="px-4 py-3 font-medium text-gray-800">{snap.name}</td>
                    <td class="px-4 py-3 text-muted">{new Date(snap.created_at).toLocaleString()}</td>
                    <td class="px-4 py-3">
                      <span class="px-2 py-0.5 rounded-full text-xs font-semibold bg-emerald-50 text-emerald-700 border border-emerald-200">
                        {snap.status}
                      </span>
                    </td>
                    <td class="px-4 py-3 text-right">
                      <div class="flex justify-end gap-1">
                        <button 
                          onclick={() => restoreSnapshot(snap.id)} 
                          disabled={snapshotLoading || (vmState !== 'stopped' && vmState !== 'prepared')}
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
        {/if}
      </div>
    {:else if activeTab === 'boot-logs'}
      <div class="space-y-4">
        <BootLogViewer {vmId} />
      </div>
    {:else if activeTab === 'firewall'}
      <div class="space-y-4">
        <FirewallRuleEditor {vmId} />
      </div>
    {:else if activeTab === 'console'}
      <div class="space-y-4" class:console-fullscreen={consoleFullscreen}>
        <div class="flex justify-between items-center">
          <div class="text-sm text-muted">
            {#if consoleFullscreen}
              Fullscreen mode — press Escape or click the button to exit
            {:else}
              VM Serial Console
            {/if}
          </div>
          <button
            onclick={toggleConsoleFullscreen}
            class="p-2 hover:bg-chrome rounded flex items-center gap-2 text-sm text-muted"
            title={consoleFullscreen ? 'Exit fullscreen' : 'Fullscreen'}
          >
            {#if consoleFullscreen}
              <Minimize2 size={16} />
            {:else}
              <Maximize2 size={16} />
            {/if}
          </button>
        </div>
        {#if showTerminal && consoleWsUrl}
          <div class="terminal-wrapper" class:terminal-fullscreen={consoleFullscreen}>
            <Terminal wsUrl={consoleWsUrl} onClose={() => { showTerminal = false; activeTab = 'overview'; }} fullscreen={consoleFullscreen} />
          </div>
        {:else}
          <div class="card p-8 text-center text-muted">
            <TerminalIcon size={32} class="mx-auto mb-3 opacity-40" />
            <p>Console not available. Make sure the VM is running.</p>
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<DeleteVMModal 
  bind:open={deleteModalOpen}
  {vm}
  onSuccess={() => goto('/vms')}
/>

<style>
  .vm-detail {
    max-width: 72rem;
  }
  .card {
    background: white;
    border: 1px solid var(--color-line);
    border-radius: 0.25rem;
  }
  .button-primary {
    padding: 0.5rem 1rem;
    border-radius: 0.25rem;
    background: var(--color-primary);
    color: white;
    border: none;
    cursor: pointer;
    transition: background 0.15s;
  }
  .button-primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-primary) 90%, black);
  }
  .button-primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .button-secondary {
    padding: 0.5rem 1rem;
    border-radius: 0.25rem;
    border: 1px solid var(--color-line);
    background: white;
    cursor: pointer;
    transition: background 0.15s;
  }
  .button-secondary:hover:not(:disabled) {
    background: #f5f5f5;
  }
  .button-secondary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
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
  .button-danger:hover {
    background: #cc0000;
  }

  .console-fullscreen {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 100;
    background: var(--color-bg);
    padding: 1rem;
    display: flex;
    flex-direction: column;
  }

  .terminal-wrapper {
    flex: 1;
    min-height: 400px;
    display: flex;
    flex-direction: column;
  }

  .terminal-fullscreen {
    min-height: unset;
    height: 100%;
  }
</style>
