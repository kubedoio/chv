<script lang="ts">
	import { slide } from 'svelte/transition';
	import { Settings2, X } from 'lucide-svelte';

	interface Props {
		columns: Array<{ key: string; title: string }>;
		visibleColumns: Set<string>;
		onToggle: (key: string) => void;
	}

	let { columns, visibleColumns, onToggle }: Props = $props();

	let open = $state(false);
</script>

<div class="datatable-toolbar">
	<button
		type="button"
		class="toolbar-btn"
		onclick={() => (open = !open)}
		aria-expanded={open}
	>
		<Settings2 size={14} />
		<span>Columns</span>
	</button>

	{#if open}
		<div class="column-menu" transition:slide={{ duration: 150 }}>
			<div class="column-menu-header">
				<span>Show columns</span>
				<button
					type="button"
					class="menu-close"
					onclick={() => (open = false)}
					aria-label="Close column menu"
				>
					<X size={14} />
				</button>
			</div>
			<div class="column-list">
				{#each columns as column}
					<label class="column-item">
						<input
							type="checkbox"
							checked={visibleColumns.has(column.key)}
							onchange={() => onToggle(column.key)}
							disabled={visibleColumns.size === 1 && visibleColumns.has(column.key)}
						/>
						<span>{column.title}</span>
					</label>
				{/each}
			</div>
		</div>
	{/if}
</div>

<style>
	.datatable-toolbar {
		position: relative;
		display: flex;
		justify-content: flex-end;
		padding: 0.5rem 1rem;
		border-bottom: 1px solid var(--color-neutral-200);
	}

	.toolbar-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.375rem 0.75rem;
		background: white;
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-sm);
		color: var(--color-neutral-600);
		font-size: var(--text-sm);
		cursor: pointer;
		transition: all var(--duration-fast);
	}

	.toolbar-btn:hover {
		border-color: var(--color-neutral-300);
		background: var(--color-neutral-50);
	}

	.column-menu {
		position: absolute;
		top: calc(100% + 0.25rem);
		right: 1rem;
		z-index: 50;
		min-width: 200px;
		background: white;
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-lg);
	}

	.column-menu-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem;
		border-bottom: 1px solid var(--color-neutral-100);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-neutral-700);
	}

	.menu-close {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0.25rem;
		background: transparent;
		border: none;
		border-radius: var(--radius-sm);
		color: var(--color-neutral-400);
		cursor: pointer;
		transition: all var(--duration-fast);
	}

	.menu-close:hover {
		color: var(--color-neutral-600);
		background: var(--color-neutral-100);
	}

	.column-list {
		max-height: 300px;
		overflow-y: auto;
		padding: 0.5rem;
	}

	.column-item {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-size: var(--text-sm);
		color: var(--color-neutral-700);
		transition: background var(--duration-fast);
	}

	.column-item:hover {
		background: var(--color-neutral-50);
	}

	.column-item input[type='checkbox'] {
		accent-color: var(--color-primary);
	}

	.column-item input[type='checkbox']:disabled {
		cursor: not-allowed;
	}
</style>
