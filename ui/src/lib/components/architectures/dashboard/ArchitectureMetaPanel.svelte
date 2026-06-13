<script lang="ts">
	import Button from '$lib/components/primitives/Button.svelte';
	import { architectureStore, StaleVersionError } from '$lib/stores/architecture-store.svelte';
	import type { Architecture, ArchitectureEnvironment } from '$lib/bff/architectures';

	interface Props {
		architecture: Architecture;
		onUpdated: (next: Architecture) => void;
		onStaleVersion: () => void;
	}

	let { architecture, onUpdated, onStaleVersion }: Props = $props();

	let editing = $state(false);
	let saving = $state(false);
	let draftName = $state('');
	let draftDescription = $state('');
	let draftEnvironment = $state<ArchitectureEnvironment>('development');

	function startEdit() {
		draftName = architecture.name;
		draftDescription = architecture.description;
		draftEnvironment = architecture.environment;
		editing = true;
	}

	function cancelEdit() {
		editing = false;
	}

	async function saveEdit() {
		if (saving) return;
		saving = true;
		try {
			const next = await architectureStore.update(
				architecture.id,
				architecture.version_number,
				{
					name: draftName.trim(),
					description: draftDescription.trim(),
					environment: draftEnvironment
				}
			);
			onUpdated(next);
			editing = false;
		} catch (err) {
			if (err instanceof StaleVersionError) {
				// Drop out of edit mode so the (about-to-be-refreshed) metadata
				// becomes visible and the user can re-apply their change against
				// the new version.
				editing = false;
				onStaleVersion();
			}
			// Other errors are already toasted by mutateWithRefresh.
		} finally {
			saving = false;
		}
	}

	function formatDate(iso: string): string {
		if (!iso) return '—';
		try {
			return new Date(iso).toLocaleString();
		} catch {
			return iso;
		}
	}
</script>

<section class="panel" aria-labelledby="meta-heading" data-testid="architecture-meta-panel">
	<header class="panel-header">
		<h2 id="meta-heading" class="panel-title">Metadata</h2>
		{#if !editing}
			<Button variant="secondary" size="sm" onclick={startEdit} ariaLabel="Edit metadata">
				Edit
			</Button>
		{/if}
	</header>

	{#if editing}
		<div class="form" data-testid="architecture-meta-edit-form">
			<div class="field">
				<label for="meta-name">Name</label>
				<input id="meta-name" type="text" bind:value={draftName} maxlength="64" required />
			</div>
			<div class="field">
				<label for="meta-description">Description</label>
				<textarea
					id="meta-description"
					bind:value={draftDescription}
					rows="3"
					maxlength="1024"
				></textarea>
			</div>
			<div class="field">
				<label for="meta-environment">Environment</label>
				<select id="meta-environment" bind:value={draftEnvironment}>
					<option value="development">development</option>
					<option value="staging">staging</option>
					<option value="production">production</option>
				</select>
			</div>
			<div class="actions">
				<Button variant="secondary" size="sm" onclick={cancelEdit}>Cancel</Button>
				<Button
					variant="primary"
					size="sm"
					loading={saving}
					onclick={saveEdit}
					ariaLabel="Save metadata"
				>
					Save
				</Button>
			</div>
		</div>
	{:else}
		<dl class="meta-grid">
			<div class="meta-row">
				<dt>Name</dt>
				<dd data-testid="meta-name">{architecture.name}</dd>
			</div>
			<div class="meta-row">
				<dt>Description</dt>
				<dd data-testid="meta-description">{architecture.description || '—'}</dd>
			</div>
			<div class="meta-row">
				<dt>Environment</dt>
				<dd data-testid="meta-environment">{architecture.environment}</dd>
			</div>
			<div class="meta-row">
				<dt>Status</dt>
				<dd data-testid="meta-status">{architecture.status}</dd>
			</div>
			<div class="meta-row">
				<dt>Version</dt>
				<dd data-testid="meta-version">{architecture.version_number}</dd>
			</div>
			<div class="meta-row">
				<dt>Created</dt>
				<dd>{formatDate(architecture.created_at)}</dd>
			</div>
			<div class="meta-row">
				<dt>Updated</dt>
				<dd>{formatDate(architecture.updated_at)}</dd>
			</div>
		</dl>
	{/if}
</section>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 1rem;
		background: var(--bg-surface);
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-sm);
	}

	.panel-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 0.5rem;
	}

	.panel-title {
		font-size: var(--text-sm);
		font-weight: 700;
		margin: 0;
		color: var(--color-neutral-700);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.meta-grid {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.5rem;
		margin: 0;
	}

	.meta-row {
		display: grid;
		grid-template-columns: 140px 1fr;
		gap: 0.75rem;
		font-size: var(--text-sm);
	}

	.meta-row dt {
		color: var(--color-neutral-500);
		font-weight: 600;
	}

	.meta-row dd {
		margin: 0;
		color: var(--color-neutral-900);
		word-break: break-word;
	}

	.form {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.field label {
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--color-neutral-700);
	}

	.field input,
	.field textarea,
	.field select {
		padding: 0.5rem 0.65rem;
		font-size: var(--text-sm);
		border: 1px solid var(--color-neutral-300);
		border-radius: var(--radius-xs);
		background: var(--bg-surface);
		color: var(--color-neutral-900);
		font-family: inherit;
	}

	.field input:focus,
	.field textarea:focus,
	.field select:focus {
		outline: 2px solid var(--color-primary);
		outline-offset: 1px;
		border-color: var(--color-primary);
	}

	.actions {
		display: flex;
		gap: 0.5rem;
		justify-content: flex-end;
	}
</style>
