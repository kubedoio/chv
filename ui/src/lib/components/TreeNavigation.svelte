<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { slide } from 'svelte/transition';
  import { 
    Server, 
    Monitor,
    HardDrive, 
    Network, 
    Image as ImageIcon, 
    Activity, 
    Settings,
    ChevronRight,
    ChevronDown,
    Database,
    FolderTree,
    Circle,
    LayoutGrid,
    LogOut,
    BarChart3
  } from 'lucide-svelte';
  import { createAPIClient, clearToken } from '$lib/api/client';
  import type { TreeNode } from '$lib/api/nodes';
  import UserMenu from './UserMenu.svelte';
  import VisuallyHidden from './VisuallyHidden.svelte';
  
  // Props
  interface Props {
    nodes?: import('$lib/api/nodes').Node[];
  }
  
  let { nodes = [] }: Props = $props();
  
  // Generate tree from nodes
  function generateTree(nodes: import('$lib/api/nodes').Node[]): TreeNode[] {
    const nodeChildren: TreeNode[] = nodes.map(node => ({
      id: node.id,
      type: 'node',
      label: node.name,
      status: node.status,
      expanded: true,
      href: `/nodes/${node.id}`,
      children: [
        {
          id: `${node.id}-vms`,
          type: 'resource',
          label: 'Virtual Machines',
          icon: 'server',
          href: `/nodes/${node.id}/vms`,
          badge: node.resources?.vms ?? 0
        },
        {
          id: `${node.id}-images`,
          type: 'resource',
          label: 'Images',
          icon: 'image',
          href: `/nodes/${node.id}/images`,
          badge: node.resources?.images ?? 0
        },
        {
          id: `${node.id}-storage`,
          type: 'resource',
          label: 'Storage',
          icon: 'hardDrive',
          href: `/nodes/${node.id}/storage`,
          badge: node.resources?.storagePools ?? 0
        },
        {
          id: `${node.id}-networks`,
          type: 'resource',
          label: 'Networks',
          icon: 'network',
          href: `/nodes/${node.id}/networks`,
          badge: node.resources?.networks ?? 0
        }
      ]
    }));

    return [
      {
        id: 'datacenter',
        type: 'datacenter',
        label: 'Datacenter',
        expanded: true,
        icon: 'datacenter',
        href: '/',
        children: [
          {
            id: 'overview',
            type: 'resource',
            label: 'Overview',
            icon: 'layout',
            href: '/'
          },
          ...(nodeChildren.length > 0 ? nodeChildren : []),
          {
            id: 'global-images',
            type: 'resource',
            label: 'Images',
            icon: 'image',
            href: '/images'
          },
          {
            id: 'global-storage',
            type: 'resource',
            label: 'Storage',
            icon: 'hardDrive',
            href: '/storage'
          },
          {
            id: 'global-networks',
            type: 'resource',
            label: 'Networks',
            icon: 'network',
            href: '/networks'
          },
          {
            id: 'global-metrics',
            type: 'resource',
            label: 'Metrics',
            icon: 'metrics',
            href: '/metrics'
          }
        ]
      }
    ];
  }
  
  let treeNodes = $derived(generateTree(nodes));
  let expandedNodes = $state<Set<string>>(new Set(['datacenter']));
  let currentPath = $derived($page.url.pathname);
  
  // Track focused node for keyboard navigation
  let focusedNodeId = $state<string | null>(null);
  let treeElement = $state<HTMLElement | null>(null);
  
  const client = createAPIClient();
  let newEvents = $state(0);
  let pollInterval: ReturnType<typeof setInterval>;
  
  onMount(() => {
    checkNewEvents();
    pollInterval = setInterval(checkNewEvents, 30000);
    return () => clearInterval(pollInterval);
  });
  
  async function checkNewEvents() {
    try {
      const events = await client.listEvents();
      const lastCheck = new Date(Date.now() - 30000);
      newEvents = events.filter(e => new Date(e.timestamp) > lastCheck).length;
    } catch (err) {
      console.error('Failed to check events:', err);
    }
  }
  
  function toggleNode(node: TreeNode, event?: Event) {
    event?.preventDefault();
    event?.stopPropagation();
    
    if (expandedNodes.has(node.id)) {
      expandedNodes.delete(node.id);
    } else {
      expandedNodes.add(node.id);
    }
    expandedNodes = expandedNodes;
  }
  
  function isExpanded(node: TreeNode): boolean {
    return node.expanded || expandedNodes.has(node.id);
  }
  
  function isActive(href: string): boolean {
    if (!href) return false;
    if (href === '/') return currentPath === '/';
    return currentPath.startsWith(href);
  }
  
  function isNodeActive(node: TreeNode): boolean {
    if (!node.href) return false;
    return isActive(node.href);
  }
  
  function getIcon(iconName: string | undefined) {
    switch (iconName) {
      case 'server': return Server;
      case 'image': return ImageIcon;
      case 'hardDrive': return HardDrive;
      case 'network': return Network;
      case 'activity': return Activity;
      case 'settings': return Settings;
      case 'datacenter': return Database;
      case 'folder': return FolderTree;
      case 'layout': return LayoutGrid;
      case 'metrics': return BarChart3;
      default: return Circle;
    }
  }
  
  function getStatusColor(status: string | undefined): string {
    switch (status) {
      case 'online': return 'text-green-500';
      case 'offline': return 'text-red-500';
      case 'warning': return 'text-yellow-500';
      case 'maintenance': return 'text-orange-500';
      default: return 'text-slate-400';
    }
  }
  
  function getStatusLabel(status: string | undefined): string {
    switch (status) {
      case 'online': return 'Online';
      case 'offline': return 'Offline';
      case 'warning': return 'Warning';
      case 'maintenance': return 'Maintenance';
      default: return 'Unknown';
    }
  }
  
  // Flatten tree for keyboard navigation
  function getFlattenedNodes(nodes: TreeNode[], level = 0): Array<{ node: TreeNode; level: number; hasChildren: boolean }> {
    const result: Array<{ node: TreeNode; level: number; hasChildren: boolean }> = [];
    
    for (const node of nodes) {
      const nodeHasChildren = !!(node.children && node.children.length > 0);
      result.push({ node, level, hasChildren: nodeHasChildren });
      if (isExpanded(node) && node.children) {
        result.push(...getFlattenedNodes(node.children, level + 1));
      }
    }
    
    return result;
  }
  
  let flattenedNodes = $derived(getFlattenedNodes(treeNodes));
  
  // Keyboard navigation handler
  async function handleTreeKeyDown(event: KeyboardEvent, node: TreeNode, level: number, hasChildren: boolean) {
    const flatList = flattenedNodes;
    const currentIndex = flatList.findIndex(item => item.node.id === node.id);
    const currentItem = flatList[currentIndex];
    
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        const nextIndex = currentIndex + 1;
        if (nextIndex < flatList.length) {
          focusedNodeId = flatList[nextIndex].node.id;
          await tick();
          document.querySelector<HTMLElement>(`[data-tree-node="${focusedNodeId}"]`)?.focus();
        }
        break;
      
      case 'ArrowUp':
        event.preventDefault();
        const prevIndex = currentIndex - 1;
        if (prevIndex >= 0) {
          focusedNodeId = flatList[prevIndex].node.id;
          await tick();
          document.querySelector<HTMLElement>(`[data-tree-node="${focusedNodeId}"]`)?.focus();
        }
        break;
      
      case 'ArrowRight':
        event.preventDefault();
        if (hasChildren) {
          if (!isExpanded(node)) {
            toggleNode(node);
          } else {
            // Move to first child
            const nextItem = flatList[currentIndex + 1];
            if (nextItem && nextItem.level > level) {
              focusedNodeId = nextItem.node.id;
              await tick();
              document.querySelector<HTMLElement>(`[data-tree-node="${focusedNodeId}"]`)?.focus();
            }
          }
        }
        break;
      
      case 'ArrowLeft':
        event.preventDefault();
        if (hasChildren && isExpanded(node)) {
          toggleNode(node);
        } else if (level > 0) {
          // Move to parent
          let parentIndex = currentIndex - 1;
          while (parentIndex >= 0 && flatList[parentIndex].level >= level) {
            parentIndex--;
          }
          if (parentIndex >= 0) {
            focusedNodeId = flatList[parentIndex].node.id;
            await tick();
            document.querySelector<HTMLElement>(`[data-tree-node="${focusedNodeId}"]`)?.focus();
          }
        }
        break;
      
      case 'Enter':
      case ' ':
        event.preventDefault();
        if (node.href) {
          goto(node.href);
        }
        break;
      
      case 'Home':
        event.preventDefault();
        if (flatList.length > 0) {
          focusedNodeId = flatList[0].node.id;
          await tick();
          document.querySelector<HTMLElement>(`[data-tree-node="${focusedNodeId}"]`)?.focus();
        }
        break;
      
      case 'End':
        event.preventDefault();
        if (flatList.length > 0) {
          focusedNodeId = flatList[flatList.length - 1].node.id;
          await tick();
          document.querySelector<HTMLElement>(`[data-tree-node="${focusedNodeId}"]`)?.focus();
        }
        break;
    }
  }
  
  function handleNodeFocus(nodeId: string) {
    focusedNodeId = nodeId;
  }

  function handleLogout() {
    clearToken();
    goto('/login');
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
      <div class="w-8 h-8 rounded bg-gradient-to-br from-[#e57035] to-[#d14a28] flex items-center justify-center shadow-lg shadow-orange-900/20">
        <Database class="text-white" size={18} aria-hidden="true" />
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
    <ul role="tree" aria-label="Navigation tree">
      {#each treeNodes as node}
        {@render treeNodeItem(node, 0)}
      {/each}
    </ul>
  </nav>
  
  <!-- Footer with User Menu -->
  <footer class="border-t border-[#1e1e28] p-3 bg-[#1e1e28]">
    <UserMenu userName="Administrator" userEmail="admin@chv.local" />
  </footer>
</aside>

<!-- Tree Node Snippet -->
{#snippet treeNodeItem(node: TreeNode, level: number)}
  {@const expanded = isExpanded(node)}
  {@const active = isNodeActive(node)}
  {@const hasChildren = node.children && node.children.length > 0}
  {@const IconComponent = getIcon(node.icon)}
  {@const paddingLeft = `${0.5 + level * 0.75}rem`}
  
  <li class="select-none" role="none">
    <!-- Node Item -->
    <div
      class="group flex items-center relative mx-2 rounded-md transition-all duration-150 {active 
        ? 'bg-[#e57035]/15 text-[#ff9a65]' 
        : 'hover:bg-white/5 hover:text-slate-100'
      }"
      style="margin-left: {paddingLeft}; margin-right: 0.5rem;"
    >
      <!-- Active indicator bar -->
      {#if active}
        <div 
          class="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-6 bg-[#e57035] rounded-full"
          aria-hidden="true"
        ></div>
      {/if}
      
      <a
        data-tree-node={node.id}
        href={node.href || '#'}
        class="flex items-center gap-2 flex-1 px-3 py-2 text-sm focus-visible:outline-none"
        style="padding-left: calc(0.5rem + {active ? '2px' : '0'});"
        role="treeitem"
        aria-expanded={hasChildren ? expanded : undefined}
        aria-selected={active}
        aria-current={active ? 'page' : undefined}
        tabindex={focusedNodeId === node.id || (!focusedNodeId && active) ? 0 : -1}
        onfocus={() => handleNodeFocus(node.id)}
        onkeydown={(e) => handleTreeKeyDown(e, node, level, !!hasChildren)}
      >
        <!-- Expand/Collapse Button -->
        {#if hasChildren}
          <button 
            type="button"
            class="w-5 h-5 flex items-center justify-center rounded hover:bg-white/10 transition-colors duration-150 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#e57035] focus-visible:outline-offset-0"
            onclick={(e) => toggleNode(node, e)}
            tabindex="-1"
            aria-label={expanded ? `Collapse ${node.label}` : `Expand ${node.label}`}
            aria-expanded={expanded}
          >
            {#if expanded}
              <ChevronDown size={14} class="text-slate-400 transition-transform duration-150" aria-hidden="true" />
            {:else}
              <ChevronRight size={14} class="text-slate-400 transition-transform duration-150" aria-hidden="true" />
            {/if}
          </button>
        {:else}
          <span class="w-5" aria-hidden="true"></span>
        {/if}
        
        <!-- Icon -->
        <span class="flex items-center justify-center w-5 shrink-0">
          {#if node.type === 'node'}
            <Circle 
              size={10} 
              class="{getStatusColor(node.status)} {active ? 'animate-pulse' : ''}" 
              fill="currentColor" 
              aria-hidden="true" 
            />
            <VisuallyHidden>{getStatusLabel(node.status)}</VisuallyHidden>
          {:else}
            <IconComponent size={16} class={active ? 'text-orange-400' : 'text-slate-400'} aria-hidden="true" />
          {/if}
        </span>
        
        <!-- Label -->
        <span class="flex-1 truncate font-medium">{node.label}</span>
        
        <!-- Badge -->
        {#if node.badge !== undefined && node.badge > 0}
          <span 
            class="bg-[#e57035] text-white text-[10px] px-1.5 py-0.5 rounded min-w-[1.25rem] text-center font-semibold shadow-sm ml-2 shrink-0"
            aria-label="{node.badge} items"
          >
            {node.badge}
          </span>
        {/if}
      </a>
    </div>
    
    <!-- Children -->
    {#if hasChildren && expanded}
      <ul 
        class="mt-0.5 overflow-hidden"
        role="group"
        aria-label="{node.label} children"
        transition:slide={{ duration: 200 }}
      >
        {#each node.children! as child}
          {@render treeNodeItem(child, level + 1)}
        {/each}
      </ul>
    {/if}
  </li>
{/snippet}

<style>
  /* Custom scrollbar */
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
  
  /* Focus visible within the tree */
  [role="treeitem"]:focus-visible {
    outline: 2px solid #e57035;
    outline-offset: -2px;
    border-radius: 6px;
  }

  /* Hide on mobile - mobile nav handles this */
  @media (max-width: 768px) {
    aside {
      display: none;
    }
  }
</style>
