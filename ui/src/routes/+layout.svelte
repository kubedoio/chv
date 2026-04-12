<script>
  import { onMount, onDestroy } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { Wifi, WifiOff, Bell } from 'lucide-svelte';
  import TreeNavigation from '$lib/components/TreeNavigation.svelte';
  import Breadcrumbs from '$lib/components/Breadcrumbs.svelte';
  import ToastContainer from '$lib/components/ToastContainer.svelte';
  import SearchModal from '$lib/components/SearchModal.svelte';
  import KeyboardShortcutsHelp from '$lib/components/KeyboardShortcutsHelp.svelte';
  import QuickActions from '$lib/components/QuickActions.svelte';
  import { getStoredToken, createAPIClient } from '$lib/api/client';
  import { getDefaultNode } from '$lib/api/nodes';
  import { 
    initKeyboardShortcuts, 
    registerShortcuts,
    createGlobalShortcuts,
    setActiveContext
  } from '$lib/stores/keyboard.svelte.ts';
  import { buildSearchIndex } from '$lib/stores/search.svelte.ts';
  import '../app.css';

  // Public paths that don't require auth
  const publicPaths = ['/login', '/install'];

  // For now, use a single default node (until backend supports multi-node)
  let nodes = $state([getDefaultNode()]);
  
  // Connection status
  let isOnline = $state(true);
  let connectionCheckInterval;
  
  // User info (placeholder until we have user API)
  let userInfo = $state({
    name: 'Administrator',
    email: 'admin@chv.local'
  });
  
  // Modal states
  let searchOpen = $state(false);
  let quickActionsOpen = $state(false);
  
  // Keyboard cleanup function
  let keyboardCleanup = $state(() => {});

  // Generate breadcrumbs based on current path
  function generateBreadcrumbs(path) {
    const items = [{ label: 'Datacenter', href: '/' }];
    
    if (path === '/') {
      items.push({ label: 'Overview' });
      return items;
    }
    
    // Handle node-specific routes
    const nodeMatch = path.match(/\/nodes\/([^\/]+)(?:\/(.+))?/);
    if (nodeMatch) {
      const nodeId = nodeMatch[1];
      const subPath = nodeMatch[2];
      
      // Find node name
      const node = nodes.find(n => n.id === nodeId);
      items.push({ 
        label: node?.name || nodeId, 
        href: `/nodes/${nodeId}` 
      });
      
      if (subPath) {
        const resourceMap = {
          'vms': 'Virtual Machines',
          'images': 'Images',
          'storage': 'Storage',
          'networks': 'Networks'
        };
        items.push({ label: resourceMap[subPath] || subPath });
      }
      return items;
    }
    
    // Handle global routes
    const resourceMap = {
      '/images': 'Images',
      '/storage': 'Storage',
      '/networks': 'Networks',
      '/settings': 'Settings',
      '/profile': 'Profile'
    };
    
    if (resourceMap[path]) {
      items.push({ label: resourceMap[path] });
    } else {
      // Fallback - use last path segment
      const segments = path.split('/').filter(Boolean);
      if (segments.length > 0) {
        items.push({ 
          label: segments[segments.length - 1].charAt(0).toUpperCase() + 
                 segments[segments.length - 1].slice(1)
        });
      }
    }
    
    return items;
  }
  
  let breadcrumbs = $derived(generateBreadcrumbs($page.url.pathname));
  
  // Determine active context for keyboard shortcuts
  let activeContext = $derived(getContextFromPath($page.url.pathname));
  
  function getContextFromPath(path) {
    if (path === '/vms') return 'vms';
    if (path.startsWith('/vms/')) return 'vm-detail';
    return 'global';
  }
  
  // Update context when path changes
  $effect(() => {
    setActiveContext(activeContext);
  });

  onMount(() => {
    const currentPath = $page.url.pathname;
    const isPublicPath = publicPaths.some(path => currentPath.startsWith(path));
    
    if (!isPublicPath && !getStoredToken()) {
      goto('/login');
    }
    
    // Check connection status periodically
    checkConnection();
    connectionCheckInterval = setInterval(checkConnection, 30000);
    
    // Listen for online/offline events
    window.addEventListener('online', () => isOnline = true);
    window.addEventListener('offline', () => isOnline = false);
    
    // Initialize keyboard shortcuts
    const cleanup = initKeyboardShortcuts();
    keyboardCleanup = cleanup || (() => {});
    
    // Register global shortcuts
    const unregisterGlobals = registerShortcuts(createGlobalShortcuts(
      () => searchOpen = true,
      () => quickActionsOpen = true
    ));
    
    // Load search index data
    loadSearchIndex();
    
    // Fetch actual nodes from API
    updateNodeResources();
    
    return () => {
      clearInterval(connectionCheckInterval);
      window.removeEventListener('online', () => isOnline = true);
      window.removeEventListener('offline', () => isOnline = false);
      cleanup?.();
      unregisterGlobals();
    };
  });
  
  async function checkConnection() {
    try {
      // Try to make a simple API call to check if backend is reachable
      const client = createAPIClient();
      await client.validateLogin();
      isOnline = true;
    } catch (err) {
      // If we get a 401, we're still online (just need to login)
      if (err?.status === 401) {
        isOnline = true;
      } else {
        isOnline = navigator.onLine;
      }
    }
  }
  
  async function updateNodeResources() {
    const token = getStoredToken();
    if (!token) {
      nodes = [getDefaultNode()];
      return;
    }
    
    try {
      const client = createAPIClient({ token });
      const apiNodes = await client.listNodes();
      if (apiNodes && apiNodes.length > 0) {
        nodes = apiNodes;
      } else {
        nodes = [getDefaultNode()];
      }
    } catch (err) {
      console.warn('Failed to fetch nodes:', err);
      nodes = [getDefaultNode()];
    }
  }
  
  async function loadSearchIndex() {
    const token = getStoredToken();
    if (!token) return;
    
    try {
      const client = createAPIClient({ token });
      const [vms, images, networks, storagePools] = await Promise.all([
        client.listVMs().catch(() => []),
        client.listImages().catch(() => []),
        client.listNetworks().catch(() => []),
        client.listStoragePools().catch(() => [])
      ]);
      
      buildSearchIndex({
        vms: vms ?? [],
        images: images ?? [],
        networks: networks ?? [],
        storagePools: storagePools ?? []
      });
    } catch (err) {
      console.error('Failed to load search index:', err);
    }
  }
</script>

<!-- Global Components -->
<ToastContainer />
<SearchModal bind:open={searchOpen} />
<KeyboardShortcutsHelp />
<QuickActions bind:open={quickActionsOpen} />

{#if $page.url.pathname === '/login'}
  <slot />
{:else}
  <div class="proxmox-layout">
    <!-- Left Sidebar with Tree Navigation -->
    <TreeNavigation {nodes} />

    <!-- Main Content Area -->
    <main class="flex-1 flex flex-col min-w-0 bg-[#f5f5f5] overflow-hidden">
      <!-- Top Navigation Bar -->
      <header class="h-12 bg-[#2d2d3a] border-b border-[#3a3a4a] flex items-center px-4 justify-between shrink-0 shadow-sm">
        <!-- Left side: Breadcrumbs -->
        <div class="flex items-center gap-4 min-w-0 flex-1">
          <Breadcrumbs items={breadcrumbs} />
        </div>
        
        <!-- Right side: Status indicators -->
        <div class="flex items-center gap-3 shrink-0">
          <!-- Search Button -->
          <button
            onclick={() => searchOpen = true}
            class="hidden sm:flex items-center gap-2 px-3 py-1.5 text-xs text-slate-300 hover:text-white hover:bg-[#3a3a4a] rounded-md transition-colors"
            title="Search (Ctrl+K or Cmd+K)"
          >
            <span class="hidden lg:inline">Search</span>
            <kbd class="px-1.5 py-0.5 bg-[#1e1e28] rounded text-[10px] text-slate-400 border border-[#3a3a4a]">
              Ctrl K
            </kbd>
          </button>
          
          <!-- Connection Status -->
          <div 
            class="flex items-center gap-2 px-3 py-1.5 rounded-md text-xs border transition-colors duration-200 {isOnline 
              ? 'bg-green-500/10 border-green-500/30 text-green-400' 
              : 'bg-red-500/10 border-red-500/30 text-red-400'
            }"
            title={isOnline ? 'Connected to server' : 'Connection lost'}
          >
            {#if isOnline}
              <Wifi size={14} class="animate-pulse" />
              <span class="hidden sm:inline">Connected</span>
            {:else}
              <WifiOff size={14} />
              <span class="hidden sm:inline">Offline</span>
            {/if}
          </div>
          
          <!-- Node Status Indicator -->
          <div class="flex items-center gap-2 px-3 py-1.5 bg-[#1e1e28] rounded-md text-xs border border-[#3a3a4a]">
            <span class="w-2 h-2 rounded-full bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.5)]"></span>
            <span class="text-slate-300">{nodes.length} node{nodes.length !== 1 ? 's' : ''}</span>
          </div>
          
          <!-- Version Badge -->
          <span class="text-[10px] text-slate-400 px-2 py-1 bg-[#1e1e28] rounded-md border border-[#3a3a4a]">
            v0.1.0-alpha
          </span>
        </div>
      </header>

      <!-- Content Area -->
      <div class="flex-1 overflow-auto p-6">
        {#key $page.url.pathname}
          <div 
            in:fly={{ y: 8, duration: 200, delay: 100, easing: cubicOut }} 
            out:fade={{ duration: 150 }}
            class="max-w-[1600px] mx-auto"
          >
            <slot />
          </div>
        {/key}
      </div>
    </main>
  </div>
{/if}

<style>
  .proxmox-layout {
    min-height: 100vh;
    display: grid;
    grid-template-columns: 256px minmax(0, 1fr);
    background: #1e1e2e;
  }
  
  @media (max-width: 768px) {
    .proxmox-layout {
      grid-template-columns: 1fr;
    }
  }
</style>
