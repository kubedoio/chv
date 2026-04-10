<script lang="ts">
  import { Home, ChevronRight } from 'lucide-svelte';
  
  interface BreadcrumbItem {
    label: string;
    href?: string;
  }
  
  interface Props {
    items: BreadcrumbItem[];
  }
  
  let { items }: Props = $props();
</script>

<nav aria-label="Breadcrumb" class="flex items-center min-w-0">
  <ol class="flex items-center gap-1.5 min-w-0">
    {#each items as item, index}
      <li class="flex items-center gap-1.5 {index === items.length - 1 ? 'min-w-0' : 'shrink-0'}">
        {#if index === 0}
          <!-- Home icon for first item -->
          {#if item.href}
            <a 
              href={item.href}
              class="p-1 rounded hover:bg-white/10 transition-colors duration-150 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#e57035] focus-visible:outline-offset-2"
              aria-label="Home"
            >
              <Home size={16} class="text-slate-400" />
            </a>
          {:else}
            <span class="p-1">
              <Home size={16} class="text-slate-400" />
            </span>
          {/if}
        {:else}
          <!-- Separator -->
          <ChevronRight size={14} class="text-slate-600 shrink-0" aria-hidden="true" />
          
          <!-- Breadcrumb item -->
          {#if index === items.length - 1}
            <!-- Current page - not clickable -->
            <span 
              class="text-[#e57035] font-medium truncate text-sm"
              aria-current="page"
            >
              {item.label}
            </span>
          {:else}
            <!-- Link to parent page -->
            <a 
              href={item.href || '#'}
              class="text-slate-400 hover:text-slate-200 transition-colors duration-150 text-sm whitespace-nowrap focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#e57035] focus-visible:outline-offset-2"
            >
              {item.label}
            </a>
          {/if}
        {/if}
      </li>
    {/each}
  </ol>
</nav>
