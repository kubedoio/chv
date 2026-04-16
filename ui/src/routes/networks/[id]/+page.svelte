<script lang="ts">
	import { PageShell, StateBanner, Badge, ResourceTable } from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';
	import type { ShellTone } from '$lib/shell/app-shell';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/networks');
	const detail = $derived(data.detail);

	function mapHealthTone(health: string): ShellTone {
		switch (health.toLowerCase()) {
			case 'healthy':
				return 'healthy';
			case 'warning':
				return 'warning';
			case 'degraded':
				return 'degraded';
			default:
				return 'unknown';
		}
	}

	function exposureTone(exposure: string): ShellTone {
		switch (exposure) {
			case 'public':
				return 'warning';
			case 'nat':
				return 'unknown';
			default:
				return 'healthy';
		}
	}

	const vmColumns = [
		{ key: 'name', label: 'VM' },
		{ key: 'ip', label: 'IP Address' }
	];

	const vmRows = $derived(
		detail.attached_vms.map((vm) => ({
			vm_id: vm.vm_id,
			name: vm.name,
			ip: vm.ip ?? 'DHCP'
		}))
	);

	function vmRowHref(row: Record<string, unknown>): string | null {
		const id = row.vm_id;
		return typeof id === 'string' ? `/vms/${id}` : null;
	}
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	{#if detail.state === 'error'}
		<StateBanner
			variant="error"
			title="Network detail unavailable"
			description="The network summary could not be loaded."
		/>
	{:else}
		<div class="detail-page">
			<article class="detail-page__hero">
				<div>
					<div class="detail-page__eyebrow">{detail.scope}</div>
					<h1>{detail.name}</h1>
					<p>Network ID: {detail.network_id}</p>
				</div>
				<div class="detail-page__hero-badges">
					<Badge label={detail.health} tone={mapHealthTone(detail.health)} />
					<Badge label={detail.exposure} tone={exposureTone(detail.exposure)} />
					{#if detail.alerts > 0}
						<Badge label="{detail.alerts} alert{detail.alerts === 1 ? '' : 's'}" tone="failed" />
					{/if}
				</div>
			</article>

			<div class="detail-page__summary-grid">
				<article class="detail-page__summary-card">
					<div class="detail-page__eyebrow">CIDR</div>
					<div class="detail-page__summary-value">{detail.cidr}</div>
					<p>Network range</p>
				</article>
				<article class="detail-page__summary-card">
					<div class="detail-page__eyebrow">Gateway</div>
					<div class="detail-page__summary-value">{detail.gateway}</div>
					<p>Default gateway</p>
				</article>
				<article class="detail-page__summary-card">
					<div class="detail-page__eyebrow">Attached VMs</div>
					<div class="detail-page__summary-value">{detail.attached_vms.length}</div>
					<p>Connected workloads</p>
				</article>
			</div>

			<div class="detail-page__panel-grid">
				<article class="detail-page__panel">
					<div class="detail-page__eyebrow">Policy</div>
					<h2>Network policy</h2>
					<div class="detail-page__kv-list">
						<div class="detail-page__kv-row">
							<div>Policy</div>
							<div>{detail.policy}</div>
						</div>
						<div class="detail-page__kv-row">
							<div>Exposure</div>
							<div>{detail.exposure}</div>
						</div>
						<div class="detail-page__kv-row">
							<div>Created</div>
							<div>{new Date(detail.created_at).toLocaleDateString('en-US')}</div>
						</div>
						<div class="detail-page__kv-row">
							<div>Last task</div>
							<div>{detail.last_task}</div>
						</div>
					</div>
				</article>

				<article class="detail-page__panel">
					<div class="detail-page__eyebrow">Attached VMs</div>
					<h2>Workloads on this network</h2>
					{#if detail.attached_vms.length > 0}
						<ResourceTable columns={vmColumns} rows={vmRows} rowHref={vmRowHref} emptyTitle="No VMs" />
					{:else}
						<StateBanner
							variant="empty"
							title="No attached VMs"
							description="This network has no workload connections."
						/>
					{/if}
				</article>
			</div>
		</div>
	{/if}
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

	.detail-page__panel {
		display: grid;
		gap: 0.95rem;
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
