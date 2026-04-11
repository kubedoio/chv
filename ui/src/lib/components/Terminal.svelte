<script lang="ts">
  import { onMount, onDestroy } from 'svelte';

  interface Props {
    wsUrl: string;
    onClose?: () => void;
    fullscreen?: boolean;
  }

  let { wsUrl, onClose, fullscreen = false }: Props = $props();

  let terminal: HTMLDivElement;
  let input: HTMLInputElement;
  let ws: WebSocket | null = null;
  let connected = $state(false);
  let output = $state<string[]>([]);
  let command = $state('');

  // Buffer for incomplete lines from the PTY
  let lineBuffer = $state('');

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
        addSystemLine('Connected to VM console');
      };

      ws.onmessage = (event) => {
        appendConsoleData(event.data);
      };

      ws.onerror = (error) => {
        addSystemLine('Connection error');
      };

      ws.onclose = () => {
        connected = false;
        // Flush any remaining buffered content
        if (lineBuffer) {
          output = [...output, lineBuffer];
          lineBuffer = '';
        }
        addSystemLine('Disconnected from console');
        onClose?.();
      };
    } catch (err) {
      addSystemLine(`Failed to connect: ${err}`);
    }
  }

  function disconnect() {
    if (ws) {
      ws.close();
      ws = null;
    }
  }

  function addSystemLine(text: string) {
    output = [...output, { text, type: 'system' as const }];
    scrollToBottom();
  }

  function appendConsoleData(data: string) {
    // Append incoming data to the line buffer
    lineBuffer += data;

    // Split on newlines - handle \r\n, \n, and \r
    // \r is used by terminals to rewrite the current line (e.g. prompts)
    const lines: string[] = [];
    let current = '';

    for (let i = 0; i < lineBuffer.length; i++) {
      const ch = lineBuffer[i];
      if (ch === '\r') {
        // Carriage return - discard current line content (terminal rewriting)
        current = '';
      } else if (ch === '\n') {
        // Newline - push current line and reset
        lines.push(current);
        current = '';
      } else {
        current += ch;
      }
    }

    // Update buffer with remaining incomplete line
    lineBuffer = current;

    // Add complete lines to output
    if (lines.length > 0) {
      const newLines = lines.map(text => ({ text, type: 'output' as const }));
      output = [...output, ...newLines];
      scrollToBottom();
    }
  }

  function scrollToBottom() {
    if (terminal) {
      requestAnimationFrame(() => {
        if (terminal && terminal.isConnected) {
          terminal.scrollTop = terminal.scrollHeight;
        }
      });
    }
  }

  function sendCommand(e: Event) {
    e.preventDefault();
    if (ws && connected && command) {
      ws.send(command + '\n');
      output = [...output, { text: `> ${command}`, type: 'input' as const }];
      command = '';
      scrollToBottom();
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      sendCommand(e);
    }
  }
</script>

<div class="terminal-container border border-line rounded bg-gray-900 text-white" class:terminal-fullscreen={fullscreen}>
  <div class="terminal-header flex justify-between items-center px-3 py-2 bg-gray-800 border-b border-gray-700">
    <div class="flex items-center gap-2">
      <div class="w-3 h-3 rounded-full" class:bg-green-500={connected} class:bg-red-500={!connected}></div>
      <span class="text-xs font-mono">{connected ? 'Connected' : 'Disconnected'}</span>
    </div>
    <button onclick={onClose} class="text-xs text-gray-400 hover:text-white">Close</button>
  </div>

  <div
    bind:this={terminal}
    class="terminal-output p-3 font-mono text-sm overflow-y-auto"
    class:h-64={!fullscreen}
    class:flex-1={fullscreen}
  >
    {#each output as line}
      <div class="whitespace-pre-wrap break-all {line.type === 'error' ? 'text-red-400' : line.type === 'system' ? 'text-yellow-400' : line.type === 'input' ? 'text-blue-400' : 'text-gray-200'}">
        {line.text}
      </div>
    {/each}
    <!-- Render incomplete buffer content (current line being typed by VM) -->
    {#if lineBuffer}
      <div class="whitespace-pre-wrap break-all text-gray-200">{lineBuffer}</div>
    {/if}
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
    display: flex;
    flex-direction: column;
  }

  .terminal-fullscreen {
    height: 100%;
    width: 100%;
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
