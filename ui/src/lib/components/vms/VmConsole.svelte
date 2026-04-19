<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Terminal } from 'xterm';
	import { FitAddon } from 'xterm-addon-fit';
	import 'xterm/css/xterm.css';

	interface Props {
		vmId: string;
		consoleUrl: string;
	}

	let { vmId, consoleUrl }: Props = $props();

	let terminalEl: HTMLDivElement;
	let terminal: Terminal;
	let fitAddon: FitAddon;
	let socket: WebSocket;
	let reconnectTimer: ReturnType<typeof setTimeout>;
	let connected = $state(false);

	function connect() {
		if (socket) socket.close();
		const wsUrl = consoleUrl.startsWith('ws') ? consoleUrl : `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}${consoleUrl}`;
		socket = new WebSocket(wsUrl);
		socket.binaryType = 'arraybuffer';

		socket.onopen = () => {
			connected = true;
			terminal.writeln('\r\n\x1b[32m[Connected to serial console]\x1b[0m\r\n');
		};

		socket.onmessage = (event) => {
			if (event.data instanceof ArrayBuffer) {
				const data = new Uint8Array(event.data);
				terminal.write(data);
			}
		};

		socket.onclose = () => {
			connected = false;
			terminal.writeln('\r\n\x1b[31m[Disconnected]\x1b[0m');
			reconnectTimer = setTimeout(connect, 3000);
		};

		socket.onerror = () => {
			connected = false;
			terminal.writeln('\r\n\x1b[31m[Connection error]\x1b[0m');
		};
	}

	onMount(() => {
		terminal = new Terminal({
			cursorBlink: true,
			fontSize: 14,
			fontFamily: 'monospace',
			allowProposedApi: true
		});
		fitAddon = new FitAddon();
		terminal.loadAddon(fitAddon);
		terminal.open(terminalEl);
		fitAddon.fit();

		terminal.onData((data) => {
			if (socket?.readyState === WebSocket.OPEN) {
				socket.send(data);
			}
		});

		terminal.onResize(({ cols, rows }) => {
			if (socket?.readyState === WebSocket.OPEN) {
				socket.send(JSON.stringify({ cols, rows }));
			}
		});

		connect();
	});

	onDestroy(() => {
		clearTimeout(reconnectTimer);
		socket?.close();
		terminal?.dispose();
	});
</script>

<div class="console-wrapper">
	<div class="console-toolbar">
		<span class="console-status">
			<span class="status-dot" class:connected></span>
			{connected ? 'Connected' : 'Disconnected'}
		</span>
		<span class="console-meta">VM {vmId}</span>
	</div>
	<div bind:this={terminalEl} class="terminal-container"></div>
</div>

<style>
	.console-wrapper {
		width: 100%;
		background: var(--shell-surface, #1a1a1a);
		border-radius: 8px;
		border: 1px solid var(--shell-line, #333);
		overflow: hidden;
	}

	.console-toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem 0.75rem;
		background: var(--shell-surface-raised, #252525);
		border-bottom: 1px solid var(--shell-line, #333);
		font-size: 0.8rem;
	}

	.console-status {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		color: var(--shell-text-secondary, #aaa);
	}

	.status-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: #ef4444;
	}

	.status-dot.connected {
		background: #22c55e;
	}

	.console-meta {
		color: var(--shell-text-muted, #888);
		font-family: monospace;
	}

	.terminal-container {
		width: 100%;
		height: 500px;
		padding: 8px;
	}

	:global(.xterm) {
		height: 100%;
	}

	:global(.xterm-screen) {
		background: transparent !important;
	}
</style>
