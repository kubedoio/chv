<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Terminal } from 'xterm';
	import { FitAddon } from 'xterm-addon-fit';
	import 'xterm/css/xterm.css';
	import { PlugZap, Unplug, Copy, Check, Download } from 'lucide-svelte';

	interface Props {
		vmId: string;
		consoleUrl: string;
		getConsoleUrl?: () => Promise<string>;
	}

	let { vmId, consoleUrl, getConsoleUrl }: Props = $props();

	let terminalEl: HTMLDivElement;
	let terminal: Terminal;
	let fitAddon: FitAddon;
	let socket: WebSocket;
	let connected = $state(false);
	let statusText = $state('Disconnected');
	let copied = $state(false);
	let copyTimer: ReturnType<typeof setTimeout>;
	let wsError = $state('');

	function validateWsUrl(url: string): boolean {
		// Allow relative paths (starting with /) — they're always same-origin
		if (url.startsWith('/')) return true;
		// Allow ws:// or wss:// only if they target the same host
		if (url.startsWith('ws://') || url.startsWith('wss://')) {
			try {
				const parsed = new URL(url);
				return parsed.host === window.location.host;
			} catch {
				return false;
			}
		}
		return false;
	}

	function buildWsUrl(url: string): string {
		return url.startsWith('ws')
			? url
			: `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}${url}`;
	}

	function connectWith(url: string) {
		if (!validateWsUrl(url)) {
			wsError = `Refused to connect: WebSocket URL does not match the application origin.`;
			statusText = 'Connection blocked';
			terminal.writeln('\r\n\x1b[31m[Connection blocked: invalid WebSocket origin]\x1b[0m');
			return;
		}
		wsError = '';
		if (socket) socket.close();
		const wsUrl = buildWsUrl(url);
		socket = new WebSocket(wsUrl);
		socket.binaryType = 'arraybuffer';

		socket.onopen = () => {
			connected = true;
			statusText = 'Connected';
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
			statusText = 'Disconnected';
			terminal.writeln('\r\n\x1b[31m[Disconnected]\x1b[0m');
			// No auto-reconnect: user controls reconnection explicitly
		};

		socket.onerror = () => {
			connected = false;
			statusText = 'Disconnected';
			terminal.writeln('\r\n\x1b[31m[Connection error]\x1b[0m');
		};
	}

	async function handleReconnect() {
		let urlToUse = consoleUrl;

		if (getConsoleUrl) {
			statusText = 'Refreshing token...';
			try {
				urlToUse = await getConsoleUrl();
			} catch {
				terminal.writeln('\r\n\x1b[33m[Token refresh failed, retrying with existing URL]\x1b[0m');
				urlToUse = consoleUrl;
			}
		}

		connectWith(urlToUse);
	}

	function handleDisconnect() {
		if (socket) {
			socket.onclose = () => {
				connected = false;
				statusText = 'Disconnected';
				terminal.writeln('\r\n\x1b[31m[Disconnected]\x1b[0m');
			};
			socket.close();
		}
	}

	function getTerminalContent(): string {
		const buffer = terminal.buffer.active;
		const lines: string[] = [];
		for (let i = 0; i < buffer.length; i++) {
			const line = buffer.getLine(i);
			if (line) lines.push(line.translateToString(true));
		}
		return lines.join('\n');
	}

	async function handleCopy() {
		const text = getTerminalContent();
		try {
			await navigator.clipboard.writeText(text);
			copied = true;
			clearTimeout(copyTimer);
			copyTimer = setTimeout(() => {
				copied = false;
			}, 2000);
		} catch {
			terminal.writeln('\r\n\x1b[33m[Copy failed: clipboard not available]\x1b[0m');
		}
	}

	function handleDownload() {
		const text = getTerminalContent();
		const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
		const filename = `console-${vmId}-${timestamp}.txt`;
		const blob = new Blob([text], { type: 'text/plain' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = filename;
		a.click();
		URL.revokeObjectURL(url);
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

		connectWith(consoleUrl);
	});

	onDestroy(() => {
		clearTimeout(copyTimer);
		socket?.close();
		terminal?.dispose();
	});
</script>

<div class="console-wrapper">
	{#if wsError}
		<div class="console-error">{wsError}</div>
	{/if}
	<div class="console-toolbar">
		<div class="toolbar-left">
			<span class="console-status">
				<span class="status-dot" class:connected></span>
				{statusText}
			</span>
			<span class="console-meta">VM {vmId}</span>
		</div>
		<div class="toolbar-right">
			<button
				class="toolbar-btn"
				title="Copy terminal contents"
				onclick={handleCopy}
			>
				{#if copied}
					<Check size={14} />
				{:else}
					<Copy size={14} />
				{/if}
			</button>
			<button
				class="toolbar-btn"
				title="Download terminal contents"
				onclick={handleDownload}
			>
				<Download size={14} />
			</button>
			{#if statusText !== 'Refreshing token...'}
				{#if connected}
					<button
						class="toolbar-btn"
						title="Disconnect"
						onclick={handleDisconnect}
					>
						<Unplug size={14} />
					</button>
				{:else}
					<button
						class="toolbar-btn"
						title="Reconnect"
						onclick={handleReconnect}
					>
						<PlugZap size={14} />
					</button>
				{/if}
			{/if}
		</div>
	</div>
	<div bind:this={terminalEl} class="terminal-container"></div>
</div>

<style>
	.console-wrapper {
		width: 100%;
		background: var(--console-bg, #1a1a1a);
		border-radius: 8px;
		border: 1px solid var(--console-line, #333);
		overflow: hidden;
	}

	.console-toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem 0.75rem;
		background: var(--console-surface, #252525);
		border-bottom: 1px solid var(--console-line, #333);
		font-size: 0.8rem;
	}

	.toolbar-left {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.toolbar-right {
		display: flex;
		align-items: center;
		gap: 0.25rem;
	}

	.console-status {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		color: var(--console-text-secondary, #aaa);
	}

	.status-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--console-disconnected, #ef4444);
	}

	.status-dot.connected {
		background: var(--console-connected, #22c55e);
	}

	.console-meta {
		color: var(--console-text-muted, #888);
		font-family: monospace;
	}

	.toolbar-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		padding: 0;
		background: transparent;
		border: none;
		border-radius: 4px;
		color: var(--console-text-secondary, #aaa);
		cursor: pointer;
		transition: background 0.15s ease;
	}

	.toolbar-btn:hover {
		background: rgba(255, 255, 255, 0.08);
		color: var(--console-text, #ddd);
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

	.console-error {
		padding: 0.5rem 0.75rem;
		background: var(--console-error-bg, #3b1111);
		border-bottom: 1px solid var(--console-error-border, #7f1d1d);
		color: var(--console-error-text, #fca5a5);
		font-size: 0.8rem;
	}
</style>
