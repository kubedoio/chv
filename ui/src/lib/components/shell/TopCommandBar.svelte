<script lang="ts">
	import type { PageDefinition } from '$lib/shell/app-shell';

	interface Props {
		page: PageDefinition;
	}

	let { page }: Props = $props();
</script>

<div class="command-bar">
	<div class="command-bar__scope">
		<div class="command-bar__scope-label">CHV WebUI</div>
		<div class="command-bar__scope-value">{page.navLabel}</div>
	</div>

	<label class="command-bar__search" aria-label="Command search">
		<span>/</span>
		<input
			type="search"
			value=""
			placeholder="Jump to nodes, workloads, volumes, or tasks"
			readonly
		/>
		<kbd>⌘K</kbd>
	</label>
</div>

<style>
	.command-bar {
		display: grid;
		grid-template-columns: auto minmax(16rem, 24rem);
		align-items: center;
		justify-content: space-between;
		gap: 0.9rem;
		border: 1px solid var(--shell-line);
		border-radius: 0.5rem;
		background: var(--shell-surface);
		padding: 0.5rem 0.75rem;
		min-height: var(--header-height);
	}

	.command-bar__scope {
		display: grid;
		gap: 0.1rem;
	}

	.command-bar__scope-label {
		font-size: 0.72rem;
		font-weight: 600;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.command-bar__scope-value {
		font-size: 0.92rem;
		font-weight: 600;
		color: var(--shell-text);
	}

	.command-bar__search {
		display: grid;
		grid-template-columns: auto 1fr auto;
		align-items: center;
		gap: 0.75rem;
		height: 2rem;
		border: 1px solid var(--shell-line);
		border-radius: 0.35rem;
		background: var(--shell-surface-muted);
		padding: 0 0.5rem;
	}

	.command-bar__search span,
	.command-bar__search kbd {
		font-size: 0.8rem;
		color: var(--shell-text-muted);
	}

	.command-bar__search input {
		width: 100%;
		border: 0;
		background: transparent;
		padding: 0;
		font-size: 0.95rem;
		color: var(--shell-text);
	}

	.command-bar__search input::placeholder {
		color: var(--shell-text-muted);
	}

	.command-bar__search input:focus {
		outline: none;
	}


	@media (max-width: 960px) {
		.command-bar {
			grid-template-columns: 1fr;
		}
	}
</style>
