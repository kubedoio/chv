<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import { ArrowLeft, Plus, Trash2, Network as NetworkIcon, Settings, Tag, Shield, Play, Square } from 'lucide-svelte';
  import type { Network, VLANNetwork, DHCPLease, DHCPServerConfig } from '$lib/api/types';
  
  const client = createAPIClient({ token: getStoredToken() ?? undefined });
  const id = $derived($page.params.id ?? '');
  
  let network = $state<Network | null>(null);
  let vlans = $state<VLANNetwork[]>([]);
  let dhcpConfig = $state<DHCPServerConfig | null>(null);
  let dhcpLeases = $state<DHCPLease[]>([]);
  let loading = $state(true);
  let activeTab = $state<'overview' | 'vlans' | 'dhcp'>('overview');
  
  // VLAN form state
  let showVLANForm = $state(false);
  let vlanForm = $state({
    vlan_id: 100,
    name: '',
    cidr: '',
    gateway_ip: ''
  });
  
  // DHCP form state
  let showDHCPForm = $state(false);
  let dhcpForm = $state({
    range_start: '',
    range_end: '',
    lease_time_seconds: 3600
  });
  
  onMount(() => {
    if (!getStoredToken()) {
      goto('/login');
      return;
    }
    loadNetwork();
    loadVLANs();
    loadDHCPStatus();
  });
  
  async function loadNetwork() {
    loading = true;
    try {
      network = await client.getNetwork(id);
    } catch (e) {
      toast.error('Failed to load network');
      goto('/networks');
    } finally {
      loading = false;
    }
  }
  
  async function loadVLANs() {
    try {
      vlans = await client.listVLANs(id);
    } catch (e) {
      console.error('Failed to load VLANs:', e);
    }
  }
  
  async function loadDHCPStatus() {
    try {
      dhcpConfig = await client.getDHCPStatus(id);
      if (dhcpConfig?.configured) {
        loadDHCPLeases();
      }
    } catch (e) {
      console.error('Failed to load DHCP status:', e);
    }
  }
  
  async function loadDHCPLeases() {
    try {
      dhcpLeases = await client.getDHCPLeases(id);
    } catch (e) {
      console.error('Failed to load DHCP leases:', e);
    }
  }
  
  async function createVLAN() {
    try {
      await client.createVLAN(id, vlanForm);
      toast.success('VLAN created');
      showVLANForm = false;
      vlanForm = { vlan_id: 100, name: '', cidr: '', gateway_ip: '' };
      loadVLANs();
    } catch (e: any) {
      toast.error(e.message || 'Failed to create VLAN');
    }
  }
  
  async function deleteVLAN(vlanId: string) {
    if (!confirm('Are you sure you want to delete this VLAN?')) return;
    try {
      await client.deleteVLAN(id, vlanId);
      toast.success('VLAN deleted');
      loadVLANs();
    } catch (e: any) {
      toast.error(e.message || 'Failed to delete VLAN');
    }
  }
  
  async function configureDHCP() {
    try {
      await client.configureDHCP(id, dhcpForm);
      toast.success('DHCP server configured');
      showDHCPForm = false;
      loadDHCPStatus();
    } catch (e: any) {
      toast.error(e.message || 'Failed to configure DHCP');
    }
  }
  
  async function startDHCPServer() {
    try {
      await client.startDHCPServer(id);
      toast.success('DHCP server started');
      loadDHCPStatus();
    } catch (e: any) {
      toast.error(e.message || 'Failed to start DHCP server');
    }
  }
  
  async function stopDHCPServer() {
    try {
      await client.stopDHCPServer(id);
      toast.success('DHCP server stopped');
      loadDHCPStatus();
    } catch (e: any) {
      toast.error(e.message || 'Failed to stop DHCP server');
    }
  }
</script>

{#if loading}
  <div class="flex items-center justify-center h-64">
    <div class="text-muted">Loading...</div>
  </div>
{:else if network}
  <div class="max-w-6xl">
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <div class="flex items-center gap-4">
        <button onclick={() => goto('/networks')} class="p-2 hover:bg-chrome rounded" title="Back to Networks">
          <ArrowLeft size={20} />
        </button>
        <div>
          <h1 class="text-2xl font-bold">{network.name}</h1>
          <p class="text-sm text-muted">{network.id}</p>
        </div>
        <StateBadge label={network.status} />
      </div>
    </div>
    
    <!-- Info Cards -->
    <div class="grid grid-cols-4 gap-4 mb-6">
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <Network size={16} />
          <span class="text-xs uppercase tracking-wider">Mode</span>
        </div>
        <div class="text-lg font-semibold">{network.mode}</div>
      </div>
      
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <Settings size={16} />
          <span class="text-xs uppercase tracking-wider">Bridge</span>
        </div>
        <div class="text-lg font-semibold">{network.bridge_name}</div>
      </div>
      
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <Tag size={16} />
          <span class="text-xs uppercase tracking-wider">CIDR</span>
        </div>
        <div class="text-lg font-semibold">{network.cidr}</div>
        <div class="text-sm text-muted">Gateway: {network.gateway_ip}</div>
      </div>
      
      <div class="card p-4">
        <div class="flex items-center gap-2 text-muted mb-2">
          <Shield size={16} />
          <span class="text-xs uppercase tracking-wider">Managed By</span>
        </div>
        <div class="text-lg font-semibold">{network.is_system_managed ? 'System' : 'User'}</div>
      </div>
    </div>
    
    <!-- Tabs -->
    <div class="border-b border-line mb-6">
      <div class="flex gap-1">
        <button 
          onclick={() => activeTab = 'overview'}
          class="px-4 py-2 text-sm font-medium {activeTab === 'overview' ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
        >
          Overview
        </button>
        <button 
          onclick={() => activeTab = 'vlans'}
          class="px-4 py-2 text-sm font-medium {activeTab === 'vlans' ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
        >
          VLANs ({vlans.length})
        </button>
        <button 
          onclick={() => activeTab = 'dhcp'}
          class="px-4 py-2 text-sm font-medium {activeTab === 'dhcp' ? 'border-b-2 border-accent text-accent' : 'text-muted hover:text-gray-700'}"
        >
          DHCP
        </button>
      </div>
    </div>
    
    <!-- Tab Content -->
    {#if activeTab === 'overview'}
      <div class="card p-6">
        <h3 class="text-lg font-semibold mb-4">Network Details</h3>
        <div class="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span class="text-muted">Name:</span>
            <span class="ml-2">{network.name}</span>
          </div>
          <div>
            <span class="text-muted">Mode:</span>
            <span class="ml-2">{network.mode}</span>
          </div>
          <div>
            <span class="text-muted">Bridge:</span>
            <span class="ml-2">{network.bridge_name}</span>
          </div>
          <div>
            <span class="text-muted">CIDR:</span>
            <span class="ml-2">{network.cidr}</span>
          </div>
          <div>
            <span class="text-muted">Gateway:</span>
            <span class="ml-2">{network.gateway_ip}</span>
          </div>
          <div>
            <span class="text-muted">Status:</span>
            <span class="ml-2">{network.status}</span>
          </div>
          <div>
            <span class="text-muted">Created:</span>
            <span class="ml-2">{new Date(network.created_at).toLocaleString()}</span>
          </div>
        </div>
      </div>
      
    {:else if activeTab === 'vlans'}
      <div class="space-y-4">
        <div class="flex justify-between items-center">
          <h3 class="text-lg font-semibold">VLAN Networks</h3>
          <button 
            onclick={() => showVLANForm = true}
            class="button-primary flex items-center gap-2"
          >
            <Plus size={16} />
            Add VLAN
          </button>
        </div>
        
        {#if showVLANForm}
          <div class="card p-4 bg-gray-50">
            <h4 class="font-medium mb-4">Create VLAN</h4>
            <div class="grid grid-cols-2 gap-4">
              <div>
                <label class="block text-sm text-muted mb-1">VLAN ID (1-4094)</label>
                <input 
                  type="number" 
                  bind:value={vlanForm.vlan_id} 
                  min="1" 
                  max="4094"
                  class="w-full px-3 py-2 border border-line rounded"
                />
              </div>
              <div>
                <label class="block text-sm text-muted mb-1">Name</label>
                <input 
                  type="text" 
                  bind:value={vlanForm.name} 
                  class="w-full px-3 py-2 border border-line rounded"
                  placeholder="e.g., production"
                />
              </div>
              <div>
                <label class="block text-sm text-muted mb-1">CIDR</label>
                <input 
                  type="text" 
                  bind:value={vlanForm.cidr} 
                  class="w-full px-3 py-2 border border-line rounded"
                  placeholder="e.g., 10.100.0.0/24"
                />
              </div>
              <div>
                <label class="block text-sm text-muted mb-1">Gateway IP</label>
                <input 
                  type="text" 
                  bind:value={vlanForm.gateway_ip} 
                  class="w-full px-3 py-2 border border-line rounded"
                  placeholder="e.g., 10.100.0.1"
                />
              </div>
            </div>
            <div class="flex gap-2 mt-4">
              <button onclick={createVLAN} class="button-primary">Create</button>
              <button onclick={() => showVLANForm = false} class="button-secondary">Cancel</button>
            </div>
          </div>
        {/if}
        
        {#if vlans.length === 0}
          <div class="card p-12 text-center text-muted border-dashed border-2">
            <Tag size={48} class="mx-auto mb-4 opacity-20" />
            <p>No VLANs configured for this network.</p>
            <p class="text-xs mt-1">VLANs allow network segmentation and isolation.</p>
          </div>
        {:else}
          <div class="card overflow-hidden">
            <table class="w-full text-left border-collapse">
              <thead>
                <tr class="bg-gray-50 text-xs uppercase tracking-wider text-muted border-b border-line">
                  <th class="px-4 py-3 font-semibold">VLAN ID</th>
                  <th class="px-4 py-3 font-semibold">Name</th>
                  <th class="px-4 py-3 font-semibold">CIDR</th>
                  <th class="px-4 py-3 font-semibold">Gateway</th>
                  <th class="px-4 py-3 font-semibold text-right">Actions</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-line text-sm bg-white">
                {#each vlans as vlan}
                  <tr class="hover:bg-gray-50 transition-colors">
                    <td class="px-4 py-3 font-medium text-gray-800">{vlan.vlan_id}</td>
                    <td class="px-4 py-3">{vlan.name}</td>
                    <td class="px-4 py-3 font-mono text-xs">{vlan.cidr}</td>
                    <td class="px-4 py-3 font-mono text-xs">{vlan.gateway_ip}</td>
                    <td class="px-4 py-3 text-right">
                      <button 
                        onclick={() => deleteVLAN(vlan.id)} 
                        class="p-2 hover:bg-rose-50 rounded text-rose-600 border border-transparent hover:border-rose-200 transition-all"
                        title="Delete VLAN"
                      >
                        <Trash2 size={16} />
                      </button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
      
    {:else if activeTab === 'dhcp'}
      <div class="space-y-4">
        <div class="flex justify-between items-center">
          <h3 class="text-lg font-semibold">DHCP Server</h3>
          {#if dhcpConfig?.configured}
            <div class="flex gap-2">
              {#if dhcpConfig.is_running}
                <button 
                  onclick={stopDHCPServer}
                  class="button-secondary flex items-center gap-2"
                >
                  <Square size={16} />
                  Stop Server
                </button>
              {:else}
                <button 
                  onclick={startDHCPServer}
                  class="button-primary flex items-center gap-2"
                >
                  <Play size={16} />
                  Start Server
                </button>
              {/if}
              <button 
                onclick={() => showDHCPForm = true}
                class="button-secondary"
              >
                Edit Config
              </button>
            </div>
          {:else}
            <button 
              onclick={() => showDHCPForm = true}
              class="button-primary flex items-center gap-2"
            >
              <Plus size={16} />
              Configure DHCP
            </button>
          {/if}
        </div>
        
        {#if showDHCPForm}
          <div class="card p-4 bg-gray-50">
            <h4 class="font-medium mb-4">{dhcpConfig?.configured ? 'Edit' : 'Configure'} DHCP Server</h4>
            <div class="grid grid-cols-3 gap-4">
              <div>
                <label class="block text-sm text-muted mb-1">Range Start</label>
                <input 
                  type="text" 
                  bind:value={dhcpForm.range_start} 
                  class="w-full px-3 py-2 border border-line rounded"
                  placeholder="e.g., 10.0.0.100"
                />
              </div>
              <div>
                <label class="block text-sm text-muted mb-1">Range End</label>
                <input 
                  type="text" 
                  bind:value={dhcpForm.range_end} 
                  class="w-full px-3 py-2 border border-line rounded"
                  placeholder="e.g., 10.0.0.200"
                />
              </div>
              <div>
                <label class="block text-sm text-muted mb-1">Lease Time (seconds)</label>
                <input 
                  type="number" 
                  bind:value={dhcpForm.lease_time_seconds} 
                  class="w-full px-3 py-2 border border-line rounded"
                  placeholder="3600"
                />
              </div>
            </div>
            <div class="flex gap-2 mt-4">
              <button onclick={configureDHCP} class="button-primary">Save</button>
              <button onclick={() => showDHCPForm = false} class="button-secondary">Cancel</button>
            </div>
          </div>
        {/if}
        
        {#if dhcpConfig?.configured}
          <div class="card p-4">
            <h4 class="font-medium mb-3">Configuration</h4>
            <div class="grid grid-cols-4 gap-4 text-sm">
              <div>
                <span class="text-muted">Range:</span>
                <span class="ml-2 font-mono">{dhcpConfig.range_start} - {dhcpConfig.range_end}</span>
              </div>
              <div>
                <span class="text-muted">Lease Time:</span>
                <span class="ml-2">{dhcpConfig.lease_time_seconds}s</span>
              </div>
              <div>
                <span class="text-muted">Status:</span>
                <span class="ml-2">
                  {#if dhcpConfig.is_running}
                    <span class="text-emerald-600 font-medium">Running</span>
                  {:else}
                    <span class="text-gray-500">Stopped</span>
                  {/if}
                </span>
              </div>
            </div>
          </div>
          
          <div class="card overflow-hidden">
            <div class="px-4 py-3 border-b border-line bg-gray-50">
              <h4 class="font-medium">Active Leases</h4>
            </div>
            {#if dhcpLeases.length === 0}
              <div class="p-8 text-center text-muted">
                <p>No active DHCP leases.</p>
              </div>
            {:else}
              <table class="w-full text-left border-collapse">
                <thead>
                  <tr class="bg-gray-50 text-xs uppercase tracking-wider text-muted border-b border-line">
                    <th class="px-4 py-3 font-semibold">MAC Address</th>
                    <th class="px-4 py-3 font-semibold">IP Address</th>
                    <th class="px-4 py-3 font-semibold">Hostname</th>
                    <th class="px-4 py-3 font-semibold">Lease Start</th>
                    <th class="px-4 py-3 font-semibold">Lease End</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-line text-sm bg-white">
                  {#each dhcpLeases as lease}
                    <tr class="hover:bg-gray-50 transition-colors">
                      <td class="px-4 py-3 font-mono text-xs">{lease.mac_address}</td>
                      <td class="px-4 py-3 font-mono text-xs">{lease.ip_address}</td>
                      <td class="px-4 py-3">{lease.hostname || '-'}</td>
                      <td class="px-4 py-3 text-muted">{new Date(lease.lease_start).toLocaleString()}</td>
                      <td class="px-4 py-3 text-muted">{new Date(lease.lease_end).toLocaleString()}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/if}
          </div>
        {:else}
          <div class="card p-12 text-center text-muted border-dashed border-2">
            <Settings size={48} class="mx-auto mb-4 opacity-20" />
            <p>DHCP server not configured.</p>
            <p class="text-xs mt-1">Configure DHCP to automatically assign IP addresses to VMs.</p>
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
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
</style>
