<script>
  import { Activity, Cpu, MemoryStick, HardDrive } from 'lucide-svelte';
  
  // Props
  let { vms = { total: 0, running: 0, stopped: 0, error: 0 } } = $props();
  
  // Calculate percentages for visualization
  let runningPercent = $derived(vms.total > 0 ? (vms.running / vms.total) * 100 : 0);
  let stoppedPercent = $derived(vms.total > 0 ? (vms.stopped / vms.total) * 100 : 0);
  let errorPercent = $derived(vms.total > 0 ? (vms.error / vms.total) * 100 : 0);
</script>

<div class="bg-white rounded-lg border border-slate-200 p-6">
  <h2 class="text-lg font-semibold text-slate-900 mb-4 flex items-center gap-2">
    <Activity size={20} class="text-slate-500" />
    VM Resource Overview
  </h2>
  
  <div class="space-y-6">
    <!-- VM State Distribution -->
    <div>
      <h3 class="text-sm font-medium text-slate-700 mb-3">VM State Distribution</h3>
      
      <!-- Progress Bar -->
      <div class="h-4 bg-slate-100 rounded-full overflow-hidden flex">
        {#if runningPercent > 0}
          <div 
            class="h-full bg-green-500 transition-all duration-500"
            style="width: {runningPercent}%"
            title="Running: {vms.running}"
          ></div>
        {/if}
        {#if stoppedPercent > 0}
          <div 
            class="h-full bg-slate-400 transition-all duration-500"
            style="width: {stoppedPercent}%"
            title="Stopped: {vms.stopped}"
          ></div>
        {/if}
        {#if errorPercent > 0}
          <div 
            class="h-full bg-red-500 transition-all duration-500"
            style="width: {errorPercent}%"
            title="Error: {vms.error}"
          ></div>
        {/if}
      </div>
      
      <!-- Legend -->
      <div class="flex flex-wrap gap-4 mt-3">
        <div class="flex items-center gap-2">
          <div class="w-3 h-3 rounded-full bg-green-500"></div>
          <span class="text-sm text-slate-600">Running ({vms.running})</span>
        </div>
        <div class="flex items-center gap-2">
          <div class="w-3 h-3 rounded-full bg-slate-400"></div>
          <span class="text-sm text-slate-600">Stopped ({vms.stopped})</span>
        </div>
        {#if vms.error > 0}
          <div class="flex items-center gap-2">
            <div class="w-3 h-3 rounded-full bg-red-500"></div>
            <span class="text-sm text-slate-600">Error ({vms.error})</span>
          </div>
        {/if}
      </div>
    </div>
    
    <!-- Resource Metrics Grid -->
    <div class="grid grid-cols-2 gap-4">
      <div class="bg-slate-50 rounded-lg p-4">
        <div class="flex items-center gap-2 mb-2">
          <Cpu size={18} class="text-blue-500" />
          <span class="text-sm font-medium text-slate-700">CPU Usage</span>
        </div>
        <div class="text-2xl font-semibold text-slate-900">
          {vms.running > 0 ? 'Active' : '—'}
        </div>
        <div class="text-xs text-slate-500 mt-1">
          {#if vms.running > 0}
            {vms.running} VMs using CPU
          {:else}
            No running VMs
          {/if}
        </div>
      </div>
      
      <div class="bg-slate-50 rounded-lg p-4">
        <div class="flex items-center gap-2 mb-2">
          <MemoryStick size={18} class="text-purple-500" />
          <span class="text-sm font-medium text-slate-700">Memory</span>
        </div>
        <div class="text-2xl font-semibold text-slate-900">
          {vms.running > 0 ? 'Allocated' : '—'}
        </div>
        <div class="text-xs text-slate-500 mt-1">
          {#if vms.running > 0}
            Memory allocated to VMs
          {:else}
            No memory in use
          {/if}
        </div>
      </div>
      
      <div class="bg-slate-50 rounded-lg p-4">
        <div class="flex items-center gap-2 mb-2">
          <HardDrive size={18} class="text-orange-500" />
          <span class="text-sm font-medium text-slate-700">Storage</span>
        </div>
        <div class="text-2xl font-semibold text-slate-900">
          {vms.total > 0 ? 'Provisioned' : '—'}
        </div>
        <div class="text-xs text-slate-500 mt-1">
          {#if vms.total > 0}
            {vms.total} VM disks
          {:else}
            No VMs created
          {/if}
        </div>
      </div>
      
      <div class="bg-slate-50 rounded-lg p-4">
        <div class="flex items-center gap-2 mb-2">
          <Activity size={18} class="text-green-500" />
          <span class="text-sm font-medium text-slate-700">Health</span>
        </div>
        <div class="text-2xl font-semibold text-slate-900">
          {#if vms.error > 0}
            <span class="text-red-500">{vms.error} Issues</span>
          {:else if vms.running > 0}
            <span class="text-green-500">Healthy</span>
          {:else}
            <span class="text-slate-400">Idle</span>
          {/if}
        </div>
        <div class="text-xs text-slate-500 mt-1">
          {#if vms.error > 0}
            {vms.error} VM(s) in error state
          {:else if vms.running > 0}
            All systems operational
          {:else}
            No active VMs
          {/if}
        </div>
      </div>
    </div>
    
    <!-- Quick Stats -->
    <div class="border-t border-slate-100 pt-4">
      <h3 class="text-sm font-medium text-slate-700 mb-3">Quick Stats</h3>
      <div class="grid grid-cols-3 gap-4 text-center">
        <div class="p-3 bg-green-50 rounded-lg">
          <div class="text-2xl font-semibold text-green-600">{vms.running}</div>
          <div class="text-xs text-green-700 mt-1">Active</div>
        </div>
        <div class="p-3 bg-slate-50 rounded-lg">
          <div class="text-2xl font-semibold text-slate-600">{vms.stopped}</div>
          <div class="text-xs text-slate-700 mt-1">Inactive</div>
        </div>
        <div class="p-3 bg-blue-50 rounded-lg">
          <div class="text-2xl font-semibold text-blue-600">{vms.total}</div>
          <div class="text-xs text-blue-700 mt-1">Total</div>
        </div>
      </div>
    </div>
  </div>
</div>
