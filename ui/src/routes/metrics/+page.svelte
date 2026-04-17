<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Activity, Server, Cpu, HardDrive, Globe, Clock, BarChart3, LineChart, PieChart } from 'lucide-svelte';
  import { createAPIClient } from '$lib/api/client';
  import StatsCard from '$lib/components/shared/StatsCard.svelte';
  import VMMetricsWidget from '$lib/components/vms/VMMetricsWidget.svelte';
  import NodeHealthDashboard from '$lib/components/NodeHealthDashboard.svelte';
  import TopResourceConsumers from '$lib/components/TopResourceConsumers.svelte';
  import ChartJS from '$lib/components/charts/ChartJS.svelte';
  import type { ChartData } from 'chart.js';
  import type { VM, Node } from '$lib/api/types';
  
  const client = createAPIClient();
  
  let vmStats = $state({ total: 0, running: 0, stopped: 0, error: 0 });
  let nodeHealth = $state<Node[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let activeTab = $state<'overview' | 'nodes' | 'vms'>('overview');
  let pollInterval: number | null = null;
  
  // Historical data for charts
  let vmHistoryData = $state<{ time: string; running: number; stopped: number }[]>([]);

  onMount(async () => {
    try {
      await loadData();
      startPolling();
    } catch (err: any) {
      console.error('Failed to load metrics:', err);
      error = err.message || 'Failed to load metrics';
    }
  });

  onDestroy(() => {
    stopPolling();
  });

  async function loadData() {
    loading = true;
    try {
      // Fetch VMs
      const vms: VM[] = (await client.listVMs()) ?? [];
      vmStats.total = vms.length;
      vmStats.running = vms.filter((v: VM) => v.actual_state === 'running').length;
      vmStats.stopped = vms.filter((v: VM) => v.actual_state === 'stopped').length;
      vmStats.error = vms.filter((v: VM) => v.actual_state === 'error').length;
      
      // Fetch nodes
      const nodes: Node[] = (await client.listNodes()) ?? [];
      nodeHealth = nodes;

      // Generate some historical data for the VM activity chart
      updateVMHistory();
      
      error = null;
    } catch (err: any) {
      console.error('Failed to load metrics:', err);
      error = err.message || 'Failed to load metrics';
    } finally {
      loading = false;
    }
  }

  function updateVMHistory() {
    const now = new Date();
    const newPoint = {
      time: now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      running: vmStats.running,
      stopped: vmStats.stopped
    };
    
    vmHistoryData = [...vmHistoryData.slice(-20), newPoint];
  }

  function startPolling() {
    pollInterval = window.setInterval(() => {
      loadData();
    }, 30000);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }
  
  function getStatusColor(status: string) {
    switch (status) {
      case 'online': return 'text-green-500';
      case 'offline': return 'text-red-500';
      case 'warning': return 'text-yellow-500';
      case 'maintenance': return 'text-orange-500';
      default: return 'text-slate-400';
    }
  }

  // Chart data
  let vmActivityChartData = $derived<ChartData>({
    labels: vmHistoryData.map(d => d.time),
    datasets: [
      {
        label: 'Running VMs',
        data: vmHistoryData.map(d => d.running),
        borderColor: '#10b981',
        backgroundColor: 'rgba(16, 185, 129, 0.1)',
        fill: true,
        tension: 0.4,
        pointRadius: 2,
        pointHoverRadius: 4
      },
      {
        label: 'Stopped VMs',
        data: vmHistoryData.map(d => d.stopped),
        borderColor: '#64748b',
        backgroundColor: 'rgba(100, 116, 139, 0.1)',
        fill: true,
        tension: 0.4,
        pointRadius: 2,
        pointHoverRadius: 4
      }
    ]
  });

  let vmDistributionData = $derived<ChartData>({
    labels: ['Running', 'Stopped', 'Error'],
    datasets: [{
      data: [vmStats.running, vmStats.stopped, vmStats.error],
      backgroundColor: [
        '#10b981',
        '#64748b',
        '#ef4444'
      ],
      borderWidth: 0
    }]
  });
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-semibold text-slate-900">Metrics Dashboard</h1>
      <p class="text-sm text-slate-500 mt-1">Real-time monitoring and performance metrics</p>
    </div>
    <div class="flex items-center gap-4">
      <div class="flex items-center gap-2 text-sm text-slate-500 bg-white px-3 py-1.5 rounded-lg border border-slate-200">
        <Clock size={16} />
        <span>Updated: {new Date().toLocaleTimeString()}</span>
      </div>
    </div>
  </div>

  <!-- Tabs -->
  <div class="border-b border-slate-200">
    <div class="flex gap-1">
      <button 
        onclick={() => activeTab = 'overview'}
        class="px-4 py-2 text-sm font-medium {activeTab === 'overview' ? 'border-b-2 border-blue-500 text-blue-600' : 'text-slate-600 hover:text-slate-800'}"
      >
        <BarChart3 size={16} class="inline mr-1" />
        Overview
      </button>
      <button 
        onclick={() => activeTab = 'nodes'}
        class="px-4 py-2 text-sm font-medium {activeTab === 'nodes' ? 'border-b-2 border-blue-500 text-blue-600' : 'text-slate-600 hover:text-slate-800'}"
      >
        <Server size={16} class="inline mr-1" />
        Node Health
      </button>
      <button 
        onclick={() => activeTab = 'vms'}
        class="px-4 py-2 text-sm font-medium {activeTab === 'vms' ? 'border-b-2 border-blue-500 text-blue-600' : 'text-slate-600 hover:text-slate-800'}"
      >
        <Cpu size={16} class="inline mr-1" />
        VM Resources
      </button>
    </div>
  </div>

  {#if loading && vmStats.total === 0}
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      {#each Array(4) as _}
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
    {#if activeTab === 'overview'}
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

      <!-- Charts Section -->
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <!-- VM Activity Chart -->
        <div class="lg:col-span-2 bg-white rounded-lg border border-slate-200 p-6">
          <div class="flex items-center justify-between mb-4">
            <div class="flex items-center gap-2">
              <LineChart size={20} class="text-blue-500" />
              <h2 class="text-lg font-semibold text-slate-900">VM Activity</h2>
            </div>
            <span class="text-sm text-slate-500">Last 20 data points</span>
          </div>
          <ChartJS type="line" data={vmActivityChartData} height={280} />
        </div>

        <!-- VM Distribution -->
        <div class="bg-white rounded-lg border border-slate-200 p-6">
          <div class="flex items-center justify-between mb-4">
            <div class="flex items-center gap-2">
              <PieChart size={20} class="text-purple-500" />
              <h2 class="text-lg font-semibold text-slate-900">VM Distribution</h2>
            </div>
          </div>
          <ChartJS 
            type="bar" 
            data={vmDistributionData} 
            height={280}
            options={{
              plugins: {
                legend: { display: false }
              },
              scales: {
                y: {
                  beginAtZero: true,
                  ticks: {
                    stepSize: 1
                  }
                }
              }
            }}
          />
        </div>
      </div>

      <!-- Resource Overview -->
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <VMMetricsWidget vms={vmStats} />
        <TopResourceConsumers />
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
              <pre class="text-xs text-slate-400 font-mono whitespace-pre-wrap">curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8888/api/v1/metrics</pre>
            </div>
          </div>
        </div>
      </div>
    {:else if activeTab === 'nodes'}
      <NodeHealthDashboard />
    {:else if activeTab === 'vms'}
      <TopResourceConsumers />
    {/if}
  {/if}
</div>
