<script>
  import { onMount } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import ToastContainer from '$lib/components/ToastContainer.svelte';
  import { getStoredToken } from '$lib/api/client';
  import '../app.css';

  // Public paths that don't require auth
  const publicPaths = ['/login', '/install'];

  onMount(() => {
    const currentPath = $page.url.pathname;
    const isPublicPath = publicPaths.some(path => currentPath.startsWith(path));
    
    if (!isPublicPath && !getStoredToken()) {
      goto('/login');
    }
  });
</script>

<!-- Global Toast Container -->
<ToastContainer />

{#if $page.url.pathname === '/login'}
  <slot />
{:else}
  <div class="console-shell">
    <Sidebar currentPath={$page.url.pathname} />

    <main class="relative flex flex-col min-w-0 bg-slate-50/50">
      <header class="sticky top-0 z-10 glass border-b border-slate-200/60 px-8 py-5 flex items-center justify-between">
        <div>
          <div class="flex items-center gap-2 text-[10px] font-bold uppercase tracking-[0.3em] text-indigo-500/80">
            Platform / Core
          </div>
          <div class="mt-0.5 text-2xl font-bold tracking-tight text-slate-900 heading">
            Cloud Hypervisor <span class="text-indigo-600">Virtualization</span>
          </div>
        </div>
        
        <div class="flex items-center gap-4">
          <div class="flex -space-x-2">
            <!-- Placeholder for multi-user/node cluster status -->
            <div class="w-8 h-8 rounded-full bg-indigo-50 border-2 border-white flex items-center justify-center text-[10px] font-bold text-indigo-600">N1</div>
            <div class="w-8 h-8 rounded-full bg-slate-50 border-2 border-white flex items-center justify-center text-[10px] font-bold text-slate-400">N2</div>
          </div>
        </div>
      </header>

      <div class="p-8 pb-20 max-w-[1600px]">
        {#key $page.url.pathname}
          <div in:fly={{ y: 8, duration: 300, delay: 300, easing: cubicOut }} out:fade={{ duration: 200 }}>
            <slot />
          </div>
        {/key}
      </div>
    </main>
  </div>
{/if}

<style>
  .heading {
    font-family: 'Outfit', sans-serif;
  }
</style>
