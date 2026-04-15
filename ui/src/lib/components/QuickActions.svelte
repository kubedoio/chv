<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { goto } from '$app/navigation';
  import { 
    Terminal, 
    Plus, 
    Play, 
    Square, 
    RefreshCw, 
    Settings, 
    Image, 
    Network, 
    HardDrive,
    Zap,
    X,
    Command,
    Home,
    Server,
    Download
  } from 'lucide-svelte';
  import Fuse from 'fuse.js';
  import { getModifierKey } from '$lib/stores/keyboard.svelte';
  
  // Props
  interface Props {
    open?: boolean;
    onClose?: () => void;
  }
  
  let { open = $bindable(false), onClose }: Props = $props();
  
  // Action definitions
  interface QuickAction {
    id: string;
    title: string;
    description: string;
    icon: typeof Terminal;
    shortcut?: string[];
    keywords: string[];
    section: string;
    action: () => void;
  }
  
  // Local state
  let query = $state('');
  let selectedIndex = $state(0);
  let isVisible = $state(false);
  let isClosing = $state(false);
  let inputRef = $state<HTMLInputElement | null>(null);
  let resultsContainerRef = $state<HTMLDivElement | null>(null);
  let recentlyUsed = $state<string[]>([]);
  
  const RECENT_ACTIONS_KEY = 'chv-recent-actions';
  const MAX_RECENT = 5;
  
  // All available actions
  const allActions: QuickAction[] = [
    {
      id: 'create-vm',
      title: 'Create Virtual Machine',
      description: 'Launch a new VM wizard',
      icon: Plus,
      shortcut: ['c'],
      keywords: ['vm', 'create', 'new', 'virtual machine', 'launch'],
      section: 'VMs',
      action: () => goto('/vms?create=true')
    },
    {
      id: 'go-dashboard',
      title: 'Go to Overview',
      description: 'View fleet overview',
      icon: Home,
      shortcut: ['g', 'd'],
      keywords: ['overview', 'dashboard', 'home', 'main'],
      section: 'Navigation',
      action: () => goto('/')
    },
    {
      id: 'go-vms',
      title: 'Go to Virtual Machines',
      description: 'View all VMs',
      icon: Server,
      shortcut: ['g', 'v'],
      keywords: ['vms', 'virtual machines', 'instances'],
      section: 'Navigation',
      action: () => goto('/vms')
    },
    {
      id: 'go-images',
      title: 'Go to Images',
      description: 'Manage OS images',
      icon: Image,
      shortcut: ['g', 'i'],
      keywords: ['images', 'os', 'templates', 'iso'],
      section: 'Navigation',
      action: () => goto('/images')
    },
    {
      id: 'go-volumes',
      title: 'Go to Volumes',
      description: 'View volume inventory',
      icon: HardDrive,
      shortcut: ['g', 's'],
      keywords: ['volumes', 'storage', 'pools', 'disks'],
      section: 'Navigation',
      action: () => goto('/volumes')
    },
    {
      id: 'go-networks',
      title: 'Go to Networks',
      description: 'Manage network configuration',
      icon: Network,
      shortcut: ['g', 'n'],
      keywords: ['networks', 'bridges', 'interfaces', 'vlan'],
      section: 'Navigation',
      action: () => goto('/networks')
    },
    {
      id: 'import-image',
      title: 'Import Image',
      description: 'Download an OS image from URL',
      icon: Download,
      keywords: ['import', 'download', 'image', 'os'],
      section: 'Images',
      action: () => goto('/images?import=true')
    },
    {
      id: 'create-network',
      title: 'Create Network',
      description: 'Add a new network bridge',
      icon: Network,
      keywords: ['network', 'create', 'bridge', 'vlan'],
      section: 'Networks',
      action: () => goto('/networks?create=true')
    },
    {
      id: 'create-storage',
      title: 'Create Storage Pool',
      description: 'Add a new storage pool',
      icon: HardDrive,
      keywords: ['storage', 'pool', 'create', 'disk'],
      section: 'Storage',
      action: () => goto('/storage?create=true')
    },
    {
      id: 'refresh-data',
      title: 'Refresh All Data',
      description: 'Reload current page data',
      icon: RefreshCw,
      shortcut: ['r'],
      keywords: ['refresh', 'reload', 'update', 'sync'],
      section: 'System',
      action: () => window.location.reload()
    },
    {
      id: 'open-settings',
      title: 'Open Settings',
      description: 'System configuration',
      icon: Settings,
      keywords: ['settings', 'config', 'preferences'],
      section: 'System',
      action: () => goto('/settings')
    },
    {
      id: 'open-help',
      title: 'Keyboard Shortcuts Help',
      description: 'View all available shortcuts',
      icon: Command,
      shortcut: ['?'],
      keywords: ['help', 'shortcuts', 'keyboard', 'hotkeys'],
      section: 'System',
      action: () => {
        close();
        // The keyboard store handles the '?' key
      }
    }
  ];
  
  // Fuse for fuzzy search
  const fuse = new Fuse(allActions, {
    keys: [
      { name: 'title', weight: 0.5 },
      { name: 'description', weight: 0.3 },
      { name: 'keywords', weight: 0.2 }
    ],
    threshold: 0.4
  });
  
  // Filtered results
  let filteredActions = $derived(getFilteredActions());
  
  function getFilteredActions(): QuickAction[] {
    if (!query.trim()) {
      // Show recently used first, then other actions
      const recent = recentlyUsed
        .map(id => allActions.find(a => a.id === id))
        .filter(Boolean) as QuickAction[];
      
      const others = allActions.filter(a => !recentlyUsed.includes(a.id));
      return [...recent, ...others];
    }
    
    const results = fuse.search(query);
    return results.map(r => r.item);
  }
  
  // Group actions by section
  let groupedActions = $derived(getGroupedActions(filteredActions));
  
  function getGroupedActions(actions: QuickAction[]): Map<string, QuickAction[]> {
    const grouped = new Map<string, QuickAction[]>();
    
    for (const action of actions) {
      if (!grouped.has(action.section)) {
        grouped.set(action.section, []);
      }
      grouped.get(action.section)!.push(action);
    }
    
    return grouped;
  }
  
  // Get flat list for keyboard navigation
  let flatActions = $derived(getFlatActions(groupedActions));
  
  function getFlatActions(grouped: Map<string, QuickAction[]>): QuickAction[] {
    const flat: QuickAction[] = [];
    for (const actions of grouped.values()) {
      flat.push(...actions);
    }
    return flat;
  }
  
  // Load recent actions
  function loadRecentActions() {
    if (typeof localStorage === 'undefined') return;
    try {
      const stored = localStorage.getItem(RECENT_ACTIONS_KEY);
      if (stored) {
        recentlyUsed = JSON.parse(stored);
      }
    } catch {
      recentlyUsed = [];
    }
  }
  
  // Save to recent
  function addToRecent(actionId: string) {
    recentlyUsed = [actionId, ...recentlyUsed.filter(id => id !== actionId)].slice(0, MAX_RECENT);
    if (typeof localStorage !== 'undefined') {
      try {
        localStorage.setItem(RECENT_ACTIONS_KEY, JSON.stringify(recentlyUsed));
      } catch {
        // Ignore
      }
    }
  }
  
  // Handle action selection
  function selectAction(action: QuickAction) {
    addToRecent(action.id);
    close();
    tick().then(() => {
      action.action();
    });
  }
  
  // Handle input
  function handleInput(event: Event) {
    query = (event.target as HTMLInputElement).value;
    selectedIndex = 0;
  }
  
  // Handle keyboard navigation
  function handleKeyDown(event: KeyboardEvent) {
    switch (event.key) {
      case 'Escape':
        event.preventDefault();
        close();
        break;
        
      case 'ArrowDown':
        event.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, flatActions.length - 1);
        scrollToSelected();
        break;
        
      case 'ArrowUp':
        event.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        scrollToSelected();
        break;
        
      case 'Enter':
        event.preventDefault();
        const action = flatActions[selectedIndex];
        if (action) {
          selectAction(action);
        }
        break;
    }
  }
  
  function scrollToSelected() {
    tick().then(() => {
      const selectedEl = resultsContainerRef?.querySelector(`[data-index="${selectedIndex}"]`);
      if (selectedEl) {
        selectedEl.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      }
    });
  }
  
  // Close handlers
  function close() {
    if (isClosing) return;
    isClosing = true;
    
    setTimeout(() => {
      isVisible = false;
      open = false;
      query = '';
      selectedIndex = 0;
      isClosing = false;
      onClose?.();
    }, 150);
  }
  
  function handleBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      close();
    }
  }
  
  // Get global index for an action
  function getGlobalIndex(sectionIndex: number, actionIndex: number): number {
    let count = 0;
    const sections = Array.from(groupedActions.entries());
    
    for (let i = 0; i < sectionIndex; i++) {
      count += sections[i][1].length;
    }
    
    return count + actionIndex;
  }
  
  // Watch for open state
  $effect(() => {
    if (open && !isVisible) {
      loadRecentActions();
      tick().then(() => {
        isVisible = true;
        inputRef?.focus();
      });
    }
  });
  
  onMount(() => {
    if (open) {
      loadRecentActions();
      tick().then(() => inputRef?.focus());
    }
  });
</script>

{#if open}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 bg-black/50 flex items-start justify-center pt-[15vh] transition-opacity duration-150"
    class:opacity-0={!isVisible || isClosing}
    class:opacity-100={isVisible && !isClosing}
    onclick={handleBackdropClick}
    aria-hidden="true"
  >
    <!-- Modal Container -->
    <div
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      aria-label="Quick actions"
      class="w-full max-w-xl mx-4 bg-white rounded-lg shadow-2xl overflow-hidden transition-all duration-150"
      class:scale-95={!isVisible || isClosing}
      class:scale-100={isVisible && !isClosing}
      class:opacity-0={!isVisible || isClosing}
      class:opacity-100={isVisible && !isClosing}
      onclick={(e) => e.stopPropagation()}
    >
      <!-- Header -->
      <div class="flex items-center gap-3 px-4 py-3 border-b border-gray-200">
        <Zap size={20} class="text-amber-500" />
        <input
          bind:this={inputRef}
          type="text"
          value={query}
          oninput={handleInput}
          onkeydown={handleKeyDown}
          placeholder="What would you like to do?"
          class="flex-1 bg-transparent text-base outline-none placeholder:text-gray-400"
          aria-label="Quick action search"
        />
        {#if query}
          <button
            onclick={() => { query = ''; inputRef?.focus(); }}
            class="p-1 rounded hover:bg-gray-100 text-gray-400 hover:text-gray-600"
            aria-label="Clear search"
          >
            <X size={16} />
          </button>
        {/if}
        <kbd class="hidden sm:inline-flex items-center gap-1 px-2 py-1 text-xs font-mono bg-gray-100 text-gray-500 rounded border border-gray-200">
          ESC
        </kbd>
      </div>
      
      <!-- Results -->
      <div 
        bind:this={resultsContainerRef}
        class="max-h-[50vh] overflow-y-auto"
        role="listbox"
      >
        {#if flatActions.length === 0}
          <div class="px-4 py-8 text-center text-gray-500">
            <p>No actions found for "{query}"</p>
            <p class="text-sm mt-1 text-gray-400">Try a different search term</p>
          </div>
        {:else}
          {@const sections = Array.from(groupedActions.entries())}
          {#each sections as [section, actions], sectionIndex}
            <div class="py-1">
              <div class="px-4 py-1.5 text-xs font-semibold text-gray-500 uppercase tracking-wider bg-gray-50">
                {section}
              </div>
              {#each actions as action, actionIndex}
                {@const globalIndex = getGlobalIndex(sectionIndex, actionIndex)}
                {@const Icon = action.icon}
                <button
                  data-index={globalIndex}
                  role="option"
                  aria-selected={selectedIndex === globalIndex}
                  class="w-full px-4 py-2.5 flex items-center gap-3 text-left hover:bg-gray-100 transition-colors"
                  class:bg-blue-50={selectedIndex === globalIndex}
                  onclick={() => selectAction(action)}
                >
                  <Icon size={18} class="text-gray-500 flex-shrink-0" />
                  <div class="flex-1 min-w-0">
                    <div class="text-sm font-medium text-gray-900">{action.title}</div>
                    <div class="text-xs text-gray-500">{action.description}</div>
                  </div>
                  {#if action.shortcut}
                    <div class="hidden sm:flex items-center gap-1">
                      {#each action.shortcut as key}
                        <kbd class="px-1.5 py-0.5 text-xs font-mono bg-gray-100 text-gray-600 rounded border border-gray-200">
                          {key.toUpperCase()}
                        </kbd>
                      {/each}
                    </div>
                  {/if}
                </button>
              {/each}
            </div>
          {/each}
        {/if}
      </div>
      
      <!-- Footer -->
      <div class="px-4 py-2 bg-gray-50 border-t border-gray-200 flex items-center justify-between text-xs text-gray-500">
        <div class="flex items-center gap-3">
          <span class="flex items-center gap-1">
            <kbd class="px-1.5 py-0.5 bg-white rounded border border-gray-200">↑↓</kbd>
            <span>Navigate</span>
          </span>
          <span class="flex items-center gap-1">
            <kbd class="px-1.5 py-0.5 bg-white rounded border border-gray-200">↵</kbd>
            <span>Select</span>
          </span>
        </div>
        <div class="flex items-center gap-2">
          <span>{getModifierKey()} + Shift + P</span>
          <span>to open</span>
        </div>
      </div>
    </div>
  </div>
{/if}
