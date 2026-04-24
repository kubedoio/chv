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

<nav class="flex flex-col h-full gap-4 select-none" aria-label="Primary">
	<div class="flex items-center gap-3 py-2 px-1">
		<div class="grid place-items-center w-8 h-8 rounded-[var(--radius-sm)] bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]">
			<Database size={16} />
		</div>
		<div class="flex flex-col">
			<div class="text-[0.875rem] font-bold text-[var(--color-sidebar-text-active,#ffffff)]">Control Plane</div>
			<div class="text-[0.625rem] text-[var(--color-neutral-400)] uppercase tracking-[0.05em]">Topology First</div>
		</div>
	</div>

	<div class="relative mx-1">
		<Search size={12} class="absolute left-[0.625rem] top-1/2 -translate-y-1/2 text-[var(--color-neutral-400)]" />
		<input
			type="text"
			placeholder="Search fleet..."
			class="w-full bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] py-[0.35rem] pr-2 pl-8 text-[length:var(--text-xs)] text-[var(--color-sidebar-text-active,#ffffff)]"
			bind:value={searchQuery}
			aria-label="Search fleet nodes and VMs"
		/>
	</div>

	<div class="flex-1 flex flex-col gap-6 overflow-y-auto pr-2 app-nav__scrollbox">
		<div class="flex flex-col gap-1">
			<a href="/" class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}" aria-current={isActive('/', $page.url.pathname) ? 'page' : undefined}>
				<House size={14} />
				<span>Fleet Overview</span>
			</a>
		</div>

		<div class="flex flex-col gap-1">
			<div class="text-[10px] font-bold uppercase text-[var(--color-neutral-500)] mb-1 pl-2">Infrastructure</div>
			
			<div class="flex flex-col pl-2">
				{#if inventory.isLoading}
					<div class="py-2 px-4 text-[10px] text-[var(--color-neutral-500)] flex items-center gap-2">
						<Loader2 size={12} class="animate-spin" />
						<span>Syncing fleet...</span>
					</div>
				{:else if inventory.nodes.length === 0}
					<div class="py-2 px-4 text-[10px] text-[var(--color-neutral-500)]">No nodes index.</div>
				{:else}
					<div class="flex flex-col">
						<button class="flex items-center gap-2 text-[length:var(--text-sm)] text-[var(--color-neutral-400)] no-underline bg-transparent border-none cursor-pointer rounded-[var(--radius-xs)] text-left py-1 pr-2 pl-0 hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)]" aria-expanded={openGroups['dc-1']} onclick={() => toggleGroup('dc-1')}>
							<ChevronDown size={10} class={!openGroups['dc-1'] ? '-rotate-90' : ''} />
							<Database size={12} />
							<span>Default-DC</span>
						</button>
						
						{#if openGroups['dc-1']}
							<div class="ml-2 pl-2 border-l border-[var(--color-neutral-700)] flex flex-col gap-[0.125rem]">
								{#each filteredNodes as node}
									<div class="flex flex-col">
										<a 
											href="/nodes/{node.id}" 
											class="flex items-center gap-2 text-[length:var(--text-sm)] text-[var(--color-neutral-400)] no-underline bg-transparent border-none cursor-pointer rounded-[var(--radius-xs)] text-left py-1 px-2 hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {selection.active.id === node.id ? 'app-nav__tree-link--active' : ''}"
											onclick={() => handleSelection('node', node.id, node.name)}
										>
											<div class="w-1 h-1 rounded-full {node.status === 'online' ? 'bg-[var(--color-success)]' : 'bg-[var(--color-neutral-600)]'}"></div>
											<Server size={12} />
											<span>{node.name}</span>
										</a>
										
										<div class="ml-2 pl-2 border-l border-[var(--color-neutral-700)] flex flex-col gap-[0.125rem]">
											{#each filteredVms(node.id) as vm}
												<a 
													href="/vms/{vm.id}" 
													class="flex items-center gap-2 text-[length:var(--text-sm)] text-[var(--color-neutral-400)] no-underline bg-transparent border-none cursor-pointer rounded-[var(--radius-xs)] text-left py-1 px-2 hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {selection.active.id === vm.id ? 'app-nav__tree-link--active' : ''}"
													onclick={() => handleSelection('vm', vm.id, vm.name)}
												>
													<div class="w-1 h-1 rounded-full {vm.actual_state === 'running' ? 'bg-[var(--color-success)]' : 'bg-[var(--color-warning)]'}"></div>
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

		<div class="flex flex-col gap-1">
			<div class="text-[10px] font-bold uppercase text-[var(--color-neutral-500)] mb-1 pl-2">Resources</div>
			<a href="/networks" class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/networks', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}" aria-current={isActive('/networks', $page.url.pathname) ? 'page' : undefined}>
				<Activity size={14} />
				<span>Network Fabric</span>
			</a>
			<a href="/storage" class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/storage', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}" aria-current={isActive('/storage', $page.url.pathname) ? 'page' : undefined}>
				<Database size={14} />
				<span>Storage Pools</span>
			</a>
			<a href="/images" class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/images', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}" aria-current={isActive('/images', $page.url.pathname) ? 'page' : undefined}>
				<Blocks size={14} />
				<span>Image Library</span>
			</a>
		</div>

		<div class="flex flex-col gap-1">
			<div class="text-[10px] font-bold uppercase text-[var(--color-neutral-500)] mb-1 pl-2">Operations</div>
			<a href="/tasks" class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/tasks', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}" aria-current={isActive('/tasks', $page.url.pathname) ? 'page' : undefined}>
				<Activity size={14} />
				<span>Operation Pipeline</span>
			</a>
			<a href="/events" class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/events', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}" aria-current={isActive('/events', $page.url.pathname) ? 'page' : undefined}>
				<AlertCircle size={14} />
				<span>Incident Log</span>
			</a>
			<a href="/backups" class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/backups', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}" aria-current={isActive('/backups', $page.url.pathname) ? 'page' : undefined}>
				<ShieldCheck size={14} />
				<span>Data Protection</span>
			</a>
		</div>
	</div>

	<div class="flex gap-2">
		<button type="button" class="w-7 h-7 grid place-items-center bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] text-[var(--color-neutral-400)] cursor-pointer transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-700)] hover:text-[var(--color-sidebar-text-active,#ffffff)]" onclick={() => theme.toggle()}>
			{#if theme.value === 'dark'}<Sun size={12} />{:else}<Moon size={12} />{/if}
		</button>
		<a href="/settings" class="w-7 h-7 grid place-items-center bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] text-[var(--color-neutral-400)] cursor-pointer transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-700)] hover:text-[var(--color-sidebar-text-active,#ffffff)]"><Settings size={12} /></a>
		<button type="button" class="w-7 h-7 grid place-items-center bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] text-[var(--color-neutral-400)] cursor-pointer transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-700)] hover:text-[var(--color-sidebar-text-active,#ffffff)]" onclick={handleLogout}><LogOut size={12} /></button>
	</div>
</nav>

<style>
	.app-nav__scrollbox::-webkit-scrollbar {
		width: 4px;
	}

	.app-nav__tree-link--active {
		background: rgba(143, 90, 42, 0.15) !important;
		color: var(--color-primary) !important;
		border-left: 2px solid var(--color-primary);
		padding-left: calc(0.5rem - 2px);
	}
</style>
