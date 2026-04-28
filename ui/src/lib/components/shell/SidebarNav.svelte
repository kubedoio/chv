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
		Server,
		Box,
		Activity,
		Settings,
		Search,
		Loader2,
		ShieldCheck,
		AlertCircle,
		Network,
		HardDrive,
		Image,
		MoreVertical,
		Pin
	} from 'lucide-svelte';
	import { inventory } from '$lib/stores/inventory.svelte';
	import { selection } from '$lib/stores/selection.svelte';
	import { theme } from '$lib/stores/theme.svelte';
	import { clearToken, getStoredToken } from '$lib/api/client';
	import { mutateVm, deleteVm } from '$lib/bff/vms';
	import { toast } from '$lib/stores/toast';
	import { buildInstanceActions, normalizeInstanceStatus } from '$lib/shell/instance-actions';
	import InstanceStatusBadge from './InstanceStatusBadge.svelte';
	import InstanceContextMenu from './InstanceContextMenu.svelte';
	import DeleteInstanceDialog from '$lib/components/vms/DeleteInstanceDialog.svelte';
	import PowerOffInstanceDialog from '$lib/components/vms/PowerOffInstanceDialog.svelte';
	import type { InstanceTreeItem, InstanceStatus } from '$lib/api/types';

	function isActive(href: string, pathname: string): boolean {
		if (href === '/') return pathname === '/';
		return pathname === href || pathname.startsWith(`${href}/`);
	}

	let openGroups = $state<Record<string, boolean>>({
		'cloud-1': true,
		'hosts': true
	});
	let searchQuery = $state('');
	let contextMenuInstance = $state<InstanceTreeItem | null>(null);
	let contextMenuPos = $state({ x: 0, y: 0 });
	let deleteDialogInstance = $state<InstanceTreeItem | null>(null);
	let poweroffDialogInstance = $state<InstanceTreeItem | null>(null);
	let pendingAction = $state<string | null>(null);

	let contextMenuRef = $state<InstanceContextMenu | null>(null);

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
			: inventory.nodes.filter((n) =>
					n.name.toLowerCase().includes(searchQuery.toLowerCase())
				)
	);

	const filteredVms = $derived(
		searchQuery.trim() === ''
			? inventory.vms
			: inventory.vms.filter((v) =>
					v.name.toLowerCase().includes(searchQuery.toLowerCase())
				)
	);

	const vmsByNode = $derived(
		(() => {
			const map = new Map<string, typeof inventory.vms>();
			for (const vm of filteredVms) {
				const nodeId = getVmNodeId(vm);
				const list = map.get(nodeId) ?? [];
				list.push(vm);
				map.set(nodeId, list);
			}
			return map;
		})()
	);

	const pinnedVms = $derived(
		inventory.vms
			.filter((vm) => normalizeInstanceStatus(vm.actual_state) === 'running')
			.slice(0, 3)
	);

	function getNodeExpandedKey(nodeId: string): string {
		return `host-${nodeId}`;
	}

	function getVmNodeId(vm: (typeof inventory.vms)[number]): string {
		return vm.node_id ?? 'unassigned';
	}

	async function handleLogout() {
		try {
			const { createAPIClient } = await import('$lib/api/client');
			await createAPIClient().logout();
		} catch {
			// Best-effort
		} finally {
			clearToken();
			goto('/login');
		}
	}

	function handleInstanceContextMenu(event: MouseEvent, instance: InstanceTreeItem) {
		event.preventDefault();
		contextMenuInstance = instance;
		requestAnimationFrame(() => {
			contextMenuRef?.openAt(event.clientX, event.clientY);
		});
	}

	function handleKebabClick(event: MouseEvent, instance: InstanceTreeItem) {
		event.preventDefault();
		event.stopPropagation();
		const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
		contextMenuInstance = instance;
		requestAnimationFrame(() => {
			contextMenuRef?.openAt(rect.right - 8, rect.top + 8);
		});
	}

	function handleInstanceAction(actionId: string) {
		if (!contextMenuInstance) return;
		const inst = contextMenuInstance;

		switch (actionId) {
			case 'open':
				goto(`/vms/${inst.id}`);
				break;
			case 'console':
				goto(`/vms/${inst.id}?tab=console`);
				break;
			case 'start':
			case 'shutdown':
			case 'restart':
				executeLifecycleAction(inst, actionId);
				break;
			case 'poweroff':
				poweroffDialogInstance = inst;
				break;
			case 'delete':
				deleteDialogInstance = inst;
				break;
			case 'rename':
				toast.info('Rename is not yet supported');
				break;
		}
	}

	async function executeLifecycleAction(inst: InstanceTreeItem, action: string) {
		const token = getStoredToken() ?? undefined;
		pendingAction = action;
		try {
			const apiAction = action === 'shutdown' ? 'stop' : action;
			const isForce = false;
			await mutateVm({ vm_id: inst.id, action: apiAction, force: isForce }, token);
			toast.success(`${action} accepted for ${inst.name}`);
			await inventory.fetch();
		} catch (err: any) {
			toast.error(err.message || `${action} failed`);
		} finally {
			pendingAction = null;
		}
	}

	async function handleDeleteConfirm() {
		if (!deleteDialogInstance) return;
		const inst = deleteDialogInstance;
		const token = getStoredToken() ?? undefined;
		pendingAction = 'delete';
		try {
			await deleteVm({ vm_id: inst.id, requested_by: 'webui' }, token);
			toast.success(`Instance ${inst.name} deleted`);
			deleteDialogInstance = null;
			await inventory.fetch();
		} catch (err: any) {
			toast.error(err.message || 'Delete failed');
		} finally {
			pendingAction = null;
		}
	}

	async function handlePowerOffConfirm() {
		if (!poweroffDialogInstance) return;
		const inst = poweroffDialogInstance;
		const token = getStoredToken() ?? undefined;
		pendingAction = 'poweroff';
		try {
			await mutateVm({ vm_id: inst.id, action: 'stop', force: true }, token);
			toast.success(`Power off accepted for ${inst.name}`);
			poweroffDialogInstance = null;
			await inventory.fetch();
		} catch (err: any) {
			toast.error(err.message || 'Power off failed');
		} finally {
			pendingAction = null;
		}
	}

	function vmToTreeItem(vm: (typeof inventory.vms)[number]): InstanceTreeItem {
		return {
			id: vm.id,
			name: vm.name,
			nodeId: getVmNodeId(vm),
			status: normalizeInstanceStatus(vm.actual_state)
		};
	}
</script>

{#if contextMenuInstance}
	<InstanceContextMenu
		bind:this={contextMenuRef}
		actions={buildInstanceActions(contextMenuInstance.status)}
		onAction={handleInstanceAction}
		instanceName={contextMenuInstance.name}
	/>
{/if}

{#if deleteDialogInstance}
	<DeleteInstanceDialog
		bind:open={() => deleteDialogInstance !== null, (v) => { if (!v) deleteDialogInstance = null; }}
		instanceName={deleteDialogInstance.name}
		instanceId={deleteDialogInstance.id}
		onConfirm={handleDeleteConfirm}
		onCancel={() => { deleteDialogInstance = null; }}
	/>
{/if}

{#if poweroffDialogInstance}
	<PowerOffInstanceDialog
		bind:open={() => poweroffDialogInstance !== null, (v) => { if (!v) poweroffDialogInstance = null; }}
		instanceName={poweroffDialogInstance.name}
		onConfirm={handlePowerOffConfirm}
		onCancel={() => { poweroffDialogInstance = null; }}
	/>
{/if}

<nav class="flex flex-col h-full gap-4 select-none" aria-label="Primary">
	<!-- Header -->
	<div class="flex items-center gap-3 py-2 px-1">
		<div class="grid place-items-center w-8 h-8 rounded-[var(--radius-sm)] bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]">
			<Database size={16} />
		</div>
		<div class="flex flex-col">
			<div class="text-[0.875rem] font-bold text-[var(--color-sidebar-text-active,#ffffff)]">CellHV</div>
			<div class="text-[0.625rem] text-[var(--color-neutral-500)] uppercase tracking-[0.05em]">Control Plane</div>
		</div>
	</div>

	<!-- Search -->
	<div class="mx-1 flex min-h-8 items-center gap-2 rounded-[var(--radius-xs)] border border-[var(--color-neutral-700)] bg-[var(--color-neutral-800)] px-[0.625rem] text-[var(--color-neutral-400)] transition-colors duration-[120ms] ease-in-out focus-within:border-[var(--color-primary)] focus-within:text-[var(--color-sidebar-text-active,#ffffff)]">
		<Search size={12} class="shrink-0" />
		<input
			type="search"
			placeholder="Search resources..."
			class="min-w-0 flex-1 border-0 bg-transparent py-[0.35rem] px-0 text-[length:var(--text-xs)] text-[var(--color-sidebar-text-active,#ffffff)] placeholder:text-[var(--color-neutral-500)]"
			bind:value={searchQuery}
			aria-label="Search fleet resources"
		/>
	</div>

	<!-- Scrollable content -->
	<div class="flex-1 flex flex-col gap-6 overflow-y-auto pr-2 app-nav__scrollbox">
		<!-- Fleet Overview -->
		<div class="flex flex-col gap-1">
			<a
				href="/"
				class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
				aria-current={isActive('/', $page.url.pathname) ? 'page' : undefined}
			>
				<House size={14} />
				<span>Fleet Overview</span>
			</a>
		</div>

		{#if pinnedVms.length > 0}
			<div class="flex flex-col gap-1">
				<div class="text-[10px] font-bold uppercase text-[var(--color-neutral-500)] mb-1 pl-2 tracking-wider">Pinned</div>
				{#each pinnedVms as vm}
					{@const inst = vmToTreeItem(vm)}
					{@const isVmActive = isActive(`/vms/${vm.id}`, $page.url.pathname)}
					<div
						class="app-nav__pinned-row group {isVmActive ? 'app-nav__tree-link--active' : ''}"
						role="button"
						tabindex="0"
						aria-label="Pinned instance {vm.name}"
						onclick={() => { handleSelection('vm', vm.id, vm.name); goto(`/vms/${vm.id}`); }}
						onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleSelection('vm', vm.id, vm.name); goto(`/vms/${vm.id}`); } }}
						oncontextmenu={(e) => handleInstanceContextMenu(e, inst)}
					>
						<Pin size={12} />
						<span class="truncate">{vm.name}</span>
						<InstanceStatusBadge status={inst.status} showText={false} />
					</div>
				{/each}
			</div>
		{/if}

		<!-- Infrastructure -->
		<div class="flex flex-col gap-1">
			<div class="text-[10px] font-bold uppercase text-[var(--color-neutral-500)] mb-1 pl-2 tracking-wider">Infrastructure</div>

			<div class="flex flex-col pl-2">
				{#if inventory.isLoading}
					<div class="py-2 px-4 text-[10px] text-[var(--color-neutral-500)] flex items-center gap-2">
						<Loader2 size={12} class="animate-spin" />
						<span>Syncing fleet...</span>
					</div>
				{:else if filteredNodes.length === 0}
					<div class="py-2 px-4 text-[10px] text-[var(--color-neutral-500)]">No hosts enrolled.</div>
				{:else}
					<div class="flex flex-col">
						<button
							type="button"
							class="flex items-center gap-2 text-[length:var(--text-sm)] text-[var(--color-neutral-400)] no-underline bg-transparent border-none cursor-pointer rounded-[var(--radius-xs)] text-left py-1 pr-2 pl-0 hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)]"
							aria-expanded={openGroups['cloud-1']}
							aria-controls="group-cloud-1"
							onclick={() => toggleGroup('cloud-1')}
						>
							<ChevronDown size={10} class={!openGroups['cloud-1'] ? '-rotate-90' : ''} />
							<Database size={12} />
							<span>Default Cloud</span>
						</button>

						{#if openGroups['cloud-1']}
							<div id="group-cloud-1" class="ml-2 pl-2 border-l border-[var(--color-neutral-700)] flex flex-col gap-[0.125rem]">
								<div class="flex flex-col">
									<button type="button"
										class="flex items-center gap-2 text-[length:var(--text-sm)] text-[var(--color-neutral-400)] no-underline bg-transparent border-none cursor-pointer rounded-[var(--radius-xs)] text-left py-1 pr-2 pl-0 hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)]"
										aria-expanded={openGroups['hosts']}
										aria-controls="group-hosts"
										onclick={() => toggleGroup('hosts')}
									>
										<ChevronDown size={10} class={!openGroups['hosts'] ? '-rotate-90' : ''} />
										<Server size={12} />
										<span>Hosts</span>
									</button>

									{#if openGroups['hosts']}
										<div id="group-hosts" class="ml-2 pl-2 border-l border-[var(--color-neutral-700)] flex flex-col gap-[0.125rem]">
											{#each filteredNodes as node}
												{@const hostExpanded = openGroups[getNodeExpandedKey(node.id)] ?? true}
												{@const hostVms = vmsByNode.get(node.id) ?? []}
												<div class="flex flex-col">
														<button
																type="button"
																class="app-nav__tree-row app-nav__tree-row--host"
																aria-expanded={hostExpanded}
																aria-controls="group-{node.id}"
																onclick={() => toggleGroup(getNodeExpandedKey(node.id))}
													>
														<ChevronDown size={10} class={!hostExpanded ? '-rotate-90' : ''} />
														<div
															class="w-1.5 h-1.5 rounded-full {node.status === 'online' ? 'bg-[var(--color-success)]' : node.status === 'error' ? 'bg-[var(--color-danger)]' : 'bg-[var(--color-neutral-600)]'}"
															aria-hidden="true"
														></div>
														<span class="truncate">{node.name}</span>
													</button>

													{#if hostExpanded}
														<div id="group-{node.id}" class="ml-2 pl-2 border-l border-[var(--color-neutral-700)] flex flex-col gap-[0.125rem]">
															<div class="app-nav__instance-list app-nav__instance-list--direct">
																{#if hostVms.length === 0}
																	<div class="py-1 px-2 text-[10px] text-[var(--color-neutral-500)]">No instances.</div>
																{:else}
																	{#each hostVms as vm}
																		{@const inst = vmToTreeItem(vm)}
																		{@const isVmActive = isActive(`/vms/${vm.id}`, $page.url.pathname)}
																		<div
																			class="app-nav__instance-row group
																			{isVmActive ? 'app-nav__tree-link--active' : 'hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] text-[var(--color-neutral-400)]'}"
																			role="button"
																			tabindex="0"
																			onclick={() => { handleSelection('vm', vm.id, vm.name); goto(`/vms/${vm.id}`); }}
																			onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleSelection('vm', vm.id, vm.name); goto(`/vms/${vm.id}`); } }}
																			oncontextmenu={(e) => handleInstanceContextMenu(e, inst)}
																		>
																			<InstanceStatusBadge status={inst.status} showText={false} />
																			<span class="truncate flex-1 min-w-0">{vm.name}</span>
																			<button
																				type="button"
																				class="app-nav__instance-action"
																				aria-label="Actions for instance {vm.name}"
																				onclick={(e) => handleKebabClick(e, inst)}
																			>
																				<MoreVertical size={12} />
																			</button>
																		</div>
																	{/each}
																{/if}
															</div>
														</div>
													{/if}
												</div>
											{/each}
										</div>
									{/if}
								</div>
							</div>
						{/if}
					</div>

					<a
						href="/vms"
						class="app-nav__infrastructure-link {isActive('/vms', $page.url.pathname) ? 'app-nav__tree-link--active' : ''}"
						aria-current={isActive('/vms', $page.url.pathname) ? 'page' : undefined}
					>
						<span class="app-nav__tree-spacer" aria-hidden="true"></span>
						<Box size={12} />
						<span class="truncate">Instances</span>
						{#if filteredVms.length > 0}
							<span class="app-nav__tree-count">{filteredVms.length}</span>
						{/if}
					</a>
				{/if}
			</div>
		</div>

		<!-- Global -->
		<div class="flex flex-col gap-1">
			<div class="text-[10px] font-bold uppercase text-[var(--color-neutral-500)] mb-1 pl-2 tracking-wider">Global</div>

			<a
				href="/images"
				class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/images', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
				aria-current={isActive('/images', $page.url.pathname) ? 'page' : undefined}
			>
				<Image size={14} />
				<span>Images</span>
			</a>

			<a
				href="/networks"
				class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/networks', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
				aria-current={isActive('/networks', $page.url.pathname) ? 'page' : undefined}
			>
				<Network size={14} />
				<span>Networks</span>
			</a>

			<a
				href="/storage"
				class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/storage', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
				aria-current={isActive('/storage', $page.url.pathname) ? 'page' : undefined}
			>
				<HardDrive size={14} />
				<span>Storage Pools</span>
			</a>

			<a
				href="/tasks"
				class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/tasks', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
				aria-current={isActive('/tasks', $page.url.pathname) ? 'page' : undefined}
			>
				<Activity size={14} />
				<span>Tasks</span>
			</a>

			<a
				href="/events"
				class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/events', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
				aria-current={isActive('/events', $page.url.pathname) ? 'page' : undefined}
			>
				<AlertCircle size={14} />
				<span>Events</span>
			</a>

			<a
				href="/backup-jobs"
				class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/backup-jobs', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
				aria-current={isActive('/backup-jobs', $page.url.pathname) ? 'page' : undefined}
			>
				<ShieldCheck size={14} />
				<span>Backups</span>
			</a>

			<a
				href="/settings"
				class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/settings', $page.url.pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
				aria-current={isActive('/settings', $page.url.pathname) ? 'page' : undefined}
			>
				<Settings size={14} />
				<span>Settings</span>
			</a>
		</div>
	</div>

	<!-- Footer controls -->
	<div class="flex gap-2">
		<button
			type="button"
			class="w-7 h-7 grid place-items-center bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] text-[var(--color-neutral-400)] cursor-pointer transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-700)] hover:text-[var(--color-sidebar-text-active,#ffffff)]"
			onclick={() => theme.toggle()}
			aria-label="Toggle theme"
		>
			{#if theme.value === 'dark'}<Sun size={12} />{:else}<Moon size={12} />{/if}
		</button>
		<a
			href="/settings"
			class="w-7 h-7 grid place-items-center bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] text-[var(--color-neutral-400)] cursor-pointer transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-700)] hover:text-[var(--color-sidebar-text-active,#ffffff)]"
			aria-label="Settings"
		>
			<Settings size={12} />
		</a>
		<button
			type="button"
			class="w-7 h-7 grid place-items-center bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] text-[var(--color-neutral-400)] cursor-pointer transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-700)] hover:text-[var(--color-sidebar-text-active,#ffffff)]"
			onclick={handleLogout}
			aria-label="Sign out"
		>
			<LogOut size={12} />
		</button>
	</div>
</nav>

<style>
	.app-nav__scrollbox::-webkit-scrollbar {
		width: 4px;
	}

	.app-nav__tree-row {
		display: grid;
		grid-template-columns: 0.75rem 0.875rem minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.35rem;
		min-height: 1.625rem;
		padding: 0.25rem 0.45rem;
		border: 0;
		border-radius: var(--radius-xs);
		background: transparent;
		color: var(--color-neutral-400);
		cursor: pointer;
		font-size: var(--text-xs);
		text-align: left;
		text-decoration: none;
		transition:
			background-color 120ms ease-in-out,
			color 120ms ease-in-out;
	}

	.app-nav__tree-row--host {
		font-size: var(--text-sm);
	}

	.app-nav__tree-row--resource,
	.app-nav__tree-row--link {
		color: var(--color-neutral-500);
	}

	.app-nav__tree-row:hover {
		background: var(--color-neutral-800);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.app-nav__infrastructure-link {
		display: grid;
		grid-template-columns: 0.75rem 0.875rem minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.35rem;
		min-height: 1.625rem;
		padding: 0.25rem 0.45rem;
		border-radius: var(--radius-xs);
		color: var(--color-neutral-400);
		font-size: var(--text-sm);
		text-decoration: none;
		transition:
			background-color 120ms ease-in-out,
			color 120ms ease-in-out;
	}

	.app-nav__infrastructure-link:hover {
		background: var(--color-neutral-800);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.app-nav__tree-spacer {
		width: 0.75rem;
	}

	.app-nav__tree-count {
		font-size: 10px;
		color: var(--color-neutral-500);
	}

	.app-nav__pinned-row {
		display: grid;
		grid-template-columns: 0.875rem minmax(0, 1fr) 0.875rem;
		align-items: center;
		gap: 0.45rem;
		min-height: 1.75rem;
		padding: 0.25rem 0.5rem;
		border-radius: var(--radius-xs);
		color: var(--color-neutral-300);
		cursor: pointer;
		font-size: var(--text-xs);
		transition:
			background-color 120ms ease-in-out,
			color 120ms ease-in-out;
	}

	.app-nav__pinned-row:hover {
		background: var(--color-neutral-800);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.app-nav__instance-list {
		display: flex;
		flex-direction: column;
		gap: 0.125rem;
		margin-left: 1.225rem;
		padding-left: 0.45rem;
		border-left: 1px solid var(--color-neutral-700);
	}

	.app-nav__instance-list--direct {
		margin-left: 0;
	}

	.app-nav__instance-row {
		position: relative;
		display: grid;
		grid-template-columns: 0.875rem minmax(0, 1fr) 1.25rem;
		align-items: center;
		gap: 0.35rem;
		min-height: 1.625rem;
		padding: 0.25rem 0.35rem;
		border-radius: var(--radius-xs);
		font-size: var(--text-xs);
		transition:
			background-color 120ms ease-in-out,
			color 120ms ease-in-out;
	}

	.app-nav__instance-action {
		display: grid;
		place-items: center;
		width: 1.25rem;
		height: 1.25rem;
		padding: 0;
		border: 0;
		border-radius: var(--radius-xs);
		background: transparent;
		color: var(--color-neutral-400);
		cursor: pointer;
		opacity: 0;
		transition:
			opacity 120ms ease-in-out,
			background-color 120ms ease-in-out,
			color 120ms ease-in-out;
	}

	.group:hover .app-nav__instance-action,
	.group:focus-within .app-nav__instance-action {
		opacity: 1;
	}

	.app-nav__instance-action:hover {
		background: var(--color-neutral-700);
		color: var(--color-sidebar-text-active, #ffffff);
	}

	.app-nav__tree-link--active {
		background: rgba(var(--color-primary-rgb), 0.15) !important;
		color: var(--color-primary) !important;
		border-left: 2px solid var(--color-primary);
		padding-left: calc(0.5rem - 2px);
	}
</style>
