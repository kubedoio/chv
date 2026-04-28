<script lang="ts">
  let { open = $bindable(false), onSuccess }: { open?: boolean; onSuccess?: () => void } = $props();
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => { if (e.target === e.currentTarget) open = false; }} onkeydown={(e) => { if (e.key === 'Escape') open = false; }}>
    <div class="bg-white rounded-lg shadow-lg w-full max-w-2xl mx-4 p-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold">Register Cloud-init Script</h2>
        <button type="button" onclick={() => open = false} class="text-muted hover:text-ink" aria-label="Close modal">✕</button>
      </div>
      <p class="text-muted text-sm">Cloud-init editor placeholder.</p>
    </div>
  </div>
{/if}
