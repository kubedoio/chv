<script lang="ts">
  import Card from '$lib/components/primitives/Card.svelte';
  import Badge from '$lib/components/primitives/Badge.svelte';
  import StateBadge from '$lib/components/StateBadge.svelte';
  import { 
    Server, 
    Play, 
    Square, 
    RotateCcw, 
    Trash2, 
    Cpu, 
    HardDrive, 
    Network,
    MoreVertical,
    Copy,
    Check
  } from 'lucide-svelte';
  import type { VM } from '$lib/api/types';

  interface Props {
    vm: VM;
    imageName?: string;
    poolName?: string;
    networkName?: string;
    selected?: boolean;
    onSelect?: (id: string) => void;
    onStart?: (id: string) => void;
    onStop?: (id: string) => void;
    onRestart?: (id: string) => void;
    onDelete?: (id: string) => void;
    onClick?: (id: string) => void;
  }

  let {
    vm,
    imageName,
    poolName,
    networkName,
    selected = false,
    onSelect,
    onStart,
    onStop,
    onRestart,
    onDelete,
    onClick
  }: Props = $props();

  let showActions = $state(false);
  let copiedId = $state(false);

  const isRunning = $derived(vm.actual_state === 'running');
  const isStopped = $derived(vm.actual_state === 'stopped');
  const isTransitioning = $derived(['starting', 'stopping'].includes(vm.actual_state));

  // Mini sparkline data (simulated history based on current state)
  const sparklineData = $derived({
    cpu: Array(20).fill(0).map(() => Math.random() * (isRunning ? 60 : 5) + (isRunning ? 10 : 0)),
    memory: Array(20).fill(0).map(() => Math.random() * (isRunning ? 40 : 5) + (isRunning ? 20 : 0))
  });

  function handleCopyId() {
    navigator.clipboard.writeText(vm.id);
    copiedId = true;
    setTimeout(() => copiedId = false, 2000);
  }

  function handleAction(e: Event, action: () => void) {
    e.stopPropagation();
    action();
  }
</script>

<div class="vm-card-wrapper {selected ? 'ring-2 ring-primary' : ''} {isTransitioning ? 'opacity-75' : ''}">
<Card interactive={true} padding="none">
  {#snippet header()}
    <div class="flex items-start justify-between p-4">
      <div class="flex items-start gap-3">
        <!-- Selection checkbox -->
        {#if onSelect}
          <button
            onclick={(e) => {
              e.stopPropagation();
              onSelect(vm.id);
            }}
            class="mt-1 text-muted hover:text-primary transition-colors"
          >
            {#if selected}
              <div class="w-4 h-4 bg-primary rounded flex items-center justify-center">
                <Check size={10} class="text-white" />
              </div>
            {:else}
              <div class="w-4 h-4 border-2 border-line rounded hover:border-primary transition-colors"></div>
            {/if}
          </button>
        {/if}

        <div>
          <!-- VM Name -->
          <button 
            class="font-semibold text-ink text-lg leading-tight hover:text-primary transition-colors text-left bg-transparent border-0 p-0"
            onclick={() => onClick?.(vm.id)}
          >
            {vm.name}
          </button>
          
          <!-- VM ID with copy -->
          <div class="flex items-center gap-1 mt-1">
            <code class="text-xs text-muted font-mono">{vm.id.slice(0, 8)}...</code>
            <button
              onclick={handleCopyId}
              class="text-muted hover:text-primary transition-colors p-0.5"
              title="Copy ID"
            >
              {#if copiedId}
                <Check size={12} class="text-success" />
              {:else}
                <Copy size={12} />
              {/if}
            </button>
          </div>
        </div>
      </div>

      <!-- Status badge -->
      <StateBadge label={vm.actual_state} />
    </div>
  {/snippet}

  <div class="px-4 pb-4 space-y-4">
    <!-- Resource mini-charts -->
    <div class="grid grid-cols-2 gap-3">
      <!-- CPU mini-chart -->
      <div class="bg-chrome rounded-lg p-2">
        <div class="flex items-center justify-between mb-1">
          <div class="flex items-center gap-1 text-muted">
            <Cpu size={12} />
            <span class="text-xs">CPU</span>
          </div>
          <span class="text-xs font-medium text-ink">{vm.vcpu} vCPU</span>
        </div>
        <svg class="w-full h-6" viewBox="0 0 100 20" preserveAspectRatio="none">
          <path
            d={`M 0,20 ${sparklineData.cpu.map((v, i) => `L ${(i / 19) * 100},${20 - v * 0.2}`).join(' ')} L 100,20 Z`}
            fill={isRunning ? 'rgba(34, 197, 94, 0.2)' : 'rgba(148, 163, 184, 0.2)'}
            stroke={isRunning ? '#22c55e' : '#94a3b8'}
            stroke-width="1"
          />
        </svg>
      </div>

      <!-- Memory mini-chart -->
      <div class="bg-chrome rounded-lg p-2">
        <div class="flex items-center justify-between mb-1">
          <div class="flex items-center gap-1 text-muted">
            <HardDrive size={12} />
            <span class="text-xs">RAM</span>
          </div>
          <span class="text-xs font-medium text-ink">{vm.memory_mb} MB</span>
        </div>
        <svg class="w-full h-6" viewBox="0 0 100 20" preserveAspectRatio="none">
          <path
            d={`M 0,20 ${sparklineData.memory.map((v, i) => `L ${(i / 19) * 100},${20 - v * 0.2}`).join(' ')} L 100,20 Z`}
            fill={isRunning ? 'rgba(59, 130, 246, 0.2)' : 'rgba(148, 163, 184, 0.2)'}
            stroke={isRunning ? '#3b82f6' : '#94a3b8'}
            stroke-width="1"
          />
        </svg>
      </div>
    </div>

    <!-- Info grid -->
    <div class="space-y-1.5 text-xs">
      <div class="flex items-center justify-between">
        <span class="text-muted flex items-center gap-1">
          <Server size={12} />
          Image
        </span>
        <span class="text-ink truncate max-w-[120px]" title={imageName}>{imageName || vm.image_id?.slice(0, 8)}</span>
      </div>
      <div class="flex items-center justify-between">
        <span class="text-muted flex items-center gap-1">
          <HardDrive size={12} />
          Pool
        </span>
        <span class="text-ink truncate max-w-[120px]" title={poolName}>{poolName || vm.storage_pool_id?.slice(0, 8)}</span>
      </div>
      <div class="flex items-center justify-between">
        <span class="text-muted flex items-center gap-1">
          <Network size={12} />
          Network
        </span>
        <span class="text-ink truncate max-w-[120px]" title={networkName}>{networkName || vm.network_id?.slice(0, 8)}</span>
      </div>
      {#if vm.ip_address}
        <div class="flex items-center justify-between">
          <span class="text-muted flex items-center gap-1">
            <Network size={12} />
            IP
          </span>
          <Badge variant="info" dot={false}>{vm.ip_address}</Badge>
        </div>
      {/if}
    </div>

    <!-- Quick action buttons -->
    <div class="flex items-center gap-2 pt-2 border-t border-line">
      {#if isStopped}
        <button
          onclick={(e) => handleAction(e, () => onStart?.(vm.id))}
          class="flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-success/10 text-success hover:bg-success hover:text-white rounded transition-all"
        >
          <Play size={12} />
          Start
        </button>
      {:else if isRunning}
        <button
          onclick={(e) => handleAction(e, () => onStop?.(vm.id))}
          class="flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-warning/10 text-warning hover:bg-warning hover:text-white rounded transition-all"
        >
          <Square size={12} />
          Stop
        </button>
        <button
          onclick={(e) => handleAction(e, () => onRestart?.(vm.id))}
          class="flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-chrome text-muted hover:bg-primary hover:text-white rounded transition-all"
        >
          <RotateCcw size={12} />
          Restart
        </button>
      {:else}
        <span class="flex-1 text-xs text-muted text-center py-1.5">
          {vm.actual_state}...
        </span>
      {/if}
      
      <button
        onclick={(e) => handleAction(e, () => onDelete?.(vm.id))}
        class="p-1.5 text-muted hover:text-danger hover:bg-danger/10 rounded transition-all"
        title="Delete"
      >
        <Trash2 size={14} />
      </button>
    </div>
  </div>
</Card>
</div>

<style>
  .vm-card-wrapper {
    transition: all 0.2s ease;
  }
  
  .vm-card-wrapper:hover {
    transform: translateY(-2px);
  }
</style>
