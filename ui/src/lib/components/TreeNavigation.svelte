<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { slide } from 'svelte/transition';
  import { 
    Server, 
    HardDrive, 
    Network, 
    ChevronRight,
    ChevronDown,
    Database,
    FolderTree,
    Circle,
    LayoutGrid,
  } from 'lucide-svelte';
  import { createAPIClient, clearToken, getStoredToken } from '$lib/api/client';
  import type { VM } from '$lib/api/types';
  import UserMenu from './UserMenu.svelte';
  import VisuallyHidden from './VisuallyHidden.svelte';
  
  // Props
  interface Props {
    nodes?: import('$lib/api/nodes').Node[];
  }
  
  let { nodes = [] }: Props = $props();
  
  // Simple state - no complex derived objects
  let vms = $state<VM[]>([]);
  let expandedNodes = $state<Set<string>>(new Set(['datacenter', 'nodes']));
  let currentPath = $state($page.url.pathname);
  let focusedNodeId = $state<string | null>(null);
  let treeElement = $state<HTMLElement | null>(null);
  
  // Update current path when page changes
  $effect(() => {
    currentPath = $page.url.pathname;
  });
  
  // Fetch VMs once on mount
  onMount(() => {
    loadVMs();
  });
  
  async function loadVMs() {
    const token = getStoredToken();
    if (!token) return;
    
    try {
      const client = createAPIClient({ token });
      vms = await client.listVMs();
    } catch (err) {
      console.error('Failed to fetch VMs:', err);
      vms = [];
    }
  }
  
  // Simple toggle function
  function toggleNode(nodeId: string, event?: Event) {
    event?.preventDefault();
    event?.stopPropagation();
    
    const newSet = new Set(expandedNodes);
    if (newSet.has(nodeId)) {
      newSet.delete(nodeId);
    } else {
      newSet.add(nodeId);
    }
    expandedNodes = newSet;
  }
  
  function isExpanded(nodeId: string): boolean {
    return expandedNodes.has(nodeId);
  }
  
  function isActive(href: string): boolean {
    if (!href) return false;
    if (href === '/') return currentPath === '/';
    return currentPath.startsWith(href);
  }
  
  // Icon mapping
  const iconComponents: Record<string, typeof Server> = {
    server: Server,
    hardDrive: HardDrive,
    network: Network,
    datacenter: Database,
    folder: FolderTree,
    layout: LayoutGrid,
  };
  
  function getIcon(iconName: string) {
    return iconComponents[iconName] || Circle;
  }
  
  function getStatusColor(status?: string): string {
    switch (status) {
      case 'online':
      case 'running':
        return 'text-green-500';
      case 'offline':
      case 'stopped':
        return 'text-gray-400';
      case 'warning':
        return 'text-yellow-500';
      case 'error':
        return 'text-red-500';
      default:
        return 'text-slate-400';
    }
  }
</script>

<aside 
  class="h-screen flex flex-col bg-[#252532] text-slate-300 w-64 border-r border-[#1e1e28]"
  role="navigation"
  aria-label="Main navigation"
>
  <!-- Header -->
  <header class="h-14 flex items-center px-4 border-b border-[#1e1e28] bg-[#1e1e28]">
    <div class="flex items-center gap-3">
      <div class="w-8 h-8 rounded bg-gradient-to-br from-[#e57035] to-[#d14a28] flex items-center justify-center">
        <Database class="text-white" size={18} />
      </div>
      <div>
        <div class="text-sm font-semibold text-white">CHV Manager</div>
        <div class="text-[10px] text-slate-500">Virtualization Platform</div>
      </div>
    </div>
  </header>
  
  <!-- Tree Navigation -->
  <nav 
    bind:this={treeElement}
    class="flex-1 overflow-y-auto py-2"
    aria-label="Resource tree"
  >
    <ul role="tree">
      <!-- Datacenter -->
      <li class="select-none">
        <div class="mx-2 rounded-md {isActive('/') ? 'bg-[#e57035]/15 text-[#ff9a65]' : 'hover:bg-white/5'}">
          <a
            href="/"
            class="flex items-center gap-2 px-3 py-2 text-sm"
            aria-current={isActive('/') ? 'page' : undefined}
          >
            <Database size={16} class={isActive('/') ? 'text-[#e57035]' : 'text-slate-400'} />
            <span class="flex-1 truncate font-medium">Datacenter</span>
          </a>
        </div>
        
        <!-- Overview -->
        <ul class="mt-0.5">
          <li class="select-none">
            <div class="mx-2 rounded-md {isActive('/') ? 'bg-[#e57035]/15 text-[#ff9a65]' : 'hover:bg-white/5'}"
                 style="margin-left: 1.25rem;">
              <a
                href="/"
                class="flex items-center gap-2 px-3 py-2 text-sm"
                aria-current={isActive('/') ? 'page' : undefined}
              >
                <LayoutGrid size={16} class={isActive('/') ? 'text-[#e57035]' : 'text-slate-400'} />
                <span class="truncate">Overview</span>
              </a>
            </div>
          </li>
        </ul>
      </li>
      
      <!-- Nodes Folder -->
      <li class="select-none mt-2">
        <div class="mx-2 rounded-md hover:bg-white/5">
          <button
            type="button"
            class="flex items-center gap-2 w-full px-3 py-2 text-sm text-left"
            onclick={(e) => toggleNode('nodes', e)}
            aria-expanded={isExpanded('nodes')}
          >
            {#if isExpanded('nodes')}
              <ChevronDown size={14} class="text-slate-400" />
            {:else}
              <ChevronRight size={14} class="text-slate-400" />
            {/if}
            <FolderTree size={16} class="text-slate-400" />
            <span class="flex-1 truncate font-medium">Nodes</span>
          </button>
        </div>
        
        {#if isExpanded('nodes')}
          <ul class="mt-0.5" transition:slide={{ duration: 150 }}>
            {#each nodes as node}
              <li class="select-none">
                <!-- Node -->
                <div 
                  class="mx-2 rounded-md {isActive(`/nodes/${node.id}`) ? 'bg-[#e57035]/15 text-[#ff9a65]' : 'hover:bg-white/5'}"
                  style="margin-left: 1.25rem;"
                >
                  <button
                    type="button"
                    class="flex items-center gap-2 w-full px-3 py-2 text-sm text-left"
                    onclick={(e) => toggleNode(node.id, e)}
                    aria-expanded={isExpanded(node.id)}
                  >
                    {#if isExpanded(node.id)}
                      <ChevronDown size={14} class="text-slate-400" />
                    {:else}
                      <ChevronRight size={14} class="text-slate-400" />
                    {/if}
                    <Circle size={8} class={getStatusColor(node.status)} fill="currentColor" />
                    <span class="truncate font-medium">{node.name}</span>
                  </button>
                </div>
                
                {#if isExpanded(node.id)}
                  <ul class="mt-0.5" transition:slide={{ duration: 150 }}>
                    <!-- Virtual Machines -->
                    <li class="select-none">
                      <div 
                        class="mx-2 rounded-md {isActive(`/nodes/${node.id}/vms`) ? 'bg-[#e57035]/15 text-[#ff9a65]' : 'hover:bg-white/5'}"
                        style="margin-left: 2rem;"
                      >
                        <button
                          type="button"
                          class="flex items-center gap-2 w-full px-3 py-2 text-sm text-left"
                          onclick={(e) => toggleNode(`${node.id}-vms`, e)}
                          aria-expanded={isExpanded(`${node.id}-vms`)}
                        >
                          {#if isExpanded(`${node.id}-vms`)}
                            <ChevronDown size={14} class="text-slate-400" />
                          {:else}
                            <ChevronRight size={14} class="text-slate-400" />
                          {/if}
                          <Server size={16} class="text-slate-400" />
                          <span class="truncate">Virtual Machines</span>
                          {#if vms.length > 0}
                            <span class="ml-auto bg-[#e57035] text-white text-[10px] px-1.5 py-0.5 rounded min-w-[1.25rem] text-center">
                              {vms.length}
                            </span>
                          {/if}
                        </button>
                      </div>
                      
                      {#if isExpanded(`${node.id}-vms`)}
                        <ul class="mt-0.5" transition:slide={{ duration: 150 }}>
                          {#each vms as vm}
                            <li class="select-none">
                              <div 
                                class="mx-2 rounded-md {isActive(`/vms/${vm.id}`) ? 'bg-[#e57035]/15 text-[#ff9a65]' : 'hover:bg-white/5'}"
                                style="margin-left: 2.75rem;"
                              >
                                <a
                                  href={`/vms/${vm.id}`}
                                  class="flex items-center gap-2 px-3 py-2 text-sm"
                                  aria-current={isActive(`/vms/${vm.id}`) ? 'page' : undefined}
                                >
                                  <Circle size={6} class={getStatusColor(vm.actual_state)} fill="currentColor" />
                                  <span class="truncate">{vm.name}</span>
                                </a>
                              </div>
                            </li>
                          {/each}
                        </ul>
                      {/if}
                    </li>
                    
                    <!-- cell network -->
                    <li class="select-none">
                      <div 
                        class="mx-2 rounded-md {isActive(`/nodes/${node.id}/networks`) ? 'bg-[#e57035]/15 text-[#ff9a65]' : 'hover:bg-white/5'}"
                        style="margin-left: 2rem;"
                      >
                        <a
                          href={`/nodes/${node.id}/networks`}
                          class="flex items-center gap-2 px-3 py-2 text-sm"
                          aria-current={isActive(`/nodes/${node.id}/networks`) ? 'page' : undefined}
                        >
                          <span class="w-5"></span>
                          <Network size={16} class="text-slate-400" />
                          <span class="truncate">cell network</span>
                          {#if node.resources?.networks}
                            <span class="ml-auto text-xs text-slate-500">{node.resources.networks}</span>
                          {/if}
                        </a>
                      </div>
                    </li>
                    
                    <!-- cell storage -->
                    <li class="select-none">
                      <div 
                        class="mx-2 rounded-md {isActive(`/nodes/${node.id}/storage`) ? 'bg-[#e57035]/15 text-[#ff9a65]' : 'hover:bg-white/5'}"
                        style="margin-left: 2rem;"
                      >
                        <a
                          href={`/nodes/${node.id}/storage`}
                          class="flex items-center gap-2 px-3 py-2 text-sm"
                          aria-current={isActive(`/nodes/${node.id}/storage`) ? 'page' : undefined}
                        >
                          <span class="w-5"></span>
                          <HardDrive size={16} class="text-slate-400" />
                          <span class="truncate">cell storage</span>
                          {#if node.resources?.storagePools}
                            <span class="ml-auto text-xs text-slate-500">{node.resources.storagePools}</span>
                          {/if}
                        </a>
                      </div>
                    </li>
                  </ul>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </li>
    </ul>
  </nav>
  
  <!-- Footer -->
  <footer class="border-t border-[#1e1e28] p-3 bg-[#1e1e28]">
    <UserMenu userName="Administrator" userEmail="admin@chv.local" />
  </footer>
</aside>

<style>
  nav::-webkit-scrollbar {
    width: 6px;
  }
  nav::-webkit-scrollbar-track {
    background: transparent;
  }
  nav::-webkit-scrollbar-thumb {
    background: #334155;
    border-radius: 3px;
  }
  nav::-webkit-scrollbar-thumb:hover {
    background: #475569;
  }
  
  @media (max-width: 768px) {
    aside {
      display: none;
    }
  }
</style>
