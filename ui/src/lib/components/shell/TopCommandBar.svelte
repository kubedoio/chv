<script lang="ts">
	import type { PageDefinition } from '$lib/shell/app-shell';
  import { Search } from 'lucide-svelte';

	interface Props {
		page: PageDefinition;
	}

	let { page }: Props = $props();
</script>

<div class="command-bar">
	<div class="command-bar__context">
		<div class="context-label">CHV_PROTOCOL_SHELL</div>
		<div class="context-value">{page.navLabel}</div>
	</div>

	<div class="command-bar__search">
		<Search size={14} class="search-icon" />
		<input
			type="text"
			placeholder="ACTIVATE_COMMAND_PALETTE (⌘K)"
			readonly
		/>
		<div class="search-kbd">/</div>
	</div>
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
		font-size: 8px;
		font-weight: 800;
		letter-spacing: 0.15em;
		text-transform: uppercase;
		color: var(--color-neutral-400);
	}

	.context-value {
		font-size: 11px;
		font-weight: 800;
		color: var(--color-neutral-900);
    text-transform: uppercase;
    letter-spacing: 0.05em;
	}

	.command-bar__search {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		height: 28px;
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-xs);
		background: var(--bg-surface-muted);
		padding: 0 0.75rem;
    width: 320px;
    transition: border-color 0.15s ease;
	}

  .command-bar__search:hover {
    border-color: var(--color-primary);
  }

  .search-icon {
    color: var(--color-neutral-400);
  }

	.command-bar__search input {
		flex: 1;
		border: 0;
		background: transparent;
		padding: 0;
		font-size: 10px;
    font-weight: 700;
		color: var(--color-neutral-900);
    font-family: var(--font-mono);
	}

	.command-bar__search input::placeholder {
		color: var(--color-neutral-400);
    opacity: 0.6;
	}

	.command-bar__search input:focus {
		outline: none;
	}

  .search-kbd {
    font-size: 10px;
    font-weight: 800;
    color: var(--color-neutral-400);
    background: var(--bg-surface);
    width: 16px;
    height: 16px;
    display: grid;
    place-items: center;
    border-radius: 2px;
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
