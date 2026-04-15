<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { PageShell, StateBanner, Badge, ResourceTable } from '$lib/components/system';
	import DetailTabs from '$lib/components/webui/DetailTabs.svelte';
	import TaskTimelineItem from '$lib/components/webui/TaskTimelineItem.svelte';
	import { normalizeTone } from '$lib/webui/formatters';
	import { mapRelatedTask } from '$lib/webui/task-helpers';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/nodes');
	const detail = $derived(data.detail);

	const summaryCards = $derived([
		{
			label: 'Hosted VMs',
			value: String(detail.hostedVms.length),
			note: 'Virtual machines on this node'
		},
		{
			label: 'Storage',
			value: detail.summary.storage,
			note: 'Storage summary for this node'
		},
		{
			label: 'Network',
			value: detail.summary.network,
			note: 'Network summary for this node'
		}
	]);

	const taskItems = $derived(
		detail.recentTasks.map((task) => mapRelatedTask(task, detail.summary.nodeId, 'node'))
	);

	const vmColumns = [
		{ key: 'name', label: 'VM' },
		{ key: 'power_state', label: 'Power state' },
		{ key: 'health', label: 'Health' },
		{ key: 'cpu', label: 'CPU' },
		{ key: 'memory', label: 'Memory' },
		{ key: 'last_task', label: 'Last task' }
	];

	const vmRows = $derived(
		detail.hostedVms.map((vm) => ({
			vm_id: vm.vm_id,
			name: vm.name,
			power_state: { label: vm.power_state, tone: normalizeTone(vm.power_state) },
			health: { label: vm.health, tone: normalizeTone(vm.health) },
			cpu: vm.cpu,
			memory: vm.memory,
			last_task: vm.last_task
		}))
	);

	function vmRowHref(row: Record<string, unknown>): string | null {
		const id = row.vm_id;
		return typeof id === 'string' ? `/vms/${id}` : null;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	<div class="detail-page">
		{#if detail.state !== 'ready'}
			<StateBanner
				variant={detail.state === 'error' ? 'error' : 'empty'}
				title={detail.state === 'error' ? 'Node detail unavailable' : 'Node not found'}
				description="The node summary, related resources, and task context could not be assembled from the current API responses."
				hint="Keep the shell active and retry once the control-plane view model becomes available."
			/>
		{:else}
			<article class="detail-page__hero">
				<div>
					<div class="detail-page__eyebrow">{detail.summary.cluster}</div>
					<h1>{detail.summary.name}</h1>
					<p>{detail.summary.nodeId}</p>
				</div>
				<div class="detail-page__hero-badges">
					<Badge label={detail.summary.state} tone={normalizeTone(detail.summary.state)} />
					<Badge label={detail.summary.health} tone={normalizeTone(detail.summary.health)} />
				</div>
			</article>

			<div class="detail-page__summary-grid">
				{#each summaryCards as card}
					<article class="detail-page__summary-card">
						<div class="detail-page__eyebrow">{card.label}</div>
						<div class="detail-page__summary-value">{card.value}</div>
						<p>{card.note}</p>
					</article>
				{/each}
			</div>

			<DetailTabs tabs={detail.sections} currentId={detail.currentTab} />

			{#if detail.currentTab === 'summary'}
				<div class="detail-page__panel-grid">
					<article class="detail-page__panel">
						<div class="detail-page__eyebrow">Alerts</div>
						<h2>Current operator signals</h2>
						<StateBanner
							variant="empty"
							title="No active node alerts"
							description="Warnings and failures tied directly to this node appear here."
							hint="Hosted VM issues remain visible in their own resource scopes."
						/>
					</article>
					<article class="detail-page__panel">
						<div class="detail-page__eyebrow">Configuration</div>
						<h2>Host-facing identifiers</h2>
						<div class="detail-page__kv-list">
							{#each detail.configuration as item}
								<div class="detail-page__kv-row">
									<div>{item.label}</div>
									<div>{item.value}</div>
								</div>
							{/each}
						</div>
					</article>
				</div>
			{:else if detail.currentTab === 'vms'}
				{#if detail.hasMoreVms}
					<StateBanner
						variant="warning"
						title="Not all VMs shown"
						description="This node hosts more than 1000 VMs. Only the first 1000 are displayed."
					/>
				{/if}
				<ResourceTable columns={vmColumns} rows={vmRows} rowHref={vmRowHref} emptyTitle="No hosted VMs" />
			{:else if detail.currentTab === 'volumes'}
				<StateBanner
					variant="empty"
					title="Volume details not yet available"
					description="Volume inventory for this node will appear once the BFF exposes storage endpoints."
					hint="Use the VM detail pages to inspect attached volumes in the meantime."
				/>
			{:else if detail.currentTab === 'networks'}
				<StateBanner
					variant="empty"
					title="Network details not yet available"
					description="Network inventory for this node will appear once the BFF exposes network endpoints."
					hint="Use the VM detail pages to inspect attached networks in the meantime."
				/>
			{:else if detail.currentTab === 'tasks'}
				<div class="detail-page__stack">
					{#if taskItems.length > 0}
						{#each taskItems as task}
							<TaskTimelineItem {task} compact />
						{/each}
					{:else}
						<StateBanner
							variant="empty"
							title="No related node tasks"
							description="Direct maintenance, scheduling, and node-scoped actions will appear here."
							hint="VM-specific work continues to live on each VM detail page and in the global task center."
						/>
					{/if}
				</div>
			{:else if detail.currentTab === 'events'}
				<StateBanner
					variant="empty"
					title="Event history not yet available"
					description="Node-scoped events will appear once the BFF exposes an event stream endpoint."
					hint="Check the global events page for fleet-wide incidents."
				/>
			{:else}
				<article class="detail-page__panel">
					<div class="detail-page__eyebrow">Configuration</div>
					<h2>Node configuration</h2>
					<div class="detail-page__kv-list">
						{#each detail.configuration as item}
							<div class="detail-page__kv-row">
								<div>{item.label}</div>
								<div>{item.value}</div>
							</div>
						{/each}
					</div>
				</article>
			{/if}
		{/if}
	</div>
</PageShell>

<style>
	.detail-page {
		display: grid;
		gap: 1.2rem;
	}

	.detail-page__hero,
	.detail-page__summary-card,
	.detail-page__panel {
		border: 1px solid var(--shell-line);
		border-radius: 1.15rem;
		background: var(--shell-surface);
	}

	.detail-page__hero,
	.detail-page__summary-card,
	.detail-page__panel {
		padding: 1rem;
	}

	.detail-page__hero {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
	}

	.detail-page__eyebrow {
		font-size: 0.74rem;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--shell-text-muted);
	}

	h1 {
		margin-top: 0.25rem;
		font-size: 2rem;
		color: var(--shell-text);
	}

	h2 {
		font-size: 1.2rem;
		color: var(--shell-text);
	}

	.detail-page__hero p,
	.detail-page__summary-card p {
		margin-top: 0.35rem;
		color: var(--shell-text-secondary);
		line-height: 1.5;
	}

	.detail-page__hero-badges {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
	}

	.detail-page__summary-grid,
	.detail-page__panel-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 1rem;
	}

	.detail-page__panel-grid {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.detail-page__summary-value {
		margin-top: 0.8rem;
		font-size: 1.5rem;
		font-weight: 700;
		color: var(--shell-text);
	}

	.detail-page__stack {
		display: grid;
		gap: 0.8rem;
	}

	.detail-page__kv-list {
		display: grid;
		gap: 0.65rem;
		margin-top: 0.9rem;
	}

	.detail-page__kv-row {
		display: grid;
		grid-template-columns: minmax(10rem, 0.75fr) minmax(0, 1fr);
		gap: 0.9rem;
		padding-bottom: 0.65rem;
		border-bottom: 1px solid var(--shell-line);
		font-size: 0.92rem;
	}

	.detail-page__kv-row div:first-child {
		color: var(--shell-text-muted);
	}





	@media (max-width: 1100px) {
		.detail-page__summary-grid,
		.detail-page__panel-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
