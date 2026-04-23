<script lang="ts">
  let { open = $bindable(false), onSuccess }: { open?: boolean; onSuccess?: () => void } = $props();
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" role="dialog" aria-modal="true" onclick={(e) => { if (e.target === e.currentTarget) open = false; }}>
    <div class="bg-white rounded-lg shadow-lg w-full max-w-2xl mx-4 p-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold">Register Cloud-init Script</h2>
        <button onclick={() => open = false} class="text-muted hover:text-ink">✕</button>
      </div>
      <p class="text-muted text-sm">Cloud-init editor placeholder.</p>
    </div>
  </div>
{/if}
