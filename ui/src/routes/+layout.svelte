<script lang="ts">
  import { page } from '$app/stores';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import '../app.css';
</script>

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

