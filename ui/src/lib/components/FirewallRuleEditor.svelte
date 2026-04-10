<script lang="ts">
  import { onMount } from 'svelte';
  import { Plus, Trash2, Shield, ArrowUp, ArrowDown } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import type { FirewallRule } from '$lib/api/types';
  
  interface Props {
    vmId: string;
  }
  
  let { vmId }: Props = $props();
  
  const client = createAPIClient({ token: getStoredToken() ?? undefined });
  
  let rules = $state<FirewallRule[]>([]);
  let loading = $state(false);
  let showForm = $state(false);
  
  let form = $state({
    direction: 'ingress' as 'ingress' | 'egress',
    protocol: 'tcp' as 'tcp' | 'udp' | 'icmp' | 'all',
    port_range: '',
    source_cidr: '0.0.0.0/0',
    action: 'allow' as 'allow' | 'deny',
    priority: 100,
    description: ''
  });
  
  onMount(() => {
    loadRules();
  });
  
  async function loadRules() {
    loading = true;
    try {
      rules = await client.listFirewallRules(vmId);
    } catch (e) {
      toast.error('Failed to load firewall rules');
    } finally {
      loading = false;
    }
  }
  
  async function createRule() {
    try {
      await client.createFirewallRule(vmId, form);
      toast.success('Firewall rule created');
      showForm = false;
      resetForm();
      loadRules();
    } catch (e: any) {
      toast.error(e.message || 'Failed to create rule');
    }
  }
  
  async function deleteRule(ruleId: string) {
    if (!confirm('Are you sure you want to delete this rule?')) return;
    try {
      await client.deleteFirewallRule(vmId, ruleId);
      toast.success('Rule deleted');
      loadRules();
    } catch (e: any) {
      toast.error(e.message || 'Failed to delete rule');
    }
  }
  
  function resetForm() {
    form = {
      direction: 'ingress',
      protocol: 'tcp',
      port_range: '',
      source_cidr: '0.0.0.0/0',
      action: 'allow',
      priority: 100,
      description: ''
    };
  }
  
  function getProtocolIcon(protocol: string) {
    switch (protocol) {
      case 'tcp': return 'TCP';
      case 'udp': return 'UDP';
      case 'icmp': return 'ICMP';
      default: return 'ALL';
    }
  }
  
  function getActionClass(action: string) {
    return action === 'allow' 
      ? 'bg-emerald-50 text-emerald-700 border-emerald-200' 
      : 'bg-rose-50 text-rose-700 border-rose-200';
  }
  
  function getDirectionIcon(direction: string) {
    return direction === 'ingress' ? ArrowDown : ArrowUp;
  }
</script>

<div class="space-y-4">
  <div class="flex justify-between items-center">
    <div class="flex items-center gap-2">
      <Shield size={20} class="text-muted" />
      <h3 class="text-lg font-semibold">Firewall Rules</h3>
      <span class="text-sm text-muted">({rules.length})</span>
    </div>
    <button 
      onclick={() => showForm = true}
      class="button-primary flex items-center gap-2"
    >
      <Plus size={16} />
      Add Rule
    </button>
  </div>
  
  {#if showForm}
    <div class="card p-4 bg-gray-50">
      <h4 class="font-medium mb-4">Create Firewall Rule</h4>
      <div class="grid grid-cols-3 gap-4">
        <div>
          <label class="block text-sm text-muted mb-1">Direction</label>
          <select 
            bind:value={form.direction}
            class="w-full px-3 py-2 border border-line rounded"
          >
            <option value="ingress">Ingress (Incoming)</option>
            <option value="egress">Egress (Outgoing)</option>
          </select>
        </div>
        <div>
          <label class="block text-sm text-muted mb-1">Protocol</label>
          <select 
            bind:value={form.protocol}
            class="w-full px-3 py-2 border border-line rounded"
          >
            <option value="tcp">TCP</option>
            <option value="udp">UDP</option>
            <option value="icmp">ICMP</option>
            <option value="all">All</option>
          </select>
        </div>
        <div>
          <label class="block text-sm text-muted mb-1">Action</label>
          <select 
            bind:value={form.action}
            class="w-full px-3 py-2 border border-line rounded"
          >
            <option value="allow">Allow</option>
            <option value="deny">Deny</option>
          </select>
        </div>
        {#if form.protocol === 'tcp' || form.protocol === 'udp'}
          <div>
            <label class="block text-sm text-muted mb-1">Port Range</label>
            <input 
              type="text" 
              bind:value={form.port_range} 
              class="w-full px-3 py-2 border border-line rounded"
              placeholder="e.g., 80, 22-80, 443,8443"
            />
            <p class="text-xs text-muted mt-1">Single port, range (22-80), or list (80,443)</p>
          </div>
        {/if}
        <div>
          <label class="block text-sm text-muted mb-1">Source CIDR</label>
          <input 
            type="text" 
            bind:value={form.source_cidr} 
            class="w-full px-3 py-2 border border-line rounded"
            placeholder="0.0.0.0/0"
          />
        </div>
        <div>
          <label class="block text-sm text-muted mb-1">Priority (100-999)</label>
          <input 
            type="number" 
            bind:value={form.priority} 
            min="100" 
            max="999"
            class="w-full px-3 py-2 border border-line rounded"
          />
          <p class="text-xs text-muted mt-1">Lower numbers evaluated first</p>
        </div>
        <div class="col-span-3">
          <label class="block text-sm text-muted mb-1">Description (optional)</label>
          <input 
            type="text" 
            bind:value={form.description} 
            class="w-full px-3 py-2 border border-line rounded"
            placeholder="e.g., Allow SSH from internal network"
          />
        </div>
      </div>
      <div class="flex gap-2 mt-4">
        <button onclick={createRule} class="button-primary">Create Rule</button>
        <button onclick={() => showForm = false} class="button-secondary">Cancel</button>
      </div>
    </div>
  {/if}
  
  {#if rules.length === 0 && !showForm}
    <div class="card p-12 text-center text-muted border-dashed border-2">
      <Shield size={48} class="mx-auto mb-4 opacity-20" />
      <p>No firewall rules configured.</p>
      <p class="text-xs mt-1">By default, all traffic is allowed. Add rules to restrict access.</p>
    </div>
  {:else if rules.length > 0}
    <div class="card overflow-hidden">
      <table class="w-full text-left border-collapse">
        <thead>
          <tr class="bg-gray-50 text-xs uppercase tracking-wider text-muted border-b border-line">
            <th class="px-4 py-3 font-semibold w-16">Priority</th>
            <th class="px-4 py-3 font-semibold">Direction</th>
            <th class="px-4 py-3 font-semibold">Protocol</th>
            <th class="px-4 py-3 font-semibold">Port/Type</th>
            <th class="px-4 py-3 font-semibold">Source</th>
            <th class="px-4 py-3 font-semibold">Action</th>
            <th class="px-4 py-3 font-semibold">Description</th>
            <th class="px-4 py-3 font-semibold text-right">Actions</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-line text-sm bg-white">
          {#each rules as rule}
            <tr class="hover:bg-gray-50 transition-colors">
              <td class="px-4 py-3 font-mono text-xs">{rule.priority}</td>
              <td class="px-4 py-3">
                <span class="flex items-center gap-1">
                  {#if rule.direction === 'ingress'}
                    <ArrowDown size={14} />
                    <span>In</span>
                  {:else}
                    <ArrowUp size={14} />
                    <span>Out</span>
                  {/if}
                </span>
              </td>
              <td class="px-4 py-3">
                <span class="px-2 py-0.5 rounded text-xs font-semibold bg-gray-100 border border-gray-200">
                  {getProtocolIcon(rule.protocol)}
                </span>
              </td>
              <td class="px-4 py-3 font-mono text-xs">
                {rule.port_range || (rule.protocol === 'icmp' ? 'Any' : 'All')}
              </td>
              <td class="px-4 py-3 font-mono text-xs">{rule.source_cidr}</td>
              <td class="px-4 py-3">
                <span class="px-2 py-0.5 rounded-full text-xs font-semibold border {getActionClass(rule.action)}">
                  {rule.action}
                </span>
              </td>
              <td class="px-4 py-3 text-muted">{rule.description || '-'}</td>
              <td class="px-4 py-3 text-right">
                <button 
                  onclick={() => deleteRule(rule.id)} 
                  class="p-2 hover:bg-rose-50 rounded text-rose-600 border border-transparent hover:border-rose-200 transition-all"
                  title="Delete Rule"
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
