<script lang="ts">
	import { Activity, Terminal, Settings } from 'lucide-svelte';
</script>

<div class="command-grid">
	<button class="cmd-btn" disabled title="Coming soon">
		<Activity size={14} />
		<span>SYNC</span>
	</button>
	<button class="cmd-btn" disabled title="Coming soon">
		<Terminal size={14} />
		<span>BYPASS_SHELL</span>
	</button>
	<button class="cmd-btn" disabled title="Coming soon">
		<Settings size={14} />
		<span>CONFIG</span>
	</button>
</div>

<style>
	.command-grid {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 0.5rem;
	}

	.cmd-btn {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.5rem;
		padding: 0.75rem 0.5rem;
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-xs);
		font-size: 9px;
		font-weight: 800;
		color: var(--color-neutral-600);
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.cmd-btn:hover:not(:disabled) {
		background: var(--bg-surface-muted);
		border-color: var(--color-primary);
		color: var(--color-primary);
	}

	.cmd-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
