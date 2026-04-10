<script lang="ts">
  import { Power, Play, Square, RotateCcw, RefreshCw, AlertTriangle } from 'lucide-svelte';
  import { slide } from 'svelte/transition';
  import { clickOutside } from '$lib/actions/clickOutside';

  interface Props {
    vmState: string;
    disabled?: boolean;
    onAction: (action: string, options?: { graceful?: boolean; timeout?: number }) => void;
  }

  let { vmState, disabled = false, onAction }: Props = $props();

  let isOpen = $state(false);
  let showConfirm = $state<string | null>(null);
  let confirmTimeout = $state(60);
  let confirmGraceful = $state(true);

  const isRunning = $derived(vmState === 'running');
  const isStopped = $derived(vmState === 'stopped' || vmState === 'prepared');
  const isTransitioning = $derived(['starting', 'stopping', 'provisioning'].includes(vmState));

  const actions = $derived([
    { 
      id: 'start', 
      label: 'Start', 
      icon: Play, 
      show: isStopped,
      danger: false,
      confirm: false
    },
    { 
      id: 'shutdown', 
      label: 'Graceful Shutdown', 
      icon: Power, 
      show: isRunning,
      danger: false,
      confirm: true,
      confirmTitle: 'Shutdown VM?',
      confirmMessage: 'Send ACPI shutdown signal to gracefully stop the VM.',
      options: { graceful: true }
    },
    { 
      id: 'force-stop', 
      label: 'Force Stop', 
      icon: Square, 
      show: isRunning,
      danger: true,
      confirm: true,
      confirmTitle: 'Force Stop VM?',
      confirmMessage: 'This will immediately terminate the VM process. Data loss may occur.'
    },
    { 
      id: 'reset', 
      label: 'Reset', 
      icon: RotateCcw, 
      show: isRunning,
      danger: false,
      confirm: true,
      confirmTitle: 'Reset VM?',
      confirmMessage: 'Power cycle the VM without shutdown. Unsaved data may be lost.'
    },
    { 
      id: 'restart', 
      label: 'Restart', 
      icon: RefreshCw, 
      show: isRunning,
      danger: false,
      confirm: true,
      confirmTitle: 'Restart VM?',
      confirmMessage: 'Shutdown and restart the VM.',
      hasOptions: true
    },
  ]);

  const visibleActions = $derived(actions.filter(a => a.show));

  function handleActionClick(actionId: string) {
    const action = actions.find(a => a.id === actionId);
    if (!action) return;

    if (action.confirm) {
      showConfirm = actionId;
      // Reset options to defaults
      confirmTimeout = 60;
      confirmGraceful = true;
    } else {
      executeAction(actionId);
    }
    isOpen = false;
  }

  function executeAction(actionId: string) {
    const action = actions.find(a => a.id === actionId);
    if (!action) return;

    let options: { graceful?: boolean; timeout?: number } = {};
    
    if (actionId === 'restart' && confirmGraceful) {
      options = { graceful: true, timeout: confirmTimeout };
    } else if (actionId === 'shutdown') {
      options = { timeout: confirmTimeout };
    }

    onAction(actionId, options);
    showConfirm = null;
  }

  function cancelConfirm() {
    showConfirm = null;
  }

  function toggleMenu() {
    isOpen = !isOpen;
  }
</script>

<div class="relative" use:clickOutside={() => isOpen = false}>
  <!-- Main Power Button -->
  <button
    onclick={toggleMenu}
    disabled={disabled || isTransitioning}
    class="flex items-center gap-2 px-3 py-2 rounded border transition-colors
      {isRunning 
        ? 'bg-amber-50 border-amber-200 text-amber-700 hover:bg-amber-100' 
        : isStopped 
          ? 'bg-emerald-50 border-emerald-200 text-emerald-700 hover:bg-emerald-100'
          : 'bg-gray-50 border-gray-200 text-gray-500'}
      disabled:opacity-50 disabled:cursor-not-allowed"
    title="Power Actions"
  >
    <Power size={16} />
    <span class="text-sm font-medium">Power</span>
    <svg class="w-4 h-4 ml-1 transition-transform {isOpen ? 'rotate-180' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
    </svg>
  </button>

  <!-- Dropdown Menu -->
  {#if isOpen}
    <div 
      class="absolute right-0 mt-2 w-56 bg-white rounded-lg shadow-lg border border-line z-50 overflow-hidden"
      transition:slide={{ duration: 150 }}
    >
      <div class="py-1">
        {#each visibleActions as action}
          <button
            onclick={() => handleActionClick(action.id)}
            class="w-full flex items-center gap-3 px-4 py-2.5 text-left text-sm transition-colors
              {action.danger 
                ? 'text-rose-600 hover:bg-rose-50' 
                : 'text-gray-700 hover:bg-gray-50'}"
          >
            <action.icon size={16} />
            <span>{action.label}</span>
            {#if action.confirm}
              <AlertTriangle size={14} class="ml-auto opacity-50" />
            {/if}
          </button>
        {/each}
        
        {#if visibleActions.length === 0}
          <div class="px-4 py-3 text-sm text-gray-500 text-center">
            No actions available
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<!-- Confirmation Modal -->
{#if showConfirm}
  {@const action = actions.find(a => a.id === showConfirm)}
  {#if action}
    <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
      <div class="bg-white rounded-lg shadow-lg w-full max-w-md" transition:slide={{ duration: 200 }}>
        <div class="px-6 py-4 border-b border-line">
          <h3 class="text-lg font-semibold text-gray-900">{action.confirmTitle}</h3>
        </div>
        
        <div class="px-6 py-4">
          <p class="text-gray-600 mb-4">{action.confirmMessage}</p>
          
          {#if action.hasOptions && showConfirm === 'restart'}
            <div class="space-y-3 bg-gray-50 p-3 rounded">
              <label class="flex items-center gap-2">
                <input 
                  type="checkbox" 
                  bind:checked={confirmGraceful}
                  class="rounded border-gray-300"
                />
                <span class="text-sm">Graceful shutdown</span>
              </label>
              
              {#if confirmGraceful}
                <label class="flex items-center gap-2">
                  <span class="text-sm text-gray-600 w-20">Timeout:</span>
                  <input 
                    type="number" 
                    bind:value={confirmTimeout}
                    min="10"
                    max="300"
                    class="w-20 px-2 py-1 text-sm border border-line rounded"
                  />
                  <span class="text-sm text-gray-500">seconds</span>
                </label>
              {/if}
            </div>
          {/if}
          
          {#if showConfirm === 'shutdown'}
            <div class="space-y-3 bg-gray-50 p-3 rounded">
              <label class="flex items-center gap-2">
                <span class="text-sm text-gray-600 w-20">Timeout:</span>
                <input 
                  type="number" 
                  bind:value={confirmTimeout}
                  min="10"
                  max="300"
                  class="w-20 px-2 py-1 text-sm border border-line rounded"
                />
                <span class="text-sm text-gray-500">seconds</span>
              </label>
            </div>
          {/if}
        </div>
        
        <div class="px-6 py-4 border-t border-line flex justify-end gap-3">
          <button 
            onclick={cancelConfirm}
            class="px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100 rounded transition-colors"
          >
            Cancel
          </button>
          <button 
            onclick={() => executeAction(showConfirm)}
            class="px-4 py-2 text-sm font-medium text-white rounded transition-colors
              {action.danger ? 'bg-rose-600 hover:bg-rose-700' : 'bg-accent hover:bg-accent/90'}"
          >
            {action.label}
          </button>
        </div>
      </div>
    </div>
  {/if}
{/if}
