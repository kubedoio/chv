<script lang="ts">
	import type { PageData } from './$types';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { PageShell, StateBanner, Badge, ResourceTable, KvList } from '$lib/components/system';
	import DetailTabs from '$lib/components/webui/DetailTabs.svelte';
	import Button from '$lib/components/primitives/Button.svelte';
	import { Pause, Play, Wrench, ArrowUpFromLine } from 'lucide-svelte';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/nodes');
	const detail = $derived(data.detail);

	function normalizeTone(status: string): ShellTone {
		const s = status.toLowerCase();
		if (['healthy', 'host_ready', 'ready', 'active', 'completed', 'success', 'online'].includes(s))
			return 'healthy';
		if (['warning', 'maintenance', 'bootstrapping', 'draining', 'starting', 'stopping'].includes(s))
			return 'warning';
		if (['degraded', 'offline'].includes(s)) return 'degraded';
		if (['failed', 'error'].includes(s)) return 'failed';
		return 'unknown';
	}

	const summaryCards = $derived([
		{ label: 'Hosted VMs', value: String(detail.hostedVms.length) },
		{ label: 'CPU', value: detail.summary.cpu },
		{ label: 'Memory', value: detail.summary.memory }
	]);

	const vmColumns = [
		{ key: 'name', label: 'VM' },
		{ key: 'power_state', label: 'Power state' },
		{ key: 'health', label: 'Health' },
		{ key: 'cpu', label: 'CPU' },
		{ key: 'memory', label: 'Memory' }
	];

	const vmRows = $derived(
		detail.hostedVms.map((vm) => ({
			vm_id: vm.vm_id,
			name: vm.name,
			power_state: { label: vm.power_state, tone: normalizeTone(vm.power_state) },
			health: { label: vm.health, tone: normalizeTone(vm.health) },
			cpu: vm.cpu,
			memory: vm.memory
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
				description="The node summary and related resources could not be loaded."
			/>
		{:else}
			<article class="detail-page__hero">
				<div>
					<div class="detail-page__eyebrow">{detail.summary.cluster}</div>
					<h1>{detail.summary.name}</h1>
					<p>Node ID: {detail.summary.nodeId}</p>
				</div>
				<div class="detail-page__hero-side">
					<div class="detail-page__hero-badges">
						<Badge label={detail.summary.state} tone={normalizeTone(detail.summary.state)} />
						<Badge label={detail.summary.health} tone={normalizeTone(detail.summary.health)} />
					</div>
					<div class="detail-page__action-row">
						{#if detail.summary.maintenance}
							<Button variant="secondary" size="sm" disabled>
								<Wrench size={14} />
								Exit maintenance
							</Button>
						{:else}
							<Button variant="secondary" size="sm" disabled>
								<Wrench size={14} />
								Enter maintenance
							</Button>
						{/if}
						{#if detail.summary.scheduling}
							<Button variant="secondary" size="sm" disabled>
								<Pause size={14} />
								Pause scheduling
							</Button>
						{:else}
							<Button variant="primary" size="sm" disabled>
								<Play size={14} />
								Resume scheduling
							</Button>
						{/if}
						<Button variant="secondary" size="sm" disabled>
							<ArrowUpFromLine size={14} />
							Drain
						</Button>
					</div>
					<p class="action-hint">
						Node actions are disabled in this build. In production, scheduling and maintenance changes create tasks.
					</p>
				</div>
			</article>

			<div class="detail-page__summary-grid">
				{#each summaryCards as card}
					<article class="detail-page__summary-card">
						<div class="detail-page__eyebrow">{card.label}</div>
						<div class="detail-page__summary-value">{card.value}</div>
					</article>
				{/each}
				<article class="detail-page__summary-card">
					<div class="detail-page__eyebrow">Scheduling</div>
					<div class="detail-page__summary-value">
						{detail.summary.scheduling ? 'Enabled' : 'Paused'}
					</div>
					<Badge
						label={detail.summary.scheduling ? 'Enabled' : 'Paused'}
						tone={detail.summary.scheduling ? 'healthy' : 'warning'}
					/>
				</article>
			</div>

			<DetailTabs tabs={detail.sections} currentId={detail.currentTab} />

			{#if detail.currentTab === 'summary'}
				<div class="detail-page__panel-grid">
					<article class="detail-page__panel">
						<div class="detail-page__eyebrow">Posture</div>
						<h2>Node readiness</h2>
						<div class="detail-page__kv-list">
							<div class="detail-page__kv-row">
								<div>State</div>
								<div>{detail.summary.state}</div>
							</div>
							<div class="detail-page__kv-row">
								<div>Health</div>
								<div>{detail.summary.health}</div>
							</div>
							<div class="detail-page__kv-row">
								<div>Storage</div>
								<div>{detail.summary.storage}</div>
							</div>
							<div class="detail-page__kv-row">
								<div>Network</div>
								<div>{detail.summary.network}</div>
							</div>
						</div>
					</article>
					<article class="detail-page__panel">
						<div class="detail-page__eyebrow">Configuration</div>
						<h2>Host identifiers</h2>
						<KvList items={detail.configuration} />
					</article>
				</div>
			{:else if detail.currentTab === 'vms'}
				<ResourceTable columns={vmColumns} rows={vmRows} rowHref={vmRowHref} emptyTitle="No hosted VMs" />
			{:else if detail.currentTab === 'tasks'}
				<div class="detail-page__stack">
					{#if detail.recentTasks.length > 0}
						{#each detail.recentTasks as task}
							<div class="task-item">
								<div class="task-item__main">
									<div class="task-item__title">{task.summary}</div>
									<div class="task-item__meta">
										<Badge label={task.status} tone={normalizeTone(task.status)} />
										<span>{task.operation}</span>
									</div>
								</div>
								<a href="/tasks?query={task.task_id}" class="task-item__link">View task</a>
							</div>
						{/each}
					{:else}
						<StateBanner
							variant="empty"
							title="No related node tasks"
							description="Maintenance, scheduling, and node-scoped actions will appear here."
						/>
					{/if}
				</div>
			{:else}
				<article class="detail-page__panel">
					<div class="detail-page__eyebrow">Configuration</div>
					<h2>Node configuration</h2>
					<KvList items={detail.configuration} />
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
		padding: 1rem;
	}

	.detail-page__hero {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
	}

	.detail-page__hero-side {
		display: grid;
		gap: 0.6rem;
		max-width: 30rem;
	}

	.detail-page__hero-badges,
	.detail-page__action-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
	}

	.action-hint {
		font-size: 0.8rem;
		color: var(--shell-text-muted);
		margin: 0;
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

	.detail-page__summary-grid,
	.detail-page__panel-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
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

	.task-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.9rem 1rem;
		border: 1px solid var(--shell-line);
		border-radius: 0.9rem;
		background: var(--shell-surface-muted);
	}

	.task-item__main {
		display: grid;
		gap: 0.25rem;
	}

	.task-item__title {
		font-weight: 600;
		color: var(--shell-text);
	}

	.task-item__meta {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.85rem;
		color: var(--shell-text-muted);
	}

	.task-item__link {
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--shell-accent);
		text-decoration: none;
	}

	.task-item__link:hover {
		text-decoration: underline;
	}

	@media (max-width: 1100px) {
		.detail-page__summary-grid,
		.detail-page__panel-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
