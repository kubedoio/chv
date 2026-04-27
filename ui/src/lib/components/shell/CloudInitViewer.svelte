<script lang="ts">
  import type { CloudInitTemplate } from '$lib/api/types';
  
  let { open = $bindable(false), template }: { open?: boolean; template?: CloudInitTemplate | null } = $props();
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" role="dialog" aria-modal="true" onclick={(e) => { if (e.target === e.currentTarget) open = false; }}>
    <div class="bg-white rounded-lg shadow-lg w-full max-w-2xl mx-4 p-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold">{template?.name ?? 'Cloud-init Config'}</h2>
        <button onclick={() => open = false} class="text-muted hover:text-ink" aria-label="Close modal">✕</button>
      </div>
      <pre class="bg-neutral-50 p-4 rounded text-sm overflow-auto max-h-[60vh]">{template?.content ?? 'No config available'}</pre>
    </div>
  </div>
{/if}
