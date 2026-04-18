<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import CompactStatStrip from '$lib/components/shell/CompactStatStrip.svelte';
	import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import FilterBar from '$lib/components/FilterBar.svelte';
	import LoadingState from '$lib/components/shell/LoadingState.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import EmptyInfrastructureState from '$lib/components/shell/EmptyInfrastructureState.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';
	import { Plus, Bell, Activity, AlertCircle, ChevronRight } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { enrollNode } from '$lib/bff/nodes';
	import { toast } from '$lib/stores/toast';

	let { data }: { data: PageData } = $props();

	const model = $derived(data.nodes);
	const items = $derived(model.items);

	const stats = $derived([
		{ label: 'Total Nodes', value: items.length },
		{ label: 'Healthy', value: items.filter(n => n.health === 'healthy' && n.state === 'online').length, status: 'healthy' as const },
		{ label: 'Degraded', value: items.filter(n => n.health !== 'healthy').length, status: items.filter(n => n.health !== 'healthy').length > 0 ? 'warning' as const : 'neutral' as const },
		{ label: 'Maintenance', value: items.filter(n => n.maintenance).length, status: items.filter(n => n.maintenance).length > 0 ? 'warning' as const : 'neutral' as const },
		{ label: 'Active Tasks', value: items.reduce((sum, n) => sum + (n.active_tasks || 0), 0), status: 'neutral' as const },
		{ label: 'Open Alerts', value: items.reduce((sum, n) => sum + (n.alerts || 0), 0), status: items.reduce((sum, n) => sum + (n.alerts || 0), 0) > 0 ? 'critical' as const : 'neutral' as const }
	]);

	const filters = [
		{ key: 'query', label: 'Search', type: 'text' as const, placeholder: 'Filter by node name or cluster...' },
		{ 
			key: 'state', 
			label: 'State', 
			type: 'select' as const, 
			options: [
				{ value: 'online', label: 'Online' },
				{ value: 'offline', label: 'Offline' },
				{ value: 'maintenance', label: 'Maintenance' },
				{ value: 'error', label: 'Error' }
			] 
		},
		{
			key: 'maintenance',
			label: 'Maintenance',
			type: 'boolean' as const
		}
	];

	function handleFilterChange(key: string, value: any) {
		const newParams = new URLSearchParams($page.url.searchParams);
		if (value === '' || value === 'all' || value === false) {
			newParams.delete(key);
		} else {
			newParams.set(key, String(value));
		}
		goto(`?${newParams.toString()}`, { keepFocus: true, noScroll: true });
	}

	function handleClearFilters() {
		goto($page.url.pathname);
	}

	const columns = [
		{ key: 'name', label: 'Node' },
		{ key: 'cluster', label: 'Cluster' },
		{ key: 'state', label: 'State' },
		{ key: 'cpu', label: 'CPU', align: 'right' as const },
		{ key: 'memory', label: 'Memory', align: 'right' as const },
		{ key: 'storage', label: 'Storage', align: 'right' as const },
		{ key: 'network', label: 'Network' },
		{ key: 'version', label: 'Version' },
		{ key: 'active_tasks', label: 'Tasks', align: 'center' as const },
		{ key: 'alerts', label: 'Alerts', align: 'center' as const }
	];

	function mapStateTone(state: string, health?: string): ShellTone {
		if (state === 'maintenance') return 'warning';
		if (state === 'error' || health === 'critical') return 'failed';
		if (health === 'warning') return 'degraded';
		if (state === 'online' && health === 'healthy') return 'healthy';
		return 'unknown';
	}

	const tableRows = $derived(items.map(item => ({
		...item,
		state: { label: item.state, tone: mapStateTone(item.state, item.health) },
		network: { label: item.network || 'Optimal', tone: 'healthy' as ShellTone }
	})));

	const attentionNodes = $derived(items.filter(n => n.health !== 'healthy' || n.alerts > 0).slice(0, 3));
	const nodePageDef = getPageDefinition('/nodes');
</script>

<div class="nodes-page">
	<PageHeaderWithAction page={nodePageDef}>
		{#snippet actions()}
			<button
				class="btn-primary"
				onclick={async () => {
					try {
						const result = await enrollNode();
						if (result.success) {
							toast.success(result.message);
						} else {
							toast.error(result.message);
						}
					} catch (e) {
						const msg = e instanceof Error ? e.message : 'Enrollment request failed';
						toast.error(msg);
					}
				}}
			>
				<Plus size={14} />
				Enroll Node
			</button>
		{/snippet}
	</PageHeaderWithAction>

	<div class="posture-strip-wrapper">
		<CompactStatStrip {stats} />
	</div>

	<div class="inventory-controls">
		<FilterBar 
			{filters} 
			activeFilters={model.filters.current} 
			onFilterChange={handleFilterChange}
			onClearAll={handleClearFilters}
		/>
	</div>

	<main class="inventory-main">
		<section class="inventory-table-area">
			{#if model.state === 'error'}
				<ErrorState />
			{:else if model.state === 'empty'}
				<EmptyInfrastructureState 
					title="No nodes match your search"
					description="Adjust your filters or enroll a new compute host to view inventory."
					hint="If this is a new installation, follow the enrollment CLI instructions."
				/>
			{:else}
				<InventoryTable 
					{columns} 
					rows={tableRows} 
					rowHref={(row) => `/nodes/${row.node_id}`} 
				/>
			{/if}
		</section>

		<aside class="support-area">
			<div class="support-panel">
				<div class="support-panel__header">
					<AlertCircle size={14} style="color: var(--color-danger)" />
					<h3>Needs Attention</h3>
				</div>
				<div class="support-panel__content">
					{#if attentionNodes.length === 0}
						<p class="empty-hint">All nodes operating within normal parameters.</p>
					{:else}
						<ul class="attention-list">
							{#each attentionNodes as node}
								<li>
									<a href="/nodes/{node.node_id}" class="attention-card">
										<div class="attention-card__main">
											<span class="node-name">{node.name}</span>
											<span class="node-issue">{node.alerts} active alerts</span>
										</div>
										<ChevronRight size={14} />
									</a>
								</li>
							{/each}
						</ul>
					{/if}
				</div>
			</div>

			<div class="support-panel">
				<div class="support-panel__header">
					<Activity size={14} />
					<h3>Recent Tasks</h3>
				</div>
				<div class="support-panel__content">
					<ul class="task-list">
						<li>
							<div class="task-item">
								<span class="task-label">OS Update</span>
								<span class="task-time">2m ago</span>
							</div>
						</li>
						<li>
							<div class="task-item">
								<span class="task-label">Storage Rebalance</span>
								<span class="task-time">15m ago</span>
							</div>
						</li>
					</ul>
				</div>
			</div>
		</aside>
	</main>
</div>

<style>
	.nodes-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.posture-strip-wrapper {
		margin-top: -0.25rem;
	}

	.inventory-controls {
		border: 1px solid var(--shell-line);
		border-radius: 0.35rem;
		overflow: hidden;
	}

	.inventory-main {
		display: grid;
		grid-template-columns: 1fr 280px;
		gap: 1rem;
		align-items: start;
	}

	.support-area {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.support-panel {
		background: var(--shell-surface);
		border: 1px solid var(--shell-line);
		border-radius: 0.35rem;
		padding: 0.75rem;
	}

	.support-panel__header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-bottom: 0.75rem;
		border-bottom: 1px solid var(--shell-line);
		padding-bottom: 0.5rem;
	}

	.support-panel__header h3 {
		font-size: var(--text-xs);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--shell-text-muted);
	}

	.empty-hint {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		padding: 0.5rem 0;
	}

	.attention-list, .task-list {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.attention-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.5rem;
		background: var(--shell-surface-muted);
		border-radius: 0.25rem;
		text-decoration: none;
		color: var(--shell-text);
		transition: background 0.15s ease;
	}

	.attention-card:hover {
		background: var(--shell-line);
	}

	.attention-card__main {
		display: flex;
		flex-direction: column;
	}

	.node-name {
		font-size: var(--text-sm);
		font-weight: 600;
	}

	.node-issue {
		font-size: var(--text-xs);
		color: var(--color-danger-dark);
	}

	.task-item {
		display: flex;
		justify-content: space-between;
		font-size: var(--text-sm);
		padding: 0.25rem 0.5rem;
	}

	.task-time {
		color: var(--shell-text-muted);
		font-size: var(--text-xs);
	}

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}
</style>
