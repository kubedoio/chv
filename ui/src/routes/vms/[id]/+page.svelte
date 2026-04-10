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
  import Terminal from '$lib/components/Terminal.svelte';
  import VMPowerMenu from '$lib/components/VMPowerMenu.svelte';
  import BootLogViewer from '$lib/components/BootLogViewer.svelte';
  import FirewallRuleEditor from '$lib/components/FirewallRuleEditor.svelte';
  import { toast } from '$lib/stores/toast';
  import { registerShortcuts, createVMDetailShortcuts, setActiveContext } from '$lib/stores/keyboard.svelte';
  import { Play, Square, Trash2, ArrowLeft, Cpu, HardDrive, Network, Image as ImageIcon, RefreshCw, Terminal as TerminalIcon, BarChart3, RotateCcw, Camera, Edit, FileText, Shield } from 'lucide-svelte';
  import type { VM, Image, StoragePool, Network as NetworkType, VMMetrics, VMMetricsResponse, VMSnapshot } from '$lib/api/types';
  
  const client = createAPIClient({ token: getStoredToken() ?? undefined });
  const id = $derived($page.params.id ?? '');
  
  let vm = $state<VM | null>(null);
  let images = $state<Image[]>([]);
  let pools = $state<StoragePool[]>([]);
  let networks = $state<NetworkType[]>([]);
  let metricsData = $state<VMMetricsResponse | null>(null);
  let metrics = $derived(metricsData?.current);
  let metricsHistory = $derived(metricsData?.history || []);
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
  let snapshotLoading = $state(false);
  let editModalOpen = $state(false);
  
  // Derived values for display
  const imageName = $derived(
    images.find(i => i.id === vm?.image_id)?.name || vm?.image_id?.slice(0, 8) || 'Unknown'
  );
  
  const poolName = $derived(
    pools.find(p => p.id === vm?.storage_pool_id)?.name || vm?.storage_pool_id?.slice(0, 8) || 'Unknown'
  );
  
  const networkName = $derived(
    networks.find(n => n.id === vm?.network_id)?.name || vm?.network_id?.slice(0, 8) || 'Unknown'
  );
  
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
    
    loadVM();
    loadDependencies();
    startPolling();
    
    return () => {
      stopPolling();
      stopMetricsPolling();
      unregister();
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
        <div class="text-lg font-semibold truncate">{imageName}</div>
        <div class="text-sm text-muted">{vm.image_id?.slice(0, 8)}...</div>
      </div>
      
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <HardDrive size={16} />
          <span class="text-xs uppercase tracking-wider">Storage</span>
        </div>
        <div class="text-lg font-semibold">{poolName}</div>
        <div class="text-sm text-muted">{vm.storage_pool_id?.slice(0, 8)}...</div>
      </div>
      
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <Network size={16} />
          <span class="text-xs uppercase tracking-wider">Network</span>
        </div>
        <div class="text-lg font-semibold">{networkName}</div>
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
            onclick={() => openConsole()}
            class="px-4 py-2 text-sm font-medium {showTerminal ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
            title="Console (5)"
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
      <div class="space-y-4">
        {#if metrics}
          <div class="grid grid-cols-2 gap-4">
            <MetricsChart 
              title="CPU & Memory"
              data={[
                { label: 'CPU Usage', value: metrics.cpu.usage_percent, max: 100, color: '#3b82f6', history: getMetricHistory('cpu') },
                { label: 'Memory Usage', value: metrics.memory.usage_percent, max: 100, color: '#10b981', history: getMetricHistory('memory') }
              ]}
            />
            <MetricsChart 
              title="Disk I/O"
              data={[
                { label: 'Read (MB)', value: metrics.disk.read_bytes / 1024 / 1024, max: 100, color: '#f59e0b', history: getMetricHistory('disk_read') },
                { label: 'Write (MB)', value: metrics.disk.write_bytes / 1024 / 1024, max: 100, color: '#ef4444', history: getMetricHistory('disk_write') }
              ]}
            />
          </div>
          <div class="bg-white border border-line rounded p-4">
            <h3 class="text-sm font-semibold text-gray-700 mb-3">System Info</h3>
            <div class="grid grid-cols-3 gap-4 text-sm">
              <div>
                <span class="text-muted">Uptime:</span>
                <span class="ml-2">{metrics.uptime}</span>
              </div>
              <div>
                <span class="text-muted">Memory:</span>
                <span class="ml-2">{metrics.memory.used_mb} / {metrics.memory.total_mb} MB</span>
              </div>
              <div>
                <span class="text-muted">vCPUs:</span>
                <span class="ml-2">{metrics.cpu.vcpus}</span>
              </div>
            </div>
          </div>
        {:else}
          <div class="text-center py-8 text-muted">
            {vmState === 'running' ? 'Loading metrics...' : 'VM must be running to view metrics'}
          </div>
        {/if}
      </div>
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
    {/if}
  </div>
{/if}

<!-- Terminal Modal -->
{#if showTerminal && consoleWsUrl}
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
    <div class="bg-white rounded-lg shadow-lg w-full max-w-4xl">
      <div class="flex justify-between items-center px-4 py-3 border-b border-line">
        <h3 class="font-semibold">VM Console</h3>
        <button onclick={() => showTerminal = false} class="text-gray-500 hover:text-gray-700">
          Close
        </button>
      </div>
      <div class="p-4">
        <Terminal wsUrl={consoleWsUrl} onClose={() => showTerminal = false} />
      </div>
    </div>
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
</style>
