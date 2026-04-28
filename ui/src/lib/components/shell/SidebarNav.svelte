<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import {
		House,
		Database,
		Search,
		Pin,
		Loader2
	} from 'lucide-svelte';
	import { inventory } from '$lib/stores/inventory.svelte';
	import { selection } from '$lib/stores/selection.svelte';
	import { clearToken } from '$lib/api/client';
	import { mutateVm, deleteVm } from '$lib/bff/vms';
	import { toast } from '$lib/stores/toast';
	import { buildInstanceActions, normalizeInstanceStatus } from '$lib/shell/instance-actions';
	import InstanceContextMenu from './InstanceContextMenu.svelte';
	import DeleteInstanceDialog from '$lib/components/vms/DeleteInstanceDialog.svelte';
	import PowerOffInstanceDialog from '$lib/components/vms/PowerOffInstanceDialog.svelte';
	import NavInfrastructureTree from './NavInfrastructureTree.svelte';
	import NavGlobalLinks from './NavGlobalLinks.svelte';
	import NavFooterControls from './NavFooterControls.svelte';
	import InstanceStatusBadge from './InstanceStatusBadge.svelte';
	import type { InstanceTreeItem } from '$lib/api/types';

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

	const pinnedVms = $derived(
		inventory.vms
			.filter((vm) => normalizeInstanceStatus(vm.actual_state) === 'running')
			.slice(0, 3)
	);

	function vmToTreeItem(vm: (typeof inventory.vms)[number]): InstanceTreeItem {
		return {
			id: vm.id,
			name: vm.name,
			nodeId: vm.node_id ?? 'unassigned',
			status: normalizeInstanceStatus(vm.actual_state)
		};
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

	function handleSelectVm(vmId: string, vmName: string) {
		handleSelection('vm', vmId, vmName);
		goto(`/vms/${vmId}`);
	}

	import { getStoredToken } from '$lib/api/client';
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
						onclick={() => handleSelectVm(vm.id, vm.name)}
						onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleSelectVm(vm.id, vm.name); } }}
						oncontextmenu={(e) => handleInstanceContextMenu(e, inst)}
					>
						<Pin size={12} />
						<span class="truncate">{vm.name}</span>
						<InstanceStatusBadge status={inst.status} showText={false} />
					</div>
				{/each}
			</div>
		{/if}

		<NavInfrastructureTree
			{openGroups}
			{searchQuery}
			onToggleGroup={toggleGroup}
			onSelectVm={handleSelectVm}
			onContextMenu={handleInstanceContextMenu}
			onKebabClick={handleKebabClick}
		/>

		<NavGlobalLinks />
	</div>

	<!-- Footer controls -->
	<NavFooterControls onLogout={handleLogout} />
</nav>

<style>
	.app-nav__scrollbox::-webkit-scrollbar {
		width: 4px;
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

	.app-nav__tree-link--active {
		background: rgba(var(--color-primary-rgb), 0.15) !important;
		color: var(--color-primary) !important;
		border-left: 2px solid var(--color-primary);
		padding-left: calc(0.5rem - 2px);
	}
</style>
