<script lang="ts">
	import type { PageDefinition } from '$lib/shell/app-shell';
	import { Search } from 'lucide-svelte';

	interface Props {
		page: PageDefinition;
	}

	let { page }: Props = $props();

	function openCommandPalette() {
		// TODO: implement command palette modal
		console.log('Command palette not yet implemented');
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
			e.preventDefault();
			openCommandPalette();
		}
	}}
/>

<div class="command-bar">
	<div class="command-bar__context">
		<div class="context-label">Control plane</div>
		<div class="context-value">{page.navLabel}</div>
	</div>

	<button class="command-bar__search" onclick={openCommandPalette} aria-label="Open command palette">
		<span class="search-icon" aria-hidden="true">
			<Search size={14} />
		</span>
		<span class="search-placeholder">Search commands or jump to a resource</span>
		<div class="search-kbd">⌘K</div>
	</button>
</div>

<style>
	.command-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1.5rem;
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-sm);
		background: var(--bg-surface);
		padding: 0.5rem 1rem;
		min-height: 48px;
	}

	.command-bar__context {
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
	}

	.context-label {
		font-size: var(--text-xs);
		font-weight: 700;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.context-value {
		font-size: var(--text-sm);
		font-weight: 700;
		color: var(--shell-text);
	}

	.command-bar__search {
		display: flex;
		align-items: center;
		gap: 0.625rem;
		min-height: 36px;
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-sm);
		background: var(--bg-surface-muted);
		padding: 0 0.75rem;
		width: min(28rem, 100%);
		transition:
			border-color 0.15s ease,
			box-shadow 0.15s ease,
			background-color 0.15s ease;
		cursor: pointer;
		font-family: inherit;
	}

	.command-bar__search:hover {
		border-color: var(--color-primary);
		background: color-mix(in srgb, var(--bg-surface-muted) 82%, var(--color-primary-light));
	}

	.command-bar__search:focus-visible {
		outline: none;
		border-color: var(--color-primary);
		box-shadow: 0 0 0 3px rgba(var(--color-primary-rgb), 0.14);
	}

	.search-icon {
		display: inline-flex;
		flex-shrink: 0;
		color: var(--shell-text-muted);
	}

	.search-placeholder {
		flex: 1;
		border: 0;
		background: transparent;
		padding: 0;
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--shell-text-secondary);
		text-align: left;
	}

	.search-kbd {
		flex-shrink: 0;
		font-size: var(--text-xs);
		font-weight: 700;
		color: var(--shell-text-muted);
		background: var(--bg-surface);
		min-width: 2.5rem;
		height: 1.5rem;
		padding: 0 0.45rem;
		display: grid;
		place-items: center;
		border-radius: var(--radius-xs);
		border: 1px solid var(--border-subtle);
	}

	@media (max-width: 960px) {
		.command-bar {
			flex-direction: column;
			align-items: stretch;
			gap: 0.75rem;
		}
		.command-bar__search {
			width: 100%;
		}
	}
</style>
