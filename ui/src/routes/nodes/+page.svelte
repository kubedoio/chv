<script lang="ts">
import Button from '$lib/components/primitives/Button.svelte';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';
	import InventoryListPage from '$lib/components/shell/InventoryListPage.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import AddNodeModal from '$lib/components/nodes/AddNodeModal.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import { Plus, AlertCircle, ShieldCheck } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { toast } from '$lib/stores/toast';
	import type { CreateNodeInput, CreateNodeResponse } from '$lib/api/types';

	let { data }: { data: PageData } = $props();

	let addNodeOpen = $state(false);
	const model = $derived(data.nodes);
	const items = $derived(model.items);

	const pageDef = getPageDefinition('/nodes');

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
		{ key: 'maintenance', label: 'Maintenance', type: 'boolean' as const }
	];

	function handleFilterChange(key: string, value: unknown) {
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
		{ key: 'name', label: 'Compute Node' },
		{ key: 'cluster', label: 'Cluster Assignment' },
		{ key: 'state', label: 'Status' },
		{ key: 'cpu', label: 'CPU Index', align: 'right' as const },
		{ key: 'memory', label: 'Memory Index', align: 'right' as const },
		{ key: 'storage', label: 'Storage Index', align: 'right' as const },
		{ key: 'hypervisor_capabilities', label: 'Host Features' },
		{ key: 'version', label: 'Platform Rev' }
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
		network: { label: item.network || 'Optimal', tone: 'healthy' as ShellTone },
		hypervisor_capabilities: item.hypervisor_capabilities?.includes('kvm') ? 'KVM' : '—'
	})));

	const attentionNodes = $derived(items.filter(n => n.health !== 'healthy' || n.alerts > 0).slice(0, 3));

	const metrics = $derived([
		{ label: 'Compute Capacity', value: items.length, color: 'neutral' as const },
		{ label: 'Operational', value: items.filter(n => n.state === 'online').length, trend: 0, color: 'primary' as const },
		{ label: 'Posture Warning', value: items.filter(n => n.health !== 'healthy').length, color: items.filter(n => n.health !== 'healthy').length > 0 ? 'warning' as const : 'neutral' as const }
	]);
</script>

<InventoryListPage
	page={pageDef}
	{filters}
	activeFilters={model.filters.current}
	{metrics}
	onFilterChange={handleFilterChange}
	onClearFilters={handleClearFilters}
	state={model.state}
	emptyTitle="No nodes detected"
	emptyDescription="Adjust your search criteria or enroll a new compute host."
	emptyHint="New hosts must be enrolled via the control-plane CLI."
	{columns}
	rows={tableRows}
	rowHref={(row) => `/nodes/${row.node_id}`}
>
	{#snippet headerActions()}
		<Button variant="primary" onclick={() => (addNodeOpen = true)}>
			<Plus size={14} />
			Enroll Node
		</Button>
	{/snippet}

	{#snippet sidebar()}
		<SectionCard title="Host Posture" icon={AlertCircle} badgeLabel={String(attentionNodes.length)}>
			{#if attentionNodes.length === 0}
				<p class="empty-hint">All compute hosts within nominal range.</p>
			{:else}
				<ul class="attention-list">
					{#each attentionNodes as node}
						<li>
							<a href="/nodes/{node.node_id}" class="attention-card">
								<div class="attention-card__main">
									<span class="res-name">{node.name}</span>
									<span class="res-issue">{node.alerts} alerts / {node.health}</span>
								</div>
							</a>
						</li>
					{/each}
				</ul>
			{/if}
		</SectionCard>

		<SectionCard title="Compute Pipeline" icon={ShieldCheck}>
			<ul class="task-list">
				<li class="task-item">
					<span class="task-label">Host Telemetry Sync</span>
					<span class="task-time">Active</span>
				</li>
				<li class="task-item">
					<span class="task-label">Policy Enforcement</span>
					<span class="task-time">Verified</span>
				</li>
			</ul>
		</SectionCard>
	{/snippet}
</InventoryListPage>

<AddNodeModal
	bind:open={addNodeOpen}
	onClose={() => (addNodeOpen = false)}
	onSubmit={async (data: CreateNodeInput): Promise<CreateNodeResponse> => {
		toast.info('Initialising node enrollment protocol...');
		return { id: 'new', name: data.name, status: 'offline' } as any;
	}}
/>

<style>
</style>
