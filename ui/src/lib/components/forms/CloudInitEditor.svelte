<script lang="ts">
  import Modal from '$lib/components/modals/Modal.svelte';
  import FormField from '$lib/components/forms/FormField.svelte';
  import Input from '$lib/components/Input.svelte';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';
  import { Copy, Check, Eye, EyeOff, Variable, FileCode } from 'lucide-svelte';

  interface Props {
    open?: boolean;
    initialContent?: string;
    initialName?: string;
    initialDescription?: string;
    onSuccess?: () => void;
  }

  let {
    open = $bindable(false),
    initialContent = '',
    initialName = '',
    initialDescription = '',
    onSuccess
  }: Props = $props();

  const client = createAPIClient({ token: getStoredToken() ?? undefined });

  // Form state
  let name = $state(initialName);
  let description = $state(initialDescription);
  let content = $state(initialContent || '#cloud-config\n');
  let variables = $state<string[]>([]);
  
  // Preview state
  let previewVariables = $state<Record<string, string>>({});
  let renderedPreview = $state('');
  let showPreview = $state(false);
  let showVariableHelper = $state(true);
  
  // UI state
  let copied = $state(false);
  let submitting = $state(false);
  let formError = $state('');
  let nameError = $state('');
  let contentError = $state('');

  const nameRegex = /^[a-zA-Z0-9\s-_]+$/;

  // Extract variables whenever content changes
  $effect(() => {
    variables = extractVariables(content);
  });

  // Reset form when modal opens with new initial values
  $effect(() => {
    if (open) {
      name = initialName;
      description = initialDescription;
      content = initialContent || '#cloud-config\n';
      previewVariables = {};
      renderedPreview = '';
      formError = '';
      nameError = '';
      contentError = '';
    }
  });

  function extractVariables(content: string): string[] {
    const vars: string[] = [];
    const seen = new Set<string>();
    const regex = /\{\{\s*\.([A-Za-z][A-Za-z0-9_]*)\s*\}\}/g;
    let match;
    while ((match = regex.exec(content)) !== null) {
      if (!seen.has(match[1])) {
        seen.add(match[1]);
        vars.push(match[1]);
      }
    }
    return vars;
  }

  function validateName(): boolean {
    if (!name.trim()) {
      nameError = 'Name is required';
      return false;
    }
    if (!nameRegex.test(name)) {
      nameError = 'Name can only contain letters, numbers, spaces, hyphens, and underscores';
      return false;
    }
    if (name.length > 50) {
      nameError = 'Name must be 50 characters or less';
      return false;
    }
    nameError = '';
    return true;
  }

  function validateContent(): boolean {
    if (!content.trim()) {
      contentError = 'Content is required';
      return false;
    }
    if (!content.includes('#cloud-config')) {
      contentError = 'Content must include #cloud-config header';
      return false;
    }
    contentError = '';
    return true;
  }

  async function updatePreview() {
    if (!validateContent()) {
      renderedPreview = '';
      return;
    }

    try {
      // Create a temporary template to render
      const tempTemplate = {
        content,
        variables
      };
      
      // Simple client-side rendering for preview
      let rendered = content;
      for (const [key, value] of Object.entries(previewVariables)) {
        const regex = new RegExp(`\\{\\{\\s*\\.${key}\\s*\\}\\}`, 'g');
        rendered = rendered.replace(regex, value);
      }
      renderedPreview = rendered;
    } catch (e) {
      renderedPreview = '# Error rendering preview';
    }
  }

  function copyToClipboard() {
    navigator.clipboard.writeText(content).then(() => {
      copied = true;
      setTimeout(() => copied = false, 2000);
      toast.success('Copied to clipboard');
    });
  }

  function insertVariable(varName: string) {
    const textarea = document.getElementById('cloudinit-content') as HTMLTextAreaElement;
    if (!textarea) return;
    
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const before = content.substring(0, start);
    const after = content.substring(end);
    const insertion = `{{.${varName}}}`;
    
    content = before + insertion + after;
    
    // Set cursor position after insertion
    setTimeout(() => {
      textarea.selectionStart = textarea.selectionEnd = start + insertion.length;
      textarea.focus();
    }, 0);
  }

  function insertSnippet(snippet: string) {
    const textarea = document.getElementById('cloudinit-content') as HTMLTextAreaElement;
    if (!textarea) return;
    
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const before = content.substring(0, start);
    const after = content.substring(end);
    
    content = before + snippet + after;
    
    // Set cursor position after insertion
    setTimeout(() => {
      textarea.selectionStart = textarea.selectionEnd = start + snippet.length;
      textarea.focus();
    }, 0);
  }

  async function handleSubmit() {
    const isNameValid = validateName();
    const isContentValid = validateContent();
    
    if (!isNameValid || !isContentValid) {
      return;
    }

    submitting = true;
    formError = '';

    try {
      const template = await client.createCloudInitTemplate({
        name: name.trim(),
        description: description.trim() || undefined,
        content: content.trim()
      });

      toast.success(`Template "${template.name}" created successfully`);
      open = false;
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to create template';
      formError = message;
      toast.error(message);
    } finally {
      submitting = false;
    }
  }

  function highlightYAML(content: string): string {
    if (!content) return '';
    
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

  const commonSnippets = [
    { name: 'User', snippet: 'users:\n  - name: {{.Username}}\n    sudo: ALL=(ALL) NOPASSWD:ALL\n    ssh_authorized_keys:\n      - {{.SSHKey}}' },
    { name: 'Package', snippet: 'packages:\n  - package-name' },
    { name: 'Runcmd', snippet: 'runcmd:\n  - echo "Hello World"' },
    { name: 'Write Files', snippet: 'write_files:\n  - path: /etc/example.conf\n    content: |\n      example content' },
    { name: 'Hostname', snippet: 'hostname: {{.Hostname}}\nmanage_etc_hosts: true' },
  ];
</script>

<Modal bind:open title={initialName ? 'Edit Cloud-init Template' : 'Create Cloud-init Template'} closeOnBackdrop={!submitting} width="wide">
  <div class="space-y-5">
    {#if formError}
      <div class="rounded border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">
        {formError}
      </div>
    {/if}

    <!-- Name -->
    <FormField label="Template Name" error={nameError} required labelFor="template-name">
      <Input
        id="template-name"
        bind:value={name}
        placeholder="e.g., My Custom Template"
        disabled={submitting || !!initialName}
        onblur={validateName}
      />
    </FormField>

    <!-- Description -->
    <FormField label="Description" labelFor="template-description">
      <Input
        id="template-description"
        bind:value={description}
        placeholder="Brief description of what this template does..."
        disabled={submitting}
      />
    </FormField>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <!-- Editor -->
      <div>
        <div class="flex items-center justify-between mb-2">
          <label for="cloudinit-content" class="text-sm font-medium text-ink">
            Cloud-init Content
          </label>
          <div class="flex items-center gap-2">
            <button
              type="button"
              onclick={copyToClipboard}
              class="flex items-center gap-1 text-xs text-muted hover:text-ink"
              title="Copy to clipboard"
            >
              {#if copied}
                <Check size={12} class="text-success" />
              {:else}
                <Copy size={12} />
              {/if}
              {copied ? 'Copied!' : 'Copy'}
            </button>
          </div>
        </div>
        
        {#if contentError}
          <div class="text-xs text-danger mb-1">{contentError}</div>
        {/if}
        
        <textarea
          id="cloudinit-content"
          bind:value={content}
          class="w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 font-mono text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          rows={20}
          disabled={submitting}
          spellcheck={false}
        ></textarea>
        
        <div class="mt-2 text-xs text-muted">
          {content.length} characters • {variables.length} variable{variables.length !== 1 ? 's' : ''}
        </div>
      </div>

      <!-- Sidebar: Variables & Snippets -->
      <div class="space-y-4">
        <!-- Variables Section -->
        <div class="bg-chrome rounded-lg p-4">
          <div class="flex items-center gap-2 mb-3">
            <Variable size={16} class="text-primary" />
            <h4 class="text-sm font-semibold text-ink">Template Variables</h4>
          </div>
          
          {#if variables.length > 0}
            <div class="flex flex-wrap gap-2 mb-3">
              {#each variables as varName}
                <button
                  type="button"
                  onclick={() => insertVariable(varName)}
                  class="text-xs bg-white px-2 py-1 rounded border border-line text-primary hover:border-primary transition-colors"
                  title="Insert variable"
                >
                  {'{{.' + varName + '}}'}
                </button>
              {/each}
            </div>
            
            <!-- Preview Inputs -->
            <div class="border-t border-line pt-3 mt-3">
              <h5 class="text-xs font-medium text-muted mb-2">Preview Values</h5>
              <div class="space-y-2">
                {#each variables as varName}
                  <div>
                    <label class="block text-xs text-muted mb-1">{varName}</label>
                    <input
                      type="text"
                      value={previewVariables[varName] || ''}
                      oninput={(e) => {
                        previewVariables = { ...previewVariables, [varName]: e.currentTarget.value };
                      }}
                      placeholder={`Enter ${varName}...`}
                      class="w-full h-7 rounded border border-[#CCCCCC] bg-white px-2 py-1 text-sm"
                    />
                  </div>
                {/each}
              </div>
              <button
                type="button"
                onclick={() => { showPreview = true; updatePreview(); }}
                class="mt-3 text-xs text-primary hover:text-primary/80 font-medium"
              >
                Update Preview
              </button>
            </div>
          {:else}
            <p class="text-sm text-muted">
              No variables detected. Use {'{{.VariableName}}'} syntax to add variables.
            </p>
          {/if}
        </div>

        <!-- Snippets Section -->
        <div class="bg-chrome rounded-lg p-4">
          <div class="flex items-center gap-2 mb-3">
            <FileCode size={16} class="text-primary" />
            <h4 class="text-sm font-semibold text-ink">Quick Snippets</h4>
          </div>
          <div class="space-y-2">
            {#each commonSnippets as snippet}
              <button
                type="button"
                onclick={() => insertSnippet(snippet.snippet)}
                class="w-full text-left text-xs px-3 py-2 rounded border border-line hover:border-primary hover:bg-white transition-colors"
              >
                {snippet.name}
              </button>
            {/each}
          </div>
        </div>

        <!-- Preview Section -->
        {#if showPreview && renderedPreview}
          <div class="bg-chrome rounded-lg p-4">
            <div class="flex items-center justify-between mb-2">
              <h4 class="text-sm font-semibold text-ink">Rendered Preview</h4>
              <button
                type="button"
                onclick={() => showPreview = false}
                class="text-xs text-muted hover:text-ink"
              >
                <EyeOff size={12} />
              </button>
            </div>
            <div class="rounded bg-neutral-900 overflow-auto max-h-64">
              <pre class="p-3 text-xs font-mono whitespace-pre-wrap"><code>{@html highlightYAML(renderedPreview)}</code></pre>
            </div>
          </div>
        {:else}
          <button
            type="button"
            onclick={() => { showPreview = true; updatePreview(); }}
            class="flex items-center gap-2 text-sm text-primary hover:text-primary/80"
          >
            <Eye size={16} />
            Show Preview
          </button>
        {/if}
      </div>
    </div>
  </div>

  {#snippet footer()}
    <button
      type="button"
      onclick={() => open = false}
      disabled={submitting}
      class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    >
      Cancel
    </button>
    <button
      type="button"
      onclick={handleSubmit}
      disabled={submitting || !name.trim() || !content.trim()}
      class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2"
    >
      {#if submitting}
        <svg class="animate-spin h-4 w-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" aria-hidden="true">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
        </svg>
      {/if}
      {submitting ? 'Creating...' : (initialName ? 'Update Template' : 'Create Template')}
    </button>
  {/snippet}
</Modal>
