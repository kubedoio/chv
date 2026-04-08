<script lang="ts">
  import { FileText } from 'lucide-svelte';
  
  interface Props {
    userData?: string;
    metaData?: string;
    networkConfig?: string;
  }
  
  let { userData = '', metaData = '', networkConfig = '' }: Props = $props();
  let activeTab = $state('user-data');
</script>

<div class="cloud-init-preview">
  <div class="flex items-center gap-2 mb-3">
    <FileText size={16} />
    <span class="font-medium">Cloud-init Configuration</span>
  </div>
  
  <div class="tabs flex gap-1 mb-3 border-b border-line">
    <button 
      onclick={() => activeTab = 'user-data'}
      class="px-3 py-2 text-sm {activeTab === 'user-data' ? 'border-b-2 border-primary font-medium' : 'text-muted'}"
    >
      user-data
    </button>
    <button 
      onclick={() => activeTab = 'meta-data'}
      class="px-3 py-2 text-sm {activeTab === 'meta-data' ? 'border-b-2 border-primary font-medium' : 'text-muted'}"
    >
      meta-data
    </button>
    <button 
      onclick={() => activeTab = 'network-config'}
      class="px-3 py-2 text-sm {activeTab === 'network-config' ? 'border-b-2 border-primary font-medium' : 'text-muted'}"
    >
      network-config
    </button>
  </div>
  
  <div class="content bg-[#1a1a1a] text-[#d4d4d4] rounded p-4 font-mono text-sm overflow-x-auto">
    {#if activeTab === 'user-data'}
      <pre>{userData || '# No user-data configured'}</pre>
    {:else if activeTab === 'meta-data'}
      <pre>{metaData || '# No meta-data'}</pre>
    {:else}
      <pre>{networkConfig || '# No network-config'}</pre>
    {/if}
  </div>
</div>

<style>
  pre {
    margin: 0;
    white-space: pre-wrap;
    word-wrap: break-word;
  }
</style>
