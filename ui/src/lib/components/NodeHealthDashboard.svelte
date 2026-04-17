<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Activity, Server, Cpu, HardDrive, AlertCircle, CheckCircle, Clock } from 'lucide-svelte';
  import { createAPIClient } from '$lib/api/client';
  import type { Node } from '$lib/api/types';
  import Sparkline from './charts/Sparkline.svelte';

  interface NodeMetrics {
    cpu_percent: number;
    memory_used_mb: number;
    memory_total_mb: number;
    disk_used_gb: number;
    disk_total_gb: number;
    timestamp: string;
  }

  interface NodeWithMetrics extends Node {
    metrics?: NodeMetrics;
    vmCount?: number;
    cpuHistory?: number[];
  }

  const client = createAPIClient();

  let nodes = $state<NodeWithMetrics[]>([]);
  let loading = $state(true);
  let pollInterval: number | null = null;
  let selectedTimeRange = $state<'1h' | '6h' | '24h'>('1h');

  async function loadNodes() {
    try {
      const nodeList = await client.listNodes();
      
      // Load additional metrics for each node
      const nodesWithMetrics = await Promise.all(
        nodeList.map(async (node: Node) => {
          try {
            const [metrics, vms] = await Promise.all([
              client.getNodeMetrics(node.id).catch(() => null),
              client.listNodeVMs(node.id).catch(() => ({ resources: [], count: 0 }))
            ]);

            // Generate some sample history for sparklines (in real implementation, this would come from API)
            const cpuHistory = generateHistory(metrics?.cpu_percent || 0);

            return {
              ...node,
              metrics: metrics || undefined,
              vmCount: vms.count || 0,
              cpuHistory
            };
          } catch (e) {
            return {
              ...node,
              vmCount: 0,
              cpuHistory: generateHistory(0)
            };
          }
        })
      );

      nodes = nodesWithMetrics;
    } catch (e) {
      console.error('Failed to load nodes:', e);
    } finally {
      loading = false;
    }
  }

  function generateHistory(currentValue: number): number[] {
    const history: number[] = [];
    for (let i = 20; i >= 0; i--) {
      const variation = (Math.random() - 0.5) * 10;
      history.push(Math.max(0, Math.min(100, currentValue + variation)));
    }
    return history;
  }

  function startPolling() {
    loadNodes();
    pollInterval = window.setInterval(loadNodes, 30000);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  onMount(() => {
    startPolling();
  });

  onDestroy(() => {
    stopPolling();
  });

  function getStatusColor(status: string): string {
    switch (status) {
      case 'online': return 'text-emerald-500';
      case 'offline': return 'text-red-500';
      case 'warning': return 'text-amber-500';
      case 'maintenance': return 'text-orange-500';
      default: return 'text-slate-400';
    }
  }

  function getStatusBg(status: string): string {
    switch (status) {
      case 'online': return 'bg-emerald-500';
      case 'offline': return 'bg-red-500';
      case 'warning': return 'bg-amber-500';
      case 'maintenance': return 'bg-orange-500';
      default: return 'bg-slate-400';
    }
  }

  function getStatusBgLight(status: string): string {
    switch (status) {
      case 'online': return 'bg-emerald-50 border-emerald-200';
      case 'offline': return 'bg-red-50 border-red-200';
      case 'warning': return 'bg-amber-50 border-amber-200';
      case 'maintenance': return 'bg-orange-50 border-orange-200';
      default: return 'bg-slate-50 border-slate-200';
    }
  }

  let onlineNodes = $derived(nodes.filter(n => n.status === 'online').length);
  let offlineNodes = $derived(nodes.filter(n => n.status === 'offline').length);
  let warningNodes = $derived(nodes.filter(n => (n.status as string) === 'warning').length);
  let totalVMs = $derived(nodes.reduce((sum, n) => sum + (n.vmCount || 0), 0));

  function formatBytes(mb: number): string {
    if (mb >= 1024) {
      return `${(mb / 1024).toFixed(1)} GB`;
    }
    return `${Math.round(mb)} MB`;
  }

  function formatGB(gb: number): string {
    if (gb >= 1000) {
      return `${(gb / 1000).toFixed(1)} TB`;
    }
    return `${Math.round(gb)} GB`;
  }
</script>

<div class="space-y-6">
  <!-- Summary Cards -->
  <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
    <div class="bg-white rounded-lg border border-slate-200 p-4">
      <div class="flex items-center gap-2 mb-2">
        <Server size={18} class="text-blue-500" />
        <span class="text-sm text-slate-500">Total Nodes</span>
      </div>
      <div class="text-2xl font-semibold text-slate-900">{nodes.length}</div>
      <div class="text-xs text-slate-500 mt-1">
        {onlineNodes} online
      </div>
    </div>

    <div class="bg-white rounded-lg border border-slate-200 p-4">
      <div class="flex items-center gap-2 mb-2">
        <CheckCircle size={18} class="text-emerald-500" />
        <span class="text-sm text-slate-500">Healthy</span>
      </div>
      <div class="text-2xl font-semibold text-emerald-600">{onlineNodes}</div>
      <div class="text-xs text-emerald-600 mt-1">
        All systems operational
      </div>
    </div>

    <div class="bg-white rounded-lg border border-slate-200 p-4">
      <div class="flex items-center gap-2 mb-2">
        <AlertCircle size={18} class="text-amber-500" />
        <span class="text-sm text-slate-500">Warnings</span>
      </div>
      <div class="text-2xl font-semibold {warningNodes > 0 ? 'text-amber-600' : 'text-slate-400'}">
        {warningNodes}
      </div>
      <div class="text-xs {warningNodes > 0 ? 'text-amber-600' : 'text-slate-500'} mt-1">
        {warningNodes > 0 ? 'Attention needed' : 'No warnings'}
      </div>
    </div>

    <div class="bg-white rounded-lg border border-slate-200 p-4">
      <div class="flex items-center gap-2 mb-2">
        <Activity size={18} class="text-purple-500" />
        <span class="text-sm text-slate-500">Total VMs</span>
      </div>
      <div class="text-2xl font-semibold text-slate-900">{totalVMs}</div>
      <div class="text-xs text-slate-500 mt-1">
        Across all nodes
      </div>
    </div>
  </div>

  <!-- Node Cards -->
  {#if loading}
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      {#each Array(4) as _}
        <div class="bg-white rounded-lg border border-slate-200 p-6 animate-pulse">
          <div class="h-4 bg-slate-200 rounded w-32 mb-4"></div>
          <div class="h-24 bg-slate-100 rounded"></div>
        </div>
      {/each}
    </div>
  {:else}
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      {#each nodes as node}
        <div class="bg-white rounded-lg border border-slate-200 p-6">
          <!-- Header -->
          <div class="flex items-center justify-between mb-4">
            <div class="flex items-center gap-3">
              <div class="relative">
                <div class="w-10 h-10 rounded-lg bg-slate-100 flex items-center justify-center">
                  <Server size={20} class="text-slate-600" />
                </div>
                <div class="absolute -bottom-1 -right-1 w-4 h-4 rounded-full border-2 border-white {getStatusBg(node.status)}"></div>
              </div>
              <div>
                <h3 class="font-semibold text-slate-900">{node.name}</h3>
                <p class="text-sm text-slate-500">{node.hostname} • {node.ip_address}</p>
              </div>
            </div>
            <span class="px-3 py-1 rounded-full text-xs font-medium {getStatusBgLight(node.status)} {getStatusColor(node.status)}">
              {node.status}
            </span>
          </div>

          <!-- Metrics -->
          {#if node.metrics}
            <div class="grid grid-cols-3 gap-4 mb-4">
              <div>
                <div class="flex items-center gap-1 text-xs text-slate-500 mb-1">
                  <Cpu size={12} />
                  CPU
                </div>
                <div class="text-lg font-semibold text-slate-900">
                  {node.metrics.cpu_percent.toFixed(1)}%
                </div>
                {#if node.cpuHistory && node.cpuHistory.length > 1}
                  <div class="mt-1">
                    <Sparkline data={node.cpuHistory} color="#3b82f6" width={80} height={24} />
                  </div>
                {/if}
              </div>

              <div>
                <div class="flex items-center gap-1 text-xs text-slate-500 mb-1">
                  <Activity size={12} />
                  Memory
                </div>
                <div class="text-lg font-semibold text-slate-900">
                  {formatBytes(node.metrics.memory_used_mb)}
                </div>
                <div class="text-xs text-slate-500">
                  of {formatBytes(node.metrics.memory_total_mb)}
                </div>
              </div>

              <div>
                <div class="flex items-center gap-1 text-xs text-slate-500 mb-1">
                  <HardDrive size={12} />
                  Storage
                </div>
                <div class="text-lg font-semibold text-slate-900">
                  {formatGB(node.metrics.disk_used_gb)}
                </div>
                <div class="text-xs text-slate-500">
                  of {formatGB(node.metrics.disk_total_gb)}
                </div>
              </div>
            </div>
          {:else}
            <div class="text-center py-4 text-slate-500 text-sm">
              No metrics available
            </div>
          {/if}

          <!-- Footer -->
          <div class="flex items-center justify-between pt-4 border-t border-slate-100">
            <div class="flex items-center gap-4 text-sm">
              <span class="text-slate-600">
                <span class="font-medium">{node.vmCount || 0}</span> VMs
              </span>
              {#if node.last_seen_at}
                <span class="text-slate-400 flex items-center gap-1">
                  <Clock size={12} />
                  Seen {new Date(node.last_seen_at).toLocaleTimeString()}
                </span>
              {/if}
            </div>
            {#if node.is_local}
              <span class="text-xs px-2 py-1 bg-blue-50 text-blue-600 rounded font-medium">
                Local
              </span>
            {/if}
          </div>
        </div>
      {:else}
        <div class="col-span-2 text-center py-12 bg-white rounded-lg border border-slate-200">
          <Server size={48} class="mx-auto text-slate-300 mb-4" />
          <h3 class="text-lg font-medium text-slate-900 mb-1">No nodes found</h3>
          <p class="text-sm text-slate-500">Add a node to start monitoring</p>
        </div>
      {/each}
    </div>
  {/if}
</div>
