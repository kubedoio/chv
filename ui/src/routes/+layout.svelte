<script lang="ts">
  import { onMount } from 'svelte';
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

    <main class="min-w-0">
      <header class="border-b border-line bg-chrome px-6 py-4">
        <div class="text-[11px] uppercase tracking-[0.18em] text-muted">CHV</div>
        <div class="mt-1 text-xl font-semibold text-ink">Cloud Hypervisor Virtualization</div>
      </header>

      <div class="p-6">
        <slot />
      </div>
    </main>
  </div>
{/if}
