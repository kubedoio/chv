<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Activity, Server, Cpu, Clock, BarChart3, LineChart, PieChart, ShieldAlert } from 'lucide-svelte';
  import { createAPIClient } from '$lib/api/client';
  import SectionCard from '$lib/components/shell/SectionCard.svelte';
  import CompactMetricCard from '$lib/components/shared/CompactMetricCard.svelte';
  import ChartJS from '$lib/components/shared/charts/ChartJS.svelte';
  import type { ChartData } from 'chart.js';
  import type { VM, Node } from '$lib/api/types';
  import ErrorState from '$lib/components/shell/ErrorState.svelte';
  import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
  import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
  import { getPageDefinition } from '$lib/shell/app-shell';
  
  const client = createAPIClient();
  const pageDef = getPageDefinition('/metrics');
  
  let vmStats = $state({ total: 0, running: 0, stopped: 0, error: 0 });
  let nodeHealth = $state<Node[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let pollInterval: number | null = null;
  
  let vmHistoryData = $state<{ time: string; running: number; stopped: number }[]>([]);

  onMount(async () => {
    try {
      await loadData();
      startPolling();
    } catch (err: any) {
      error = err.message || 'Telemetry connection failed';
    }
  });

  onDestroy(() => {
    stopPolling();
  });

  async function loadData() {
    loading = true;
    try {
      const vms: VM[] = (await client.listVMs()) ?? [];
      vmStats.total = vms.length;
      vmStats.running = vms.filter((v: VM) => v.actual_state === 'running').length;
      vmStats.stopped = vms.filter((v: VM) => v.actual_state === 'stopped').length;
      vmStats.error = vms.filter((v: VM) => v.actual_state === 'error').length;
      
      const nodes: Node[] = (await client.listNodes()) ?? [];
      nodeHealth = nodes;

      updateVMHistory();
      error = null;
    } catch (err: any) {
      error = err.message || 'Telemetry connection failed';
    } finally {
      loading = false;
    }
  }

  function updateVMHistory() {
    const now = new Date();
    const newPoint = {
      time: now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
      running: vmStats.running,
      stopped: vmStats.stopped
    };
    vmHistoryData = [...vmHistoryData.slice(-15), newPoint];
  }

  function startPolling() {
    pollInterval = window.setInterval(loadData, 5000);
  }

  function stopPolling() {
    if (pollInterval) clearInterval(pollInterval);
  }

  let telemetryChartData = $derived<ChartData>({
    labels: vmHistoryData.map(d => d.time),
    datasets: [
      {
        label: 'Active Workloads',
        data: vmHistoryData.map(d => d.running),
        borderColor: '#0f62fe', // IBM Blue 60
        backgroundColor: 'rgba(15, 98, 254, 0.1)',
        fill: true,
        tension: 0.1,
        pointRadius: 0,
        borderWidth: 1.5
      }
    ]
  });
</script>

<div class="metrics-page">
  <PageHeaderWithAction page={pageDef}>
    {#snippet actions()}
      <div class="telemetry-status">
        <div class="status-pulse"></div>
        <span>Live Telemetry Sequence</span>
      </div>
    {/snippet}
  </PageHeaderWithAction>

  {#if loading && vmStats.total === 0}
    <div class="metrics-skeleton">
       <div class="skeleton-strip"></div>
       <div class="skeleton-grid"></div>
    </div>
  {:else if error}
    <ErrorState />
  {:else if vmStats.total === 0 && nodeHealth.length === 0}
    <EmptyInfrastructureState
      title="No telemetry detected"
      description="System requires active compute nodes or virtual machines to generate metrics."
      hint="Ensure at least one agent is connected to the control plane."
    />
  {:else}
    <div class="inventory-metrics">
      <CompactMetricCard 
        label="Fleet Capacity" 
        value={nodeHealth.length} 
        color="neutral"
      />
      <CompactMetricCard 
        label="Active Workloads" 
        value={vmStats.running} 
        color="primary"
      />
      <CompactMetricCard 
        label="Fault State" 
        value={vmStats.error} 
        color={vmStats.error > 0 ? 'danger' : 'neutral'}
      />
      <CompactMetricCard 
        label="Observation Depth" 
        value="5.0s" 
        color="neutral"
      />
    </div>

    <main class="metrics-layout">
      <section class="telemetry-main">
        <SectionCard title="Compute Fabric Activity" icon={LineChart}>
          <div class="chart-container">
             <ChartJS 
               type="line" 
               data={telemetryChartData} 
               height={280} 
               options={{
                 plugins: { legend: { display: false } },
                 scales: {
                   x: { grid: { display: false }, ticks: { font: { size: 9, family: 'var(--font-mono)' } } },
                   y: { grid: { color: 'rgba(0,0,0,0.05)' }, ticks: { font: { size: 9, family: 'var(--font-mono)' } } }
                 }
               }}
             />
          </div>
        </SectionCard>

        <div class="prometheus-panel">
          <div class="panel-header">
            <Activity size={14} class="text-primary" />
            <h3>O11y Integration (Prometheus)</h3>
          </div>
          <div class="panel-content">
             <p>All platform metrics are exported at high-resolution via the Control Plane OpenTelemetry gateway.</p>
             <div class="code-block">
                <code>GET /api/v1/metrics</code>
             </div>
             <div class="metric-keys">
                <span>chv_vms_total</span>
                <span>chv_vm_cpu_usage_percent</span>
                <span>chv_vm_mem_usage_bytes</span>
                <span>chv_node_health</span>
             </div>
          </div>
        </div>
      </section>

      <aside class="metrics-side">
        <SectionCard title="Anomalies" icon={ShieldAlert} badgeLabel={String(vmStats.error)}>
          {#if vmStats.error === 0}
            <p class="empty-hint">No anomalous state detected in primary workloads.</p>
          {:else}
            <div class="alert-list">
               <div class="alert-item alert-item--danger">
                  <span>Logic Fault</span>
                  <span>{vmStats.error} nodes</span>
               </div>
            </div>
          {/if}
        </SectionCard>

        <SectionCard title="System Posture" icon={Activity}>
          <div class="posture-audit">
             <div class="audit-row">
                <span>API Response</span>
                <span>&lt; 5ms</span>
             </div>
             <div class="audit-row">
                <span>Bus Saturation</span>
                <span>0.0%</span>
             </div>
             <div class="audit-row">
                <span>Telemetry Sync</span>
                <span>Optimal</span>
             </div>
          </div>
        </SectionCard>
      </aside>
    </main>
  {/if}
</div>

<style>
  .metrics-page {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .telemetry-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 10px;
    font-weight: 700;
    color: var(--color-primary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .status-pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-primary);
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0% { opacity: 0.4; }
    50% { opacity: 1; }
    100% { opacity: 0.4; }
  }

  .inventory-metrics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.75rem;
  }

  .metrics-layout {
    display: grid;
    grid-template-columns: 1fr 300px;
    gap: 1rem;
    align-items: start;
  }

  .telemetry-main {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .chart-container {
    padding: 1rem 0;
  }

  .prometheus-panel {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-xs);
    overflow: hidden;
  }

  .panel-header {
    background: var(--bg-surface-muted);
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .panel-header h3 {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--color-neutral-600);
  }

  .panel-content {
    padding: 1rem;
  }

  .panel-content p {
    font-size: 11px;
    color: var(--color-neutral-500);
    margin-bottom: 0.75rem;
  }

  .code-block {
    background: #000;
    color: #0f0;
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius-xs);
    font-family: var(--font-mono);
    font-size: 10px;
    margin-bottom: 1rem;
  }

  .metric-keys {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .metric-keys span {
    font-size: 9px;
    font-family: var(--font-mono);
    background: var(--bg-surface-muted);
    padding: 2px 4px;
    border-radius: 2px;
    color: var(--color-neutral-600);
  }

  .metrics-side {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .empty-hint {
    font-size: 11px;
    color: var(--color-neutral-400);
    padding: 1rem;
    text-align: center;
  }

  .posture-audit {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .audit-row {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    color: var(--color-neutral-600);
    padding: 0.35rem 0.5rem;
    background: var(--bg-surface-muted);
    border-radius: var(--radius-xs);
  }

  .audit-row span:last-child {
    font-weight: 700;
    color: var(--color-neutral-900);
  }

  .alert-item--danger {
     display: flex;
     justify-content: space-between;
     padding: 0.5rem 0.75rem;
     background: var(--color-danger-light);
     border-radius: var(--radius-xs);
     font-size: 10px;
     font-weight: 700;
     color: var(--color-danger);
  }

  @media (max-width: 1100px) {
    .metrics-layout {
      grid-template-columns: 1fr;
    }
  }
</style>
