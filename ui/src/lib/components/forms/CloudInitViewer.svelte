<script lang="ts">
  import Modal from '$lib/components/modals/Modal.svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import type { CloudInitTemplate } from '$lib/api/types';
  import { Copy, Check } from 'lucide-svelte';

  interface Props {
    open?: boolean;
    template?: CloudInitTemplate | null;
  }

  let {
    open = $bindable(false),
    template = null
  }: Props = $props();

  const client = createAPIClient({ token: getStoredToken() ?? undefined });

  // Preview state
  let previewVariables = $state<Record<string, string>>({});
  let renderedContent = $state('');
  let showRendered = $state(false);
  let copied = $state(false);

  // Reset when template changes
  $effect(() => {
    if (template) {
      previewVariables = {};
      renderedContent = '';
      showRendered = false;
    }
  });

  async function renderPreview() {
    if (!template) return;

    try {
      const result = await client.renderCloudInitTemplate(template.id, {
        variables: previewVariables
      });
      renderedContent = result.rendered;
      showRendered = true;
    } catch (err) {
      toast.error('Failed to render template');
    }
  }

  function copyToClipboard() {
    const contentToCopy = showRendered ? renderedContent : (template?.content || '');
    navigator.clipboard.writeText(contentToCopy).then(() => {
      copied = true;
      setTimeout(() => copied = false, 2000);
      toast.success('Copied to clipboard');
    });
  }

  function highlightYAML(content: string): string {
    if (!content) return '';
    
    // Simple YAML syntax highlighting
    return content
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      // Comments
      .replace(/(#.*$)/gm, '<span class="text-neutral-500">$1</span>')
      // Keys
      .replace(/^(\s*)([a-zA-Z_][a-zA-Z0-9_]*)(:)/gm, '$1<span class="text-sky-400">$2</span>$3')
      // String values (basic)
      .replace(/(:\s*)'(.*?)'/g, '$1<span class="text-emerald-400">\'$2\'</span>')
      // Numbers
      .replace(/(:\s*)(\d+)/g, '$1<span class="text-amber-400">$2</span>')
      // Template variables
      .replace(/(\{\{.*?\}\})/g, '<span class="text-pink-400">$1</span>');
  }
</script>

<Modal bind:open title={template?.name || 'Cloud-init Template'} closeOnBackdrop={true} width="wide">
  {#if template}
    <div class="space-y-4">
      <!-- Description -->
      {#if template.description}
        <p class="text-sm text-muted">{template.description}</p>
      {/if}

      <!-- Variables -->
      {#if template.variables.length > 0}
        <div class="bg-chrome rounded-lg p-3">
          <h4 class="text-xs font-medium text-muted uppercase tracking-wide mb-2">Available Variables</h4>
          <div class="flex flex-wrap gap-2">
            {#each template.variables as varName}
              <code class="text-xs bg-white px-2 py-1 rounded border border-line text-primary">{`{{.${varName}}}`}</code>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Variable Inputs for Preview -->
      {#if template.variables.length > 0}
        <div class="border-t border-line pt-4">
          <h4 class="text-sm font-semibold text-ink mb-3">Preview with Variables</h4>
          <div class="grid grid-cols-2 gap-3">
            {#each template.variables as varName}
              <div>
                <label for={`preview-${varName}`} class="block text-xs text-muted mb-1">{varName}</label>
                <input
                  id={`preview-${varName}`}
                  type="text"
                  value={previewVariables[varName] || ''}
                  oninput={(e) => {
                    previewVariables = { ...previewVariables, [varName]: e.currentTarget.value };
                  }}
                  placeholder={`Enter ${varName}...`}
                  class="w-full h-8 rounded border border-[#CCCCCC] bg-white px-2 py-1 text-sm"
                />
              </div>
            {/each}
          </div>
          <button
            type="button"
            onclick={renderPreview}
            class="mt-3 text-sm text-primary hover:text-primary/80 font-medium"
          >
            Render Preview
          </button>
        </div>
      {/if}

      <!-- Content Display -->
      <div class="border-t border-line pt-4">
        <div class="flex items-center justify-between mb-2">
          <h4 class="text-sm font-semibold text-ink">
            {showRendered ? 'Rendered Output' : 'Template Content'}
          </h4>
          <div class="flex items-center gap-2">
            {#if template.variables.length > 0}
              <button
                type="button"
                onclick={() => showRendered = !showRendered}
                class="text-xs text-muted hover:text-ink"
              >
                {showRendered ? 'Show Source' : 'Show Rendered'}
              </button>
              <span class="text-line">|</span>
            {/if}
            <button
              type="button"
              onclick={copyToClipboard}
              class="flex items-center gap-1 text-xs text-muted hover:text-ink"
            >
              {#if copied}
                <Check size={12} class="text-success" />
                Copied!
              {:else}
                <Copy size={12} />
                Copy
              {/if}
            </button>
          </div>
        </div>

        <div class="rounded-lg bg-neutral-900 overflow-auto max-h-96">
          <pre class="p-4 text-sm font-mono whitespace-pre-wrap"><code>{@html highlightYAML(showRendered ? renderedContent : template.content)}</code></pre>
        </div>
      </div>
    </div>
  {:else}
    <div class="text-center py-8 text-muted">
      No template selected
    </div>
  {/if}

  {#snippet footer()}
    <button
      type="button"
      onclick={() => open = false}
      class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors"
    >
      Close
    </button>
  {/snippet}
</Modal>
