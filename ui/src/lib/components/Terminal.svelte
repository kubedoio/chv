<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  
  interface Props {
    wsUrl: string;
    onClose?: () => void;
  }
  
  let { wsUrl, onClose }: Props = $props();
  
  let terminal: HTMLDivElement;
  let input: HTMLInputElement;
  let ws: WebSocket | null = null;
  let connected = $state(false);
  let output = $state<string[]>([]);
  let command = $state('');
  
  onMount(() => {
    connect();
  });
  
  onDestroy(() => {
    disconnect();
  });
  
  function connect() {
    try {
      ws = new WebSocket(wsUrl);
      
      ws.onopen = () => {
        connected = true;
        addOutput('Connected to VM console', 'system');
      };
      
      ws.onmessage = (event) => {
        addOutput(event.data, 'output');
      };
      
      ws.onerror = (error) => {
        addOutput('Connection error', 'error');
      };
      
      ws.onclose = () => {
        connected = false;
        addOutput('Disconnected from console', 'system');
        onClose?.();
      };
    } catch (err) {
      addOutput(`Failed to connect: ${err}`, 'error');
    }
  }
  
  function disconnect() {
    if (ws) {
      ws.close();
      ws = null;
    }
  }
  
  function addOutput(text: string, type: 'output' | 'error' | 'system' = 'output') {
    output = [...output, `[${type}] ${text}`];
    // Scroll to bottom
    if (terminal) {
      setTimeout(() => {
        terminal.scrollTop = terminal.scrollHeight;
      }, 0);
    }
  }
  
  function sendCommand(e: Event) {
    e.preventDefault();
    if (ws && connected && command) {
      ws.send(command + '\n');
      addOutput(`> ${command}`, 'input');
      command = '';
    }
  }
  
  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      sendCommand(e);
    }
  }
</script>

<div class="terminal-container border border-line rounded bg-gray-900 text-white">
  <div class="terminal-header flex justify-between items-center px-3 py-2 bg-gray-800 border-b border-gray-700">
    <div class="flex items-center gap-2">
      <div class="w-3 h-3 rounded-full" class:bg-green-500={connected} class:bg-red-500={!connected}></div>
      <span class="text-xs font-mono">{connected ? 'Connected' : 'Disconnected'}</span>
    </div>
    <button onclick={onClose} class="text-xs text-gray-400 hover:text-white">Close</button>
  </div>
  
  <div 
    bind:this={terminal}
    class="terminal-output p-3 font-mono text-sm h-64 overflow-y-auto"
  >
    {#each output as line}
      <div class="whitespace-pre-wrap break-all {line.startsWith('[error]') ? 'text-red-400' : line.startsWith('[system]') ? 'text-yellow-400' : line.startsWith('[input]') ? 'text-blue-400' : 'text-gray-200'}">
        {line.replace(/^\[(\w+)\] /, '')}
      </div>
    {/each}
  </div>
  
  <form onsubmit={sendCommand} class="terminal-input flex border-t border-gray-700">
    <span class="px-3 py-2 text-green-400 font-mono text-sm">$</span>
    <input
      bind:this={input}
      bind:value={command}
      onkeydown={handleKeyDown}
      type="text"
      class="flex-1 bg-transparent text-white font-mono text-sm px-2 py-2 focus:outline-none"
      placeholder={connected ? 'Type command...' : 'Disconnected'}
      disabled={!connected}
    />
  </form>
</div>

<style>
  .terminal-container {
    font-family: 'Menlo', 'Monaco', 'Courier New', monospace;
  }
  
  .terminal-output::-webkit-scrollbar {
    width: 8px;
  }
  
  .terminal-output::-webkit-scrollbar-track {
    background: #1f2937;
  }
  
  .terminal-output::-webkit-scrollbar-thumb {
    background: #4b5563;
    border-radius: 4px;
  }
</style>
