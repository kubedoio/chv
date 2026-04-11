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
  let buffer = $state('');
  let command = $state('');

  // Maximum buffer size to prevent memory growth
  const MAX_BUFFER_SIZE = 100_000;

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
        appendText('\r\n[Connected to VM console]\r\n');
      };

      ws.onmessage = (event) => {
        appendText(event.data);
      };

      ws.onerror = () => {
        appendText('\r\n[Connection error]\r\n');
      };

      ws.onclose = () => {
        connected = false;
        appendText('\r\n[Disconnected from console]\r\n');
        onClose?.();
      };
    } catch (err) {
      appendText(`\r\n[Failed to connect: ${err}]\r\n`);
    }
  }

  function disconnect() {
    if (ws) {
      ws.close();
      ws = null;
    }
  }

  function appendText(text: string) {
    buffer += text;
    // Trim buffer if it gets too large (keep last 90%)
    if (buffer.length > MAX_BUFFER_SIZE) {
      buffer = buffer.slice(-Math.floor(MAX_BUFFER_SIZE * 0.9));
    }
    scrollToBottom();
  }

  function scrollToBottom() {
    if (terminal) {
      requestAnimationFrame(() => {
        if (terminal) {
          terminal.scrollTop = terminal.scrollHeight;
        }
      });
    }
  }

  function sendCommand(e: Event) {
    e.preventDefault();
    if (ws && connected && command) {
      // Send command to VM - VM will echo it back naturally
      ws.send(command + '\n');
      command = '';
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      sendCommand(e);
    }
  }

  // Strip ANSI escape sequences for display
  function stripAnsi(text: string): string {
    // eslint-disable-next-line no-control-regex
    return text
      .replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '')   // CSI sequences like [?2004h, [0m, [32m
      .replace(/\x1b\][0-9;]*\x07/g, '')       // OSC sequences
      .replace(/\x1b\[\?[0-9]*[hl]/g, '')      // Set/reset mode sequences
      .replace(/\x1b\[[0-9]*[ABCDEFGJKST]/g, '') // Cursor movement
      .replace(/\x1b[@-Z\\-~]/g, '')          // Single-char sequences
      .replace(/\x1b\[[0-9;]*m/g, '')          // Color codes
      .replace(/\x1b\[\?[0-9;]*[a-zA-Z]/g, '') // Extended sequences
      .replace(/\x1b\[>\?[0-9;]*[a-zA-Z]/g, '')// Private sequences
      .replace(/\x1b[()][0-9A-Za-z]/g, '')    // Character set sequences
      .replace(/\x1b#\d/g, '')                // Line attributes
      .replace(/\x1b%/g, '')                  // Select charset
      .replace(/\x1b[0-7]/g, '')              // Control chars
      .replace(/\x1b[89:;<=>?]/g, '')         // More control
      .replace(/\x1b[cdefghlmnopsu`{|}~]/g, ''); // Final bytes
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
    class="terminal-output p-3 font-mono text-sm overflow-y-auto whitespace-pre"
    class:h-64={!fullscreen}
    class:flex-1={fullscreen}
  >
    {stripAnsi(buffer)}
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
