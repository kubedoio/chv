<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Download, RefreshCw, ScrollText } from 'lucide-svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';

  interface Props {
    vmId: string;
  }

  let { vmId }: Props = $props();

  const client = createAPIClient({ token: getStoredToken() ?? undefined });

  let logs = $state<string[]>([]);
  let loading = $state(true);
  let autoScroll = $state(true);
  let pollInterval = $state<number | null>(null);
  let logContainer = $state<HTMLDivElement | null>(null);
  let linesToShow = $state(100);

  // Polling is active when VM is in a state that might produce logs
  let isPolling = $state(true);

  onMount(() => {
    loadLogs();
    startPolling();
  });

  onDestroy(() => {
    stopPolling();
  });

  function startPolling() {
    // Poll every 3 seconds for new logs
    pollInterval = window.setInterval(() => {
      if (isPolling) {
        loadLogs();
      }
    }, 3000);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  async function loadLogs() {
    try {
      const response = await client.getBootLogs(vmId, linesToShow);
      const newLogs = response.lines.map((l: { content: string }) => l.content);
      
      // Only update if logs changed
      if (JSON.stringify(newLogs) !== JSON.stringify(logs)) {
        logs = newLogs;
        
        if (autoScroll && logContainer) {
          // Scroll to bottom after update
          setTimeout(() => {
            if (logContainer) {
              logContainer.scrollTop = logContainer.scrollHeight;
            }
          }, 0);
        }
      }
    } catch (e: any) {
      // Don't show toast on auto-poll, only log to console
      console.error('Failed to load boot logs:', e);
    } finally {
      loading = false;
    }
  }

  function handleScroll() {
    if (!logContainer) return;
    
    // Check if user has scrolled away from bottom
    const isAtBottom = 
      logContainer.scrollHeight - logContainer.scrollTop <= logContainer.clientHeight + 50;
    autoScroll = isAtBottom;
  }

  function downloadLogs() {
    const content = logs.join('\n');
    const blob = new Blob([content], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `vm-${vmId}-boot.log`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    
    toast.success('Boot logs downloaded');
  }

  function togglePolling() {
    isPolling = !isPolling;
    if (isPolling) {
      loadLogs();
    }
  }
</script>

<div class="boot-log-viewer">
  <!-- Toolbar -->
  <div class="flex items-center justify-between mb-4 p-3 bg-gray-50 rounded-lg border border-line">
    <div class="flex items-center gap-3">
      <ScrollText size={18} class="text-gray-500" />
      <span class="text-sm font-medium text-gray-700">
        Boot Logs
        {#if loading}
          <span class="text-gray-400 font-normal">(loading...)</span>
        {:else}
          <span class="text-gray-400 font-normal">({logs.length} lines)</span>
        {/if}
      </span>
    </div>
    
    <div class="flex items-center gap-2">
      <!-- Auto-scroll toggle -->
      <label class="flex items-center gap-2 text-sm text-gray-600 mr-2">
        <input 
          type="checkbox" 
          bind:checked={autoScroll}
          class="rounded border-gray-300"
        />
        Auto-scroll
      </label>
      
      <!-- Polling toggle -->
      <button
        onclick={togglePolling}
        class="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded transition-colors
          {isPolling ? 'bg-emerald-50 text-emerald-700' : 'bg-gray-100 text-gray-600 hover:bg-gray-200'}"
        title={isPolling ? 'Stop auto-refresh' : 'Start auto-refresh'}
      >
        <span class="relative flex h-2 w-2">
          {#if isPolling}
            <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
          {/if}
          <span class="relative inline-flex rounded-full h-2 w-2 {isPolling ? 'bg-emerald-500' : 'bg-gray-400'}"></span>
        </span>
        {isPolling ? 'Live' : 'Paused'}
      </button>
      
      <!-- Refresh button -->
      <button
        onclick={loadLogs}
        disabled={loading}
        class="p-1.5 text-gray-600 hover:bg-gray-200 rounded transition-colors disabled:opacity-50"
        title="Refresh logs"
      >
        <RefreshCw size={16} class={loading ? 'animate-spin' : ''} />
      </button>
      
      <!-- Download button -->
      <button
        onclick={downloadLogs}
        disabled={logs.length === 0}
        class="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-white border border-line rounded hover:bg-gray-50 
               transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <Download size={14} />
        Download
      </button>
    </div>
  </div>

  <!-- Lines selector -->
  <div class="flex items-center gap-2 mb-3">
    <span class="text-xs text-gray-500">Show last:</span>
    <select 
      bind:value={linesToShow}
      onchange={loadLogs}
      class="text-xs border border-line rounded px-2 py-1 bg-white"
    >
      <option value={50}>50 lines</option>
      <option value={100}>100 lines</option>
      <option value={250}>250 lines</option>
      <option value={500}>500 lines</option>
      <option value={1000}>1000 lines</option>
    </select>
  </div>

  <!-- Log display -->
  <div 
    bind:this={logContainer}
    onscroll={handleScroll}
    class="font-mono text-xs bg-gray-900 text-gray-100 p-4 rounded-lg overflow-auto max-h-[600px] min-h-[300px] leading-relaxed"
  >
    {#if logs.length === 0}
      <div class="text-gray-500 text-center py-8">
        {#if loading}
          Loading boot logs...
        {:else}
          No boot logs available
          <p class="text-gray-600 mt-1">Logs will appear here when the VM starts booting.</p>
        {/if}
      </div>
    {:else}
      {#each logs as line, i}
        <div class="flex gap-3 hover:bg-gray-800/50 px-1 -mx-1 rounded">
          <span class="text-gray-600 select-none w-12 text-right flex-shrink-0">{i + 1}</span>
          <span class="break-all">{line}</span>
        </div>
      {/each}
    {/if}
  </div>

  <!-- Scroll to bottom button (shown when not at bottom) -->
  {#if !autoScroll && logs.length > 0}
    <button
      onclick={() => {
        if (logContainer) {
          logContainer.scrollTop = logContainer.scrollHeight;
          autoScroll = true;
        }
      }}
      class="fixed bottom-8 right-8 bg-accent text-white px-4 py-2 rounded-full shadow-lg 
             hover:bg-accent/90 transition-colors text-sm font-medium flex items-center gap-2"
    >
      <span>Scroll to bottom</span>
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 14l-7 7m0 0l-7-7m7 7V3" />
      </svg>
    </button>
  {/if}
</div>

<style>
  .boot-log-viewer {
    width: 100%;
  }
  
  /* Custom scrollbar for log display */
  div::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }
  
  div::-webkit-scrollbar-track {
    background: #1f2937;
    border-radius: 4px;
  }
  
  div::-webkit-scrollbar-thumb {
    background: #4b5563;
    border-radius: 4px;
  }
  
  div::-webkit-scrollbar-thumb:hover {
    background: #6b7280;
  }
</style>
