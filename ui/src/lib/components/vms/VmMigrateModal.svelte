<script lang="ts">
	import Modal from '$lib/components/primitives/Modal.svelte';
	import Button from '$lib/components/primitives/Button.svelte';
	import { listNodes } from '$lib/bff/nodes';
	import { getStoredToken } from '$lib/api/client';
	import type { NodeListItem } from '$lib/bff/types';
	import { AlertCircle } from 'lucide-svelte';

	interface Props {
		open: boolean;
		vmId: string;
		currentNodeId: string;
		submitting?: boolean;
		onmigrate?: (targetNodeId: string) => void;
		onclose?: () => void;
	}

	let {
		open = $bindable(false),
		vmId,
		currentNodeId,
		submitting = false,
		onmigrate,
		onclose
	}: Props = $props();

	let nodes = $state<NodeListItem[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let selectedNodeId = $state('');

	const eligibleNodes = $derived(
		nodes.filter((n) => n.node_id !== currentNodeId && n.state === 'TenantReady')
	);

	$effect(() => {
		if (open) {
			selectedNodeId = '';
			error = null;
			fetchNodes();
		}
	});

	async function fetchNodes() {
		loading = true;
		error = null;
		try {
			const token = getStoredToken() ?? undefined;
			const res = await listNodes({ page: 1, page_size: 100, filters: {} }, token);
			nodes = res.items;
		} catch (err: unknown) {
			const message = err instanceof Error ? err.message : 'Failed to load nodes';
			error = message;
		} finally {
			loading = false;
		}
	}

	function handleMigrate() {
		if (!selectedNodeId || submitting) return;
		onmigrate?.(selectedNodeId);
	}
</script>

<Modal bind:open title="Migrate VM" onClose={onclose}>
{#snippet children()}
	<div class="form-fields">
		<p class="description">
			Select a target node to live-migrate VM <strong>{vmId}</strong>.
			The VM will remain running during the migration.
		</p>

		{#if loading}
			<p class="loading-hint">Loading available nodes...</p>
		{:else if error}
			<div class="form-error">
				<AlertCircle size={14} />
				<span>{error}</span>
			</div>
		{:else if eligibleNodes.length === 0}
			<div class="form-error">
				No eligible target nodes available. Nodes must be in TenantReady state.
			</div>
		{:else}
			<div class="field">
				<label for="migrate-target-node">Target Node</label>
				<select id="migrate-target-node" bind:value={selectedNodeId}>
					<option value="" disabled>Select a node...</option>
					{#each eligibleNodes as node (node.node_id)}
						<option value={node.node_id}>{node.name} ({node.node_id})</option>
					{/each}
				</select>
			</div>
		{/if}
	</div>
{/snippet}
{#snippet footer()}
	<Button type="button" onclick={onclose} variant="secondary">Cancel</Button>
	<Button
		type="button"
		onclick={handleMigrate}
		disabled={submitting || !selectedNodeId || loading}
		variant="primary"
	>
		{submitting ? 'Migrating...' : 'Migrate'}
	</Button>
{/snippet}
</Modal>

<style>
	.form-fields {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.description {
		font-size: var(--text-sm);
		color: var(--shell-text-muted);
		line-height: 1.5;
		margin: 0;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
	}

	.field label {
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--shell-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.field select {
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--shell-line-strong);
		border-radius: 0.25rem;
		background: var(--shell-surface);
		color: var(--shell-text);
		font-size: var(--text-sm);
		outline: none;
		transition: border-color 0.15s;
	}

	.field select:focus {
		border-color: var(--shell-accent);
	}

	.loading-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-align: center;
		padding: 1rem 0;
		margin: 0;
	}

	.form-error {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.75rem;
		background: var(--status-failed-bg);
		border: 1px solid var(--status-failed-border);
		border-radius: 0.25rem;
		color: var(--status-failed-text);
		font-size: var(--text-sm);
	}
</style>
