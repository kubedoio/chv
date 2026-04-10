<script>
  import { onMount } from 'svelte';
  import { Activity, Server, Cpu, HardDrive, Globe, Clock } from 'lucide-svelte';
  import { createAPIClient } from '$lib/api/client';
  import StatsCard from '$lib/components/StatsCard.svelte';
  import VMMetricsWidget from '$lib/components/VMMetricsWidget.svelte';
  
  const client = createAPIClient();
  
  let vmStats = $state({ total: 0, running: 0, stopped: 0, error: 0 });
  let nodeHealth = $state([]);
  let loading = $state(true);
  let error = $state(null);
  
  onMount(async () => {
    try {
      loading = true;
      
      // Fetch VMs
      const vms = await client.listVMs();
      vmStats.total = vms.length;
      vmStats.running = vms.filter(v => v.actual_state === 'running').length;
      vmStats.stopped = vms.filter(v => v.actual_state === 'stopped').length;
      vmStats.error = vms.filter(v => v.actual_state === 'error').length;
      
      // Fetch nodes
      const nodes = await client.listNodes();
      nodeHealth = nodes;
    } catch (err) {
      console.error('Failed to load metrics:', err);
      error = err.message || 'Failed to load metrics';
    } finally {
      loading = false;
    }
  });
  
  function getStatusColor(status) {
    switch (status) {
      case 'online': return 'text-green-500';
      case 'offline': return 'text-red-500';
      case 'warning': return 'text-yellow-500';
      case 'maintenance': return 'text-orange-500';
      default: return 'text-slate-400';
    }
  }
  
  function getStatusBg(status) {
    switch (status) {
      case 'online': return 'bg-green-500/10';
      case 'offline': return 'bg-red-500/10';
      case 'warning': return 'bg-yellow-500/10';
      case 'maintenance': return 'bg-orange-500/10';
      default: return 'bg-slate-500/10';
    }
  }
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-semibold text-slate-900">Metrics Dashboard</h1>
      <p class="text-sm text-slate-500 mt-1">Real-time monitoring and performance metrics</p>
    </div>
    <div class="flex items-center gap-2 text-sm text-slate-500">
      <Clock size={16} />
      <span>Updated: {new Date().toLocaleTimeString()}</span>
    </div>
  </div>

  {#if loading}
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      {#each Array(4) as _, i}
        <div class="bg-white rounded-lg border border-slate-200 p-6 animate-pulse">
          <div class="h-4 bg-slate-200 rounded w-24 mb-4"></div>
          <div class="h-8 bg-slate-200 rounded w-16"></div>
        </div>
      {/each}
    </div>
  {:else if error}
    <div class="bg-red-50 border border-red-200 rounded-lg p-4 text-red-700">
      {error}
    </div>
  {:else}
    <!-- Stats Grid -->
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      <StatsCard
        title="Total VMs"
        value={vmStats.total}
        icon={Server}
        trend={vmStats.running > 0 ? `${vmStats.running} running` : 'None running'}
        trendUp={vmStats.running > 0}
      />
      
      <StatsCard
        title="Running VMs"
        value={vmStats.running}
        icon={Activity}
        trend={vmStats.total > 0 ? `${Math.round((vmStats.running / vmStats.total) * 100)}% of total` : 'No VMs'}
        trendUp={vmStats.running > 0}
      />
      
      <StatsCard
        title="Stopped VMs"
        value={vmStats.stopped}
        icon={Cpu}
        trend={vmStats.stopped > 0 ? 'Ready to start' : 'All running'}
        trendUp={false}
      />
      
      <StatsCard
        title="Nodes"
        value={nodeHealth.length}
        icon={Globe}
        trend={`${nodeHealth.filter(n => n.status === 'online').length} online`}
        trendUp={nodeHealth.every(n => n.status === 'online')}
      />
    </div>

    <!-- Node Health Section -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <div class="bg-white rounded-lg border border-slate-200 p-6">
        <h2 class="text-lg font-semibold text-slate-900 mb-4 flex items-center gap-2">
          <Globe size={20} class="text-slate-500" />
          Node Health
        </h2>
        
        <div class="space-y-3">
          {#each nodeHealth as node}
            <div class="flex items-center justify-between p-3 rounded-lg border border-slate-100 hover:border-slate-200 transition-colors">
              <div class="flex items-center gap-3">
                <div class="w-2 h-2 rounded-full {getStatusColor(node.status)}" class:animate-pulse={node.status === 'online'}></div>
                <div>
                  <div class="font-medium text-slate-900">{node.name}</div>
                  <div class="text-xs text-slate-500">{node.hostname}</div>
                </div>
              </div>
              <div class="flex items-center gap-4">
                <div class="text-right">
                  <div class="text-xs text-slate-500">Resources</div>
                  <div class="text-sm font-medium text-slate-900">
                    {node.resources?.vms || 0} VMs
                  </div>
                </div>
                <span class="px-2 py-1 rounded text-xs font-medium {getStatusBg(node.status)} {getStatusColor(node.status)}">
                  {node.status}
                </span>
              </div>
            </div>
          {:else}
            <div class="text-center py-8 text-slate-500">
              No nodes found
            </div>
          {/each}
        </div>
      </div>

      <VMMetricsWidget vms={vmStats} />
    </div>

    <!-- Prometheus Info Section -->
    <div class="bg-slate-900 rounded-lg p-6">
      <h2 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
        <Activity size={20} class="text-orange-400" />
        Prometheus Metrics
      </h2>
      
      <div class="space-y-4">
        <p class="text-slate-400 text-sm">
          CHV exposes Prometheus-compatible metrics for external monitoring systems.
          Configure your Prometheus server to scrape the endpoint below.
        </p>
        
        <div class="bg-slate-800 rounded-lg p-4 font-mono text-sm">
          <div class="flex items-center justify-between mb-2">
            <span class="text-slate-400">Endpoint:</span>
            <code class="text-orange-400">GET /api/v1/metrics</code>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-slate-400">Example URL:</span>
            <code class="text-green-400">http://localhost:8888/api/v1/metrics</code>
          </div>
        </div>
        
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4">
          <div class="bg-slate-800/50 rounded p-4">
            <h3 class="text-sm font-medium text-white mb-2">Available Metrics</h3>
            <ul class="space-y-1 text-xs text-slate-400 font-mono">
              <li>chv_vms_total</li>
              <li>chv_vm_cpu_usage_percent</li>
              <li>chv_vm_memory_usage_bytes</li>
              <li>chv_node_health</li>
              <li>chv_api_requests_total</li>
              <li>chv_api_request_duration_seconds</li>
            </ul>
          </div>
          
          <div class="bg-slate-800/50 rounded p-4">
            <h3 class="text-sm font-medium text-white mb-2">Example Query</h3>
            <pre class="text-xs text-slate-400 font-mono whitespace-pre-wrap">
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8888/api/v1/metrics</pre>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>
