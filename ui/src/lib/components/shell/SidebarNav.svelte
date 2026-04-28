<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import {
		House,
		Database,
		Search,
		Loader2
	} from 'lucide-svelte';
	import { inventory } from '$lib/stores/inventory.svelte';
	import { selection } from '$lib/stores/selection.svelte';
	import { clearToken, getStoredToken } from '$lib/api/client';
	import { mutateVm, deleteVm } from '$lib/bff/vms';
	import { toast } from '$lib/stores/toast';
	import { buildInstanceActions } from '$lib/shell/instance-actions';
	import InstanceContextMenu from './InstanceContextMenu.svelte';
	import DeleteInstanceDialog from '$lib/components/vms/DeleteInstanceDialog.svelte';
	import PowerOffInstanceDialog from '$lib/components/vms/PowerOffInstanceDialog.svelte';
	import NavInfrastructureTree from './NavInfrastructureTree.svelte';
	import NavGlobalLinks from './NavGlobalLinks.svelte';
	import NavFooterControls from './NavFooterControls.svelte';
	import SidebarPinnedVms from './SidebarPinnedVms.svelte';
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

		<SidebarPinnedVms pathname={$page.url.pathname} onSelectVm={handleSelectVm} onContextMenu={handleInstanceContextMenu} />

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

</style>
