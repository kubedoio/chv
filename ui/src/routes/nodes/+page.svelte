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

	const page = getPageDefinition('/nodes');
	const model = $derived(data.nodes);
	const healthyCount = $derived(model.items.filter((item) => item.healthTone === 'healthy').length);
	const degradedCount = $derived(
		model.items.filter((item) => item.healthTone === 'degraded' || item.healthTone === 'failed').length
	);

	onMount(() => {
		if (data.meta.clientRefreshRecommended && getStoredToken()) {
			invalidate('webui:nodes');
		}
	});
</script>

<div class="resource-page">
	<SectionHeader {page} />

	{#if data.meta.deferred}
		<StatePanel
			variant="loading"
			title="Loading node inventory"
			description="This route waits for the client-authenticated pass before loading protected node data."
			hint="Node summaries stay shell-first while the browser rehydrates with the stored session token."
		/>
	{:else}
		{#if data.meta.partial}
			<article class="resource-page__notice">
				<div class="resource-page__eyebrow">Partial node data</div>
				<p>Some node-adjacent queries did not return, so telemetry and task context may be incomplete.</p>
			</article>
		{/if}

		<form class="resource-page__filters" method="GET">
			<label class="resource-page__field">
				<span>Search</span>
				<input type="search" name="query" value={model.filters.current.query} placeholder="Node name or cluster" />
			</label>
			<label class="resource-page__field">
				<span>State</span>
				<select name="state">
					<option value="all" selected={model.filters.current.state === 'all'}>All states</option>
					<option value="online" selected={model.filters.current.state === 'online'}>Online</option>
					<option value="maintenance" selected={model.filters.current.state === 'maintenance'}>Maintenance</option>
					<option value="offline" selected={model.filters.current.state === 'offline'}>Offline</option>
					<option value="error" selected={model.filters.current.state === 'error'}>Error</option>
				</select>
			</label>
			<label class="resource-page__field">
				<span>Maintenance</span>
				<select name="maintenance">
					<option value="all" selected={model.filters.current.maintenance === 'all'}>All nodes</option>
					<option value="true" selected={model.filters.current.maintenance === 'true'}>In maintenance</option>
					<option value="false" selected={model.filters.current.maintenance === 'false'}>Scheduling enabled</option>
				</select>
			</label>
			<div class="resource-page__actions">
				<button type="submit">Apply filters</button>
				<a href="/nodes">Reset</a>
			</div>
		</form>

		<div class="resource-page__summary">
			<div>
				<div class="resource-page__eyebrow">Visible nodes</div>
				<div class="resource-page__summary-value">{model.items.length}</div>
			</div>
			<div class="resource-page__badges">
				<StatusBadge label={`${healthyCount} healthy`} tone="healthy" />
				<StatusBadge label={`${degradedCount} degraded`} tone={degradedCount > 0 ? 'failed' : 'unknown'} />
				{#if Object.keys(model.filters.applied).length > 0}
					<StatusBadge label="filters applied" tone="unknown" />
				{/if}
			</div>
		</div>

		{#if model.state === 'error'}
			<StatePanel
				variant="error"
				title="Node inventory unavailable"
				description="The control-plane view model for nodes could not be assembled from the current responses."
				hint="The shell remains usable while the page waits for a healthy refresh."
			/>
		{:else if model.state === 'empty'}
			<StatePanel
				variant="empty"
				title="No nodes match the current view"
				description="Try widening the search or state filters, or enroll a compute host to populate this page."
				hint="Node list filters stay URL-backed so operators can share a filtered view."
			/>
		{:else}
			<div class="resource-page__table-shell">
				<table class="resource-page__table">
					<thead>
						<tr>
							<th>Node</th>
							<th>Cluster</th>
							<th>State</th>
							<th>CPU usage</th>
							<th>Memory usage</th>
							<th>Storage summary</th>
							<th>Network health</th>
							<th>Version</th>
							<th>Maintenance</th>
						</tr>
					</thead>
					<tbody>
						{#each model.items as item}
							<tr>
								<td>
									<a href={item.href} class="resource-page__primary-link">{item.name}</a>
								</td>
								<td>{item.cluster}</td>
								<td><StatusBadge label={item.stateLabel} tone={item.stateTone} /></td>
								<td>{item.cpuLabel}</td>
								<td>{item.memoryLabel}</td>
								<td>{item.storageLabel}</td>
								<td><StatusBadge label={item.healthLabel} tone={item.healthTone} /></td>
								<td>{item.versionLabel}</td>
								<td><StatusBadge label={item.maintenanceLabel} tone={item.maintenanceTone} /></td>
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
	.resource-page__summary,
	.resource-page__table-shell {
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
	}

	.resource-page__notice,
	.resource-page__filters,
	.resource-page__summary {
		padding: 1rem;
	}

	.resource-page__filters {
		display: grid;
		grid-template-columns: minmax(0, 1.4fr) repeat(2, minmax(0, 0.9fr)) auto;
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

	.resource-page__summary {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 1rem;
		align-items: center;
	}

	.resource-page__summary-value {
		margin-top: 0.2rem;
		font-size: 1.5rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.resource-page__badges {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 0.6rem;
	}

	.resource-page__table-shell {
		overflow-x: auto;
	}

	.resource-page__table {
		width: 100%;
		border-collapse: collapse;
		min-width: 980px;
	}

	.resource-page__table th,
	.resource-page__table td {
		padding: 0.95rem 1rem;
		border-bottom: 1px solid var(--shell-line);
		font-size: 0.92rem;
		text-align: left;
		color: var(--shell-text-secondary);
		vertical-align: middle;
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

	.resource-page__primary-link:hover {
		color: var(--shell-accent);
	}

	@media (max-width: 980px) {
		.resource-page__filters,
		.resource-page__summary {
			grid-template-columns: 1fr;
		}

		.resource-page__badges,
		.resource-page__actions {
			justify-content: flex-start;
		}
	}
</style>
