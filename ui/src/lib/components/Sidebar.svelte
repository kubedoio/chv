<script lang="ts">
  export let currentPath = '/';

  const items = [
    { href: '/', label: 'Overview' },
    { href: '/install', label: 'Install' },
    { href: '/networks', label: 'Networks' },
    { href: '/storage', label: 'Storage' },
    { href: '/images', label: 'Images' },
    { href: '/vms', label: 'Virtual Machines' },
    { href: '/operations', label: 'Operations' },
    { href: '/settings', label: 'Settings' }
  ];
</script>

<aside class="border-r border-line bg-chrome">
  <div class="border-b border-line px-5 py-4">
    <div class="text-[11px] uppercase tracking-[0.2em] text-muted">CHV</div>
    <div class="mt-2 text-lg font-semibold text-ink">Operator Console</div>
    <div class="mt-1 text-sm text-muted">Cloud Hypervisor MVP-1</div>
  </div>

  <nav class="p-3">
    {#each items as item}
      <a
        class={`mb-1 block border px-3 py-2 text-sm no-underline transition ${
          currentPath === item.href ? 'border-primary bg-selected text-ink' : 'border-transparent text-muted hover:border-line hover:bg-white hover:text-ink'
        }`}
        href={item.href}
      >
        {item.label}
      </a>
    {/each}
  </nav>
</aside>

