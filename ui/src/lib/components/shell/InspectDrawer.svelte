<script lang="ts">
	import { selection } from '$lib/stores/selection.svelte';
	import { createAPIClient, getStoredToken } from '$lib/api/client';
	import { 
		Info, Activity, Settings, X, Server, Box, Database,
		ShieldCheck, Terminal, Loader2, AlertTriangle, ChevronRight
	} from 'lucide-svelte';
	import { fade } from 'svelte/transition';
  import StatusBadge from '$lib/components/shell/StatusBadge.svelte';

	const active = $derived(selection.active);
	let details = $state<any>(null);
	let metrics = $state<any>(null);
	let isLoading = $state(false);

	$effect(() => {
		if (active.id) {
			fetchDetails(active.type, active.id);
		} else {
			details = null;
			metrics = null;
		}
	});

	async function fetchDetails(type: string, id: string) {
		const token = getStoredToken();
		if (!token) return;
		
		isLoading = true;
		const client = createAPIClient({ token });
		
		try {
			if (type === 'node') {
				const [d, m] = await Promise.all([
					client.getNode(id),
					client.getNodeMetrics(id).catch(() => null)
				]);
				details = d;
				metrics = m;
			} else if (type === 'vm') {
				const [d, m] = await Promise.all([
					client.getVM(id),
					client.getVMMetrics(id).catch(() => null)
				]);
				details = d;
				metrics = m;
			}
		} catch (err) {
			console.error('Failed to fetch inspector details:', err);
		} finally {
			isLoading = false;
		}
	}
</script>

<div class="inspect-drawer">
	<header class="drawer-header">
		<div class="header-title">
			<Info size={12} />
			<span>TECH_INSPECTOR</span>
		</div>
		<button class="btn-close" onclick={() => selection.clear()}>
			<X size={12} />
		</button>
	</header>

	{#if active.id}
		<div class="drawer-content" in:fade={{duration: 100}}>
			<section class="entity-identity">
				<div class="entity-icon" class:is-node={active.type === 'node'} class:is-vm={active.type === 'vm'}>
					{#if active.type === 'node'}<Server size={18} />
					{:else if active.type === 'vm'}<Box size={18} />
					{:else}<Database size={18} />{/if}
				</div>
				<div class="entity-meta">
					<h3 class="entity-name">{active.label}</h3>
					<span class="entity-id">ID // {active.id.slice(0, 12)}</span>
				</div>
			</section>

			{#if isLoading}
				<div class="drawer-loading">
					<Loader2 size={16} class="animate-spin" />
					<span>COLLECTING_TELEMETRY...</span>
				</div>
			{:else if details}
				<div class="inspector-sections">
					<div class="section">
						<div class="label">Operational Posture</div>
						<div class="posture-card" class:is-warning={details.status !== 'online' && details.actual_state !== 'running'}>
							{#if (details.status === 'online' || details.actual_state === 'running')}
								<ShieldCheck size={14} class="text-success" />
								<div class="posture-info">
									<span class="status">HEALTH_NOMINAL</span>
									<span class="detail">Signals within expected thresholds.</span>
								</div>
							{:else}
								<AlertTriangle size={14} class="text-warning" />
								<div class="posture-info">
									<span class="status">DEGRADED_STATE</span>
									<span class="detail">Incomplete signal chain or offline.</span>
								</div>
							{/if}
						</div>
					</div>

					<div class="section">
						<div class="label">System Pulse</div>
						<div class="metric-list">
							<div class="metric-item">
								<div class="metric-meta">
									<span>CPU_PRESSURE</span>
									<span>{Math.round(metrics?.cpu_usage || 0)}%</span>
								</div>
								<div class="bar-track">
									<div class="bar-fill" style="width: {metrics?.cpu_usage || 0}%"></div>
								</div>
							</div>
							<div class="metric-item">
								<div class="metric-meta">
									<span>RAM_RESERVATION</span>
									<span>{Math.round(metrics?.memory_usage_percent || 0)}%</span>
								</div>
								<div class="bar-track">
									<div class="bar-fill" style="width: {metrics?.memory_usage_percent || 0}%"></div>
								</div>
							</div>
						</div>
					</div>

					<div class="section">
						<div class="label">Property Mesh</div>
						<div class="prop-matrix">
							<div class="prop-row">
								<span class="p-key">ARCHITECTURE</span>
								<span class="p-val">{details.architecture || 'x86_64'}</span>
							</div>
							<div class="prop-row">
								<span class="p-key">FABRIC_DOMAIN</span>
								<span class="p-val">{details.provider_type || 'LOCAL_HOST'}</span>
							</div>
              <div class="prop-row">
								<span class="p-key">VER_REGISTRY</span>
								<span class="p-val">v3.4.12-rc</span>
							</div>
						</div>
					</div>

					<div class="section">
						<div class="label">Mutation Controls</div>
						<div class="command-grid">
							<button class="cmd-btn">
								<Activity size={14} />
								<span>SYNC</span>
							</button>
							<button class="cmd-btn">
								<Terminal size={14} />
								<span>BYPASS_SHELL</span>
							</button>
							<button class="cmd-btn">
								<Settings size={14} />
								<span>CONFIG</span>
							</button>
						</div>
					</div>
          
          <a href="/{active.type}s/{active.id}" class="inspect-full-link">
            <span>FULL_INSPECTION_DETAIL</span>
            <ChevronRight size={14} />
          </a>
				</div>
			{/if}
		</div>
	{:else}
		<div class="empty-state">
			<div class="empty-icon">
				<Database size={32} />
			</div>
			<p>SELECT_INFRA_ENTITY_TO_INSPECT_TELEMETRY</p>
		</div>
	{/if}
</div>

<style>
	.inspect-drawer {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-surface);
	}
	
	.animate-spin {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.drawer-header {
		padding: 0.5rem 0.75rem;
		border-bottom: 1px solid var(--border-subtle);
		display: flex;
		justify-content: space-between;
		align-items: center;
		background: var(--bg-surface-muted);
	}

	.header-title {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 10px;
		font-weight: 800;
		color: var(--color-neutral-500);
		letter-spacing: 0.1em;
	}

	.btn-close {
		background: transparent;
		border: none;
		color: var(--color-neutral-400);
		cursor: pointer;
		padding: 0.25rem;
    display: flex;
	}

  .btn-close:hover { color: var(--color-neutral-900); }

	.drawer-content {
		flex: 1;
		padding: 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
		overflow-y: auto;
	}

	.drawer-loading {
		padding: 3rem 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.75rem;
		font-size: 10px;
		font-weight: 800;
		color: var(--color-neutral-500);
	}

	.entity-identity {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.entity-icon {
		width: 40px;
		height: 40px;
		display: grid;
		place-items: center;
		background: var(--bg-surface-muted);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-xs);
		color: var(--color-neutral-500);
	}

	.entity-icon.is-node { color: var(--color-primary); background: rgba(var(--color-primary-rgb), 0.1); }
	.entity-icon.is-vm { color: var(--color-accent); background: rgba(var(--color-accent-rgb), 0.1); }

	.entity-meta {
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
	}

	.entity-name {
		font-size: 14px;
		font-weight: 800;
		color: var(--color-neutral-900);
		margin: 0;
    line-height: 1;
	}

	.entity-id {
		font-size: 9px;
		font-weight: 700;
		color: var(--color-neutral-400);
    font-family: var(--font-mono);
	}

  .inspector-sections {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
  }

  .section .label {
    font-size: 9px;
    font-weight: 800;
    color: var(--color-neutral-500);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

	.posture-card {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem;
		background: rgba(var(--color-success-rgb), 0.08);
		border-left: 2px solid var(--color-success);
		border-radius: 2px;
	}

	.posture-card.is-warning {
		background: rgba(var(--color-warning-rgb), 0.08);
		border-left-color: var(--color-warning);
	}

	.posture-info {
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
	}

	.posture-info .status {
		font-size: 10px;
		font-weight: 800;
    color: var(--color-neutral-900);
	}

	.posture-info .detail {
		font-size: 10px;
		color: var(--color-neutral-500);
	}

  .metric-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .metric-item {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .metric-meta {
    display: flex;
    justify-content: space-between;
    font-size: 9px;
    font-weight: 800;
    color: var(--color-neutral-500);
  }

  .bar-track {
    height: 4px;
    background: var(--bg-surface-muted);
    border-radius: 2px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    background: var(--color-primary);
    transition: width 0.3s ease;
  }

  .prop-matrix {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

	.prop-row {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
    padding-bottom: 0.35rem;
		border-bottom: 1px solid var(--border-subtle);
	}

	.p-key { color: var(--color-neutral-500); font-weight: 700; }
	.p-val { font-weight: 800; color: var(--color-neutral-900); font-family: var(--font-mono); }

	.command-grid {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 0.5rem;
	}

	.cmd-btn {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.5rem;
		padding: 0.75rem 0.5rem;
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-xs);
		font-size: 9px;
		font-weight: 800;
		color: var(--color-neutral-600);
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.cmd-btn:hover {
		background: var(--bg-surface-muted);
		border-color: var(--color-primary);
		color: var(--color-primary);
	}

  .inspect-full-link {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem;
    background: rgba(var(--color-primary-rgb), 0.1);
    color: var(--color-primary);
    text-decoration: none;
    font-size: 10px;
    font-weight: 800;
    border-radius: var(--radius-xs);
    margin-top: 0.5rem;
  }

  .inspect-full-link:hover {
    background: rgba(var(--color-primary-rgb), 0.15);
  }

	.empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 2rem;
		text-align: center;
		gap: 1.5rem;
	}

	.empty-icon {
		color: var(--color-neutral-200);
	}

	.empty-state p {
		font-size: 10px;
    font-weight: 800;
    color: var(--color-neutral-300);
    letter-spacing: 0.05em;
    max-width: 200px;
    line-height: 1.5;
	}
	
	.text-success { color: var(--color-success); }
	.text-warning { color: var(--color-warning); }
</style>

<style>
	.inspect-drawer {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-surface);
	}
	
	.animate-spin {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.drawer-header {
		padding: 0.75rem;
		border-bottom: 1px solid var(--border-subtle);
		display: flex;
		justify-content: space-between;
		align-items: center;
		background: var(--bg-surface-muted);
	}

	.header-title {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 10px;
		font-weight: 700;
		text-transform: uppercase;
		color: var(--color-neutral-600);
		letter-spacing: 0.05em;
	}

	.btn-close {
		background: transparent;
		border: none;
		color: var(--color-neutral-400);
		cursor: pointer;
		padding: 2px;
	}

	.drawer-content {
		flex: 1;
		padding: 1rem;
		display: flex;
		flex-direction: column;
		gap: 1.25rem;
		overflow-y: auto;
	}

	.drawer-loading {
		padding: 2rem;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.75rem;
		font-size: 11px;
		color: var(--color-neutral-400);
	}

	.entity-identity {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.entity-icon {
		width: 36px;
		height: 36px;
		display: grid;
		place-items: center;
		background: var(--bg-surface-muted);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-sm);
		color: var(--color-neutral-500);
	}

	.entity-icon.is-node { color: var(--color-primary); background: var(--color-primary-light); }
	.entity-icon.is-vm { color: var(--color-accent); background: var(--color-accent-soft); }

	.entity-meta {
		display: flex;
		flex-direction: column;
	}

	.entity-name {
		font-size: var(--text-sm);
		font-weight: 700;
		color: var(--color-neutral-900);
		margin: 0;
	}

	.entity-type {
		font-size: 9px;
		text-transform: uppercase;
		font-weight: 600;
		color: var(--color-neutral-400);
	}

	.group-header {
		font-size: 9px;
		font-weight: 700;
		text-transform: uppercase;
		color: var(--color-neutral-500);
		margin-bottom: 0.5rem;
		letter-spacing: 0.05em;
	}

	.posture-card {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.65rem;
		background: var(--color-success-light);
		border: 1px solid var(--color-success);
		border-radius: var(--radius-sm);
	}

	.posture-card.is-warning {
		background: var(--color-warning-light);
		border-color: var(--color-warning);
	}

	.posture-info {
		display: flex;
		flex-direction: column;
	}

	.posture-info .status {
		font-size: 10px;
		font-weight: 700;
	}

	.posture-info .detail {
		font-size: 9px;
		opacity: 0.8;
	}

	.metric-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 10px;
		margin-bottom: 0.5rem;
	}

	.metric-row .label {
		width: 60px;
		color: var(--color-neutral-500);
	}

	.bar-container {
		flex: 1;
		height: 4px;
		background: var(--color-neutral-100);
		border-radius: 2px;
	}

	.bar-fill {
		height: 100%;
		background: var(--color-primary);
		border-radius: 2px;
	}

	.metric-row .value {
		width: 30px;
		text-align: right;
		font-weight: 600;
		color: var(--color-neutral-700);
	}

	.prop-row {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
		padding: 0.25rem 0;
		border-bottom: 1px solid var(--border-subtle);
	}

	.prop-label { color: var(--color-neutral-500); }
	.prop-val { font-weight: 600; color: var(--color-neutral-800); }

	.action-grid {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 0.5rem;
	}

	.action-btn {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.35rem;
		padding: 0.5rem;
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-xs);
		font-size: 9px;
		font-weight: 600;
		color: var(--color-neutral-700);
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.action-btn:hover {
		background: var(--bg-surface-muted);
		border-color: var(--color-primary);
		color: var(--color-primary);
	}

	.empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 2rem;
		text-align: center;
		color: var(--color-neutral-400);
	}

	.empty-icon {
		margin-bottom: 1rem;
		opacity: 0.2;
	}

	.empty-state p {
		font-size: 11px;
	}
	
	.text-success { color: var(--color-success); }
	.text-warning { color: var(--color-warning); }
</style>
