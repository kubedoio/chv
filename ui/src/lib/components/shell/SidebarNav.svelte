<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { 
		ChevronDown, 
		House, 
		LogOut, 
		Moon, 
		Sun,
		Database,
		Blocks,
		Server,
		Box,
		Activity,
		Settings,
		Search,
		Loader2,
		ShieldCheck,
		AlertCircle
	} from 'lucide-svelte';
	import { navigationGroups } from '$lib/shell/app-shell';
	import { clearToken, createAPIClient, getStoredToken } from '$lib/api/client';
	import { theme } from '$lib/stores/theme.svelte';
	import { inventory } from '$lib/stores/inventory.svelte';
	import { selection } from '$lib/stores/selection.svelte';

	function isActive(href: string, pathname: string): boolean {
		if (href === '/') return pathname === '/';
		return pathname === href || pathname.startsWith(`${href}/`);
	}

	let openGroups = $state<Record<string, boolean>>({
		'dc-1': true,
		'cl-1': true,
		'nodes': true
	});
	let searchQuery = $state('');

	onMount(() => {
		inventory.fetch();
	});

	function toggleGroup(label: string) {
		openGroups[label] = !openGroups[label];
	}

	function handleSelection(type: any, id: string, label: string) {
		selection.select(type, id, label);
	}

	const filteredNodes = $derived(
		searchQuery.trim() === ''
			? inventory.nodes
			: inventory.nodes.filter(n =>
					n.name.toLowerCase().includes(searchQuery.toLowerCase())
				)
	);

	function filteredVms(nodeId: string) {
		const vms = inventory.vms.filter(v => v.node_id === nodeId);
		if (searchQuery.trim() === '') return vms;
		return vms.filter(v =>
			v.name.toLowerCase().includes(searchQuery.toLowerCase())
		);
	}

	async function handleLogout() {
		try {
			await createAPIClient().logout();
		} catch {
			// Best-effort
		} finally {
			clearToken();
			goto('/login');
		}
	}
</script>

<nav class="app-nav" aria-label="Primary">
	<div class="app-nav__brand">
		<div class="app-nav__brand-mark">
			<Database size={16} />
		</div>
		<div class="app-nav__brand-text">
			<div class="app-nav__brand-title">Control Plane</div>
			<div class="app-nav__brand-subtitle">Topology First</div>
		</div>
	</div>

		<div class="app-nav__search">
			<Search size={12} class="app-nav__search-icon" />
			<input
				type="text"
				placeholder="Search fleet..."
				class="app-nav__search-input"
				bind:value={searchQuery}
				aria-label="Search fleet nodes and VMs"
			/>
		</div>

	<div class="app-nav__scrollbox">
		<div class="app-nav__section">
			<a href="/" class="app-nav__item" class:app-nav__item--active={isActive('/', $page.url.pathname)} aria-current={isActive('/', $page.url.pathname) ? 'page' : undefined}>
				<House size={14} />
				<span>Fleet Overview</span>
			</a>
		</div>

		<div class="app-nav__section">
			<div class="app-nav__section-header">Infrastructure</div>
			
			<div class="app-nav__tree">
				{#if inventory.isLoading}
					<div class="app-nav__loading">
						<Loader2 size={12} class="animate-spin" />
						<span>Syncing fleet...</span>
					</div>
				{:else if inventory.nodes.length === 0}
					<div class="app-nav__empty">No nodes index.</div>
				{:else}
					<!-- Live Datacenter (Placeholder for multi-dc expansion) -->
					<div class="app-nav__tree-node app-nav__tree-node--dc">
						<button class="app-nav__tree-toggle" aria-expanded={openGroups['dc-1']} onclick={() => toggleGroup('dc-1')}>
							<ChevronDown size={10} class={!openGroups['dc-1'] ? 'is-closed' : ''} />
							<Database size={12} />
							<span>Default-DC</span>
						</button>
						
						{#if openGroups['dc-1']}
							<div class="app-nav__tree-children">
								{#each filteredNodes as node}
									<div class="app-nav__tree-node app-nav__tree-node--node">
										<a 
											href="/nodes/{node.id}" 
											class="app-nav__tree-link" 
											class:app-nav__tree-link--active={selection.active.id === node.id}
											onclick={() => handleSelection('node', node.id, node.name)}
										>
											<div class="status-status-orb" class:status-status-orb--healthy={node.status === 'online'}></div>
											<Server size={12} />
											<span>{node.name}</span>
										</a>
										
										<div class="app-nav__tree-children">
											{#each filteredVms(node.id) as vm}
												<a 
													href="/vms/{vm.id}" 
													class="app-nav__tree-link app-nav__tree-link--vm"
													class:app-nav__tree-link--active={selection.active.id === vm.id}
													onclick={() => handleSelection('vm', vm.id, vm.name)}
												>
													<div class="status-status-orb" class:status-status-orb--healthy={vm.actual_state === 'running'} class:status-status-orb--warning={vm.actual_state !== 'running'}></div>
													<Box size={10} />
													<span>{vm.name}</span>
												</a>
											{/each}
										</div>
									</div>
								{/each}
							</div>
						{/if}
					</div>
				{/if}
			</div>
		</div>

		<div class="app-nav__section">
			<div class="app-nav__section-header">Resources</div>
			<a href="/networks" class="app-nav__item" class:app-nav__item--active={isActive('/networks', $page.url.pathname)} aria-current={isActive('/networks', $page.url.pathname) ? 'page' : undefined}>
				<Activity size={14} />
				<span>Network Fabric</span>
			</a>
			<a href="/storage" class="app-nav__item" class:app-nav__item--active={isActive('/storage', $page.url.pathname)} aria-current={isActive('/storage', $page.url.pathname) ? 'page' : undefined}>
				<Database size={14} />
				<span>Storage Pools</span>
			</a>
			<a href="/images" class="app-nav__item" class:app-nav__item--active={isActive('/images', $page.url.pathname)} aria-current={isActive('/images', $page.url.pathname) ? 'page' : undefined}>
				<Blocks size={14} />
				<span>Image Library</span>
			</a>
		</div>

		<div class="app-nav__section">
			<div class="app-nav__section-header">Operations</div>
			<a href="/tasks" class="app-nav__item" class:app-nav__item--active={isActive('/tasks', $page.url.pathname)} aria-current={isActive('/tasks', $page.url.pathname) ? 'page' : undefined}>
				<Activity size={14} />
				<span>Operation Pipeline</span>
			</a>
			<a href="/events" class="app-nav__item" class:app-nav__item--active={isActive('/events', $page.url.pathname)} aria-current={isActive('/events', $page.url.pathname) ? 'page' : undefined}>
				<AlertCircle size={14} />
				<span>Incident Log</span>
			</a>
			<a href="/backups" class="app-nav__item" class:app-nav__item--active={isActive('/backups', $page.url.pathname)} aria-current={isActive('/backups', $page.url.pathname) ? 'page' : undefined}>
				<ShieldCheck size={14} />
				<span>Data Protection</span>
			</a>
		</div>
	</div>

	<div class="app-nav__footer">
		<button type="button" class="app-nav__footer-btn" onclick={() => theme.toggle()}>
			{#if theme.value === 'dark'}<Sun size={12} />{:else}<Moon size={12} />{/if}
		</button>
		<a href="/settings" class="app-nav__footer-btn"><Settings size={12} /></a>
		<button type="button" class="app-nav__footer-btn" onclick={handleLogout}><LogOut size={12} /></button>
	</div>
</nav>

<style>
	.app-nav {
		display: flex;
		flex-direction: column;
		height: 100%;
		gap: 1rem;
		user-select: none;
	}

	.app-nav__brand {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 0.25rem;
	}

	.app-nav__brand-mark {
		display: grid;
		place-items: center;
		width: 2rem;
		height: 2rem;
		border-radius: var(--radius-sm);
		background: var(--color-primary);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.app-nav__brand-text {
		display: flex;
		flex-direction: column;
	}

	.app-nav__brand-title {
		font-size: 0.875rem;
		font-weight: 700;
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.app-nav__brand-subtitle {
		font-size: 0.625rem;
		color: var(--color-neutral-400);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.app-nav__search {
		position: relative;
		margin: 0 0.25rem;
	}

	.app-nav__search-icon {
		position: absolute;
		left: 0.625rem;
		top: 50%;
		transform: translateY(-50%);
		color: var(--color-neutral-400);
	}

	.app-nav__search-input {
		width: 100%;
		background: var(--color-neutral-800);
		border: 1px solid var(--color-neutral-700);
		border-radius: var(--radius-xs);
		padding: 0.35rem 2rem;
		font-size: var(--text-xs);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.app-nav__search-kbd {
		position: absolute;
		right: 0.625rem;
		top: 50%;
		transform: translateY(-50%);
		font-size: 9px;
		background: var(--color-neutral-700);
		color: var(--color-neutral-400);
		padding: 1px 3px;
		border-radius: 2px;
	}

	.app-nav__scrollbox {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
		overflow-y: auto;
		padding-right: 0.5rem;
	}

	.app-nav__section {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.app-nav__section-header {
		font-size: 10px;
		font-weight: 700;
		text-transform: uppercase;
		color: var(--color-neutral-500);
		margin-bottom: 0.25rem;
		padding-left: 0.5rem;
	}

	.app-nav__item {
		display: flex;
		align-items: center;
		gap: 0.625rem;
		padding: 0.35rem 0.5rem;
		font-size: var(--text-sm);
		color: var(--color-neutral-300);
		text-decoration: none;
		border-radius: var(--radius-xs);
		transition: all 120ms ease;
	}

	.app-nav__item:hover {
		background: var(--color-neutral-800);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.app-nav__item--active {
		background: var(--color-primary);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	/* Tree View */
	.app-nav__tree {
		display: flex;
		flex-direction: column;
		padding-left: 0.5rem;
	}

	.app-nav__tree-node {
		display: flex;
		flex-direction: column;
	}

	.app-nav__tree-toggle,
	.app-nav__tree-link {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: var(--text-sm);
		color: var(--color-neutral-400);
		text-decoration: none;
		background: transparent;
		border: none;
		cursor: pointer;
		border-radius: var(--radius-xs);
		text-align: left;
	}

	.app-nav__tree-toggle {
		padding: 0.25rem 0.5rem 0.25rem 0;
	}

	.app-nav__tree-link {
		padding: 0.25rem 0.5rem;
	}

	.app-nav__tree-toggle:hover,
	.app-nav__tree-link:hover {
		background: var(--color-neutral-800);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.app-nav__tree-children {
		margin-left: 0.5rem;
		padding-left: 0.5rem;
		border-left: 1px solid var(--color-neutral-700);
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
	}

	.status-pulse {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--color-neutral-500);
		position: relative;
	}

	.app-nav__tree-link--active {
		background: rgba(143, 90, 42, 0.15) !important;
		color: var(--color-primary) !important;
		border-left: 2px solid var(--color-primary);
		padding-left: calc(0.5rem - 2px);
	}

	.status-status-orb {
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: var(--color-neutral-600);
	}

	.status-status-orb--healthy { background: var(--color-success); }
	.status-status-orb--warning { background: var(--color-warning); }

	.app-nav__scrollbox::-webkit-scrollbar {
		width: 4px;
	}
	.app-nav__loading,
	.app-nav__empty {
		padding: 0.5rem 1rem;
		font-size: 10px;
		color: var(--color-neutral-500);
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.animate-spin {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.is-closed {
		transform: rotate(-90deg);
	}

	.app-nav__footer-btn {
		width: 1.75rem;
		height: 1.75rem;
		display: grid;
		place-items: center;
		background: var(--color-neutral-800);
		border: 1px solid var(--color-neutral-700);
		border-radius: var(--radius-xs);
		color: var(--color-neutral-400);
		cursor: pointer;
		transition: all 120ms ease;
	}

	.app-nav__footer-btn:hover {
		background: var(--color-neutral-700);
		color: var(--color-sidebar-text-active, #ffffff);
	}
</style>
