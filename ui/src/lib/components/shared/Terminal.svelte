<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Terminal } from 'xterm';
  import { AttachAddon } from 'xterm-addon-attach';
  import { FitAddon } from 'xterm-addon-fit';
  import 'xterm/css/xterm.css';

  interface Props {
    wsUrl: string;
    onClose?: () => void;
    fullscreen?: boolean;
  }

  let { wsUrl, onClose, fullscreen = false }: Props = $props();

  let terminalElement: HTMLDivElement;
  let term: Terminal;
  let socket: WebSocket;
  let fitAddon: FitAddon;
  let connected = $state(false);

  onMount(() => {
    initTerminal();
    return () => {
      cleanup();
    };
  });

  function initTerminal() {
    // Initialize xterm.js terminal
    term = new Terminal({
      cursorBlink: true,
      cursorStyle: 'block',
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      fontSize: 14,
      theme: {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
        cursor: '#d4d4d4',
        selectionBackground: '#264f78',
        black: '#000000',
        red: '#cd3131',
        green: '#0dbc79',
        yellow: '#e5e510',
        blue: '#2472c8',
        magenta: '#bc3fbc',
        cyan: '#11a8cd',
        white: '#e5e5e5',
        brightBlack: '#666666',
        brightRed: '#f14c4c',
        brightGreen: '#23d18b',
        brightYellow: '#f5f543',
        brightBlue: '#3b8eea',
        brightMagenta: '#d670d6',
        brightCyan: '#29b8db',
        brightWhite: '#e5e5e5'
      },
      scrollback: 10000,
      allowProposedApi: true
    });

    // Add fit addon for responsive sizing
    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);

    // Open terminal in the container
    term.open(terminalElement);
    fitAddon.fit();

    // Connect to WebSocket
    connectWebSocket();

    // Handle window resize
    window.addEventListener('resize', handleResize);

    // Focus the terminal
    term.focus();
  }

  function connectWebSocket() {
    try {
      socket = new WebSocket(wsUrl);

      socket.onopen = () => {
        connected = true;
        // Add attach addon to bridge WebSocket to terminal
        const attachAddon = new AttachAddon(socket);
        term.loadAddon(attachAddon);
        
        // Send initial newline to trigger login prompt
        setTimeout(() => {
          socket.send('\r');
        }, 500);
      };

      socket.onerror = (error) => {
        term.writeln('\r\n[Connection error]');
        connected = false;
      };

      socket.onclose = () => {
        connected = false;
        term.writeln('\r\n[Disconnected from console]');
        onClose?.();
      };
    } catch (err) {
      term.writeln(`\r\n[Failed to connect: ${err}]`);
    }
  }

  function handleResize() {
    if (fitAddon) {
      fitAddon.fit();
    }
  }

  function cleanup() {
    window.removeEventListener('resize', handleResize);
    if (socket) {
      socket.close();
    }
    if (term) {
      term.dispose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    // Handle Ctrl+C for copy
    if (e.ctrlKey && e.key === 'c' && e.shiftKey) {
      e.preventDefault();
      const selection = term.getSelection();
      if (selection) {
        navigator.clipboard.writeText(selection);
      }
    }
    // Handle Ctrl+Shift+V for paste
    if (e.ctrlKey && e.key === 'v' && e.shiftKey) {
      e.preventDefault();
      navigator.clipboard.readText().then(text => {
        socket?.send(text);
      });
    }
  }
</script>

<div class="terminal-container border border-line rounded bg-gray-900" class:terminal-fullscreen={fullscreen}>
  <div class="terminal-header flex justify-between items-center px-3 py-2 bg-gray-800 border-b border-gray-700">
    <div class="flex items-center gap-2">
      <div class="w-3 h-3 rounded-full" class:bg-green-500={connected} class:bg-red-500={!connected}></div>
      <span class="text-xs font-mono text-white">{connected ? 'Connected' : 'Disconnected'}</span>
    </div>
    <div class="flex items-center gap-2">
      <span class="text-xs text-gray-400">Ctrl+Shift+C: Copy | Ctrl+Shift+V: Paste</span>
      <button type="button" onclick={onClose} class="text-xs text-gray-400 hover:text-white ml-2">Close</button>
    </div>
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={terminalElement}
    class="terminal-content"
    class:h-96={!fullscreen}
    class:flex-1={fullscreen}
    onkeydown={handleKeydown}
    tabindex="0"
    role="application"
    aria-label="Terminal"
  ></div>
</div>

<style>
  .terminal-container {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .terminal-fullscreen {
    height: 100%;
    width: 100%;
  }

  .terminal-content {
    padding: 8px;
    overflow: hidden;
  }

  .terminal-content :global(.xterm) {
    height: 100%;
  }

  .terminal-content :global(.xterm-viewport) {
    scrollbar-width: thin;
    scrollbar-color: #4b5563 #1f2937;
  }

  .terminal-content :global(.xterm-viewport::-webkit-scrollbar) {
    width: 8px;
  }

  .terminal-content :global(.xterm-viewport::-webkit-scrollbar-track) {
    background: #1f2937;
  }

  .terminal-content :global(.xterm-viewport::-webkit-scrollbar-thumb) {
    background: #4b5563;
    border-radius: 4px;
  }

  .terminal-content:focus {
    outline: none;
  }
</style>
