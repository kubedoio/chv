<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { onMount } from 'svelte';
	import SectionHeader from '$lib/components/shell/SectionHeader.svelte';
	import StatePanel from '$lib/components/shell/StatePanel.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import { getStoredToken } from '$lib/api/client';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/vms');
	const model = $derived(data.vms);

	onMount(() => {
		if (data.meta.clientRefreshRecommended && getStoredToken()) {
			invalidate('webui:vms');
		}
	});
</script>

<div class="resource-page">
	<SectionHeader {page} />

	{#if data.meta.deferred}
		<StatePanel
			variant="loading"
			title="Loading virtual machines"
			description="This route waits for the client-authenticated pass before loading protected VM data."
			hint="VM inventory stays shell-first while the browser rehydrates with the stored session token."
		/>
	{:else}
		{#if data.meta.partial}
			<article class="resource-page__notice">
				<div class="resource-page__eyebrow">Partial VM data</div>
				<p>Some placement or task queries did not return, so node and task context may be incomplete.</p>
			</article>
		{/if}

		<form class="resource-page__filters" method="GET">
			<label class="resource-page__field">
				<span>Search</span>
				<input type="search" name="query" value={model.filters.current.query} placeholder="VM name or node" />
			</label>
			<label class="resource-page__field">
				<span>Power state</span>
				<select name="powerState">
					<option value="all" selected={model.filters.current.powerState === 'all'}>All states</option>
					<option value="running" selected={model.filters.current.powerState === 'running'}>Running</option>
					<option value="stopped" selected={model.filters.current.powerState === 'stopped'}>Stopped</option>
					<option value="failed" selected={model.filters.current.powerState === 'failed'}>Failed</option>
				</select>
			</label>
			<label class="resource-page__field">
				<span>Health</span>
				<select name="health">
					<option value="all" selected={model.filters.current.health === 'all'}>All health</option>
					<option value="healthy" selected={model.filters.current.health === 'healthy'}>Healthy</option>
					<option value="degraded" selected={model.filters.current.health === 'degraded'}>Degraded</option>
					<option value="failed" selected={model.filters.current.health === 'failed'}>Failed</option>
					<option value="unknown" selected={model.filters.current.health === 'unknown'}>Unknown</option>
				</select>
			</label>
			<div class="resource-page__actions">
				<button type="submit">Apply filters</button>
				<a href="/vms">Reset</a>
			</div>
		</form>

		{#if model.state === 'error'}
			<StatePanel
				variant="error"
				title="VM inventory unavailable"
				description="The VM roster could not be shaped from the current control-plane responses."
				hint="The UI keeps power state, health, and task transparency ready for the next healthy refresh."
			/>
		{:else if model.state === 'empty'}
			<StatePanel
				variant="empty"
				title="No virtual machines match the current view"
				description="Widen the filters or create a VM to populate this page."
				hint="The list remains URL-backed so a filtered VM view can be shared between operators."
			/>
		{:else}
			<div class="resource-page__table-shell">
				<table class="resource-page__table">
					<thead>
						<tr>
							<th>Virtual machine</th>
							<th>Node</th>
							<th>Power state</th>
							<th>Health</th>
							<th>CPU</th>
							<th>Memory</th>
							<th>Storage</th>
							<th>Networks</th>
							<th>Tags</th>
							<th>Last task</th>
						</tr>
					</thead>
					<tbody>
						{#each model.items as item}
							<tr>
								<td><a href={item.href} class="resource-page__primary-link">{item.name}</a></td>
								<td>{item.nodeName}</td>
								<td><StatusBadge label={item.powerStateLabel} tone={item.powerStateTone} /></td>
								<td><StatusBadge label={item.healthLabel} tone={item.healthTone} /></td>
								<td>{item.cpuLabel}</td>
								<td>{item.memoryLabel}</td>
								<td>{item.storageCount}</td>
								<td>{item.networkCount}</td>
								<td>{item.tagsLabel}</td>
								<td><StatusBadge label={item.lastTaskLabel} tone={item.lastTaskTone} /></td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	{/if}
</div>

<style>
	.resource-page {
		display: grid;
		gap: 1.2rem;
	}

	.resource-page__notice,
	.resource-page__filters,
	.resource-page__table-shell {
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
	}

	.resource-page__notice,
	.resource-page__filters {
		padding: 1rem;
	}

	.resource-page__filters {
		display: grid;
		grid-template-columns: minmax(0, 1.5fr) repeat(2, minmax(0, 0.9fr)) auto;
		gap: 0.85rem;
		align-items: end;
	}

	.resource-page__field {
		display: grid;
		gap: 0.35rem;
	}

	.resource-page__eyebrow,
	.resource-page__field span {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	.resource-page__field input,
	.resource-page__field select {
		min-height: 2.75rem;
		border-radius: 0.85rem;
		border: 1px solid var(--shell-line-strong);
		background: var(--shell-surface-muted);
		padding: 0.7rem 0.8rem;
		color: var(--shell-text);
	}

	.resource-page__actions {
		display: flex;
		align-items: center;
		gap: 0.7rem;
	}

	.resource-page__actions button,
	.resource-page__actions a {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.75rem;
		padding: 0 1rem;
		border-radius: 999px;
		font-size: 0.9rem;
		font-weight: 600;
		text-decoration: none;
	}

	.resource-page__actions button {
		border: 1px solid transparent;
		background: var(--shell-accent);
		color: #fff9f2;
		cursor: pointer;
	}

	.resource-page__actions a {
		color: var(--shell-text-secondary);
	}

	.resource-page__table-shell {
		overflow-x: auto;
	}

	.resource-page__table {
		width: 100%;
		min-width: 1100px;
		border-collapse: collapse;
	}

	.resource-page__table th,
	.resource-page__table td {
		padding: 0.95rem 1rem;
		border-bottom: 1px solid var(--shell-line);
		font-size: 0.92rem;
		text-align: left;
		color: var(--shell-text-secondary);
	}

	.resource-page__table th {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
		background: rgba(247, 242, 234, 0.75);
	}

	.resource-page__primary-link {
		color: var(--shell-text);
		font-weight: 700;
		text-decoration: none;
	}

	@media (max-width: 980px) {
		.resource-page__filters {
			grid-template-columns: 1fr;
		}

		.resource-page__actions {
			justify-content: flex-start;
		}
	}
</style>
