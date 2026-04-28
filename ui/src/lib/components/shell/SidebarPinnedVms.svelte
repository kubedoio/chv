<script lang="ts">
	import { Pin } from 'lucide-svelte';
	import InstanceStatusBadge from './InstanceStatusBadge.svelte';
	import { normalizeInstanceStatus } from '$lib/shell/instance-actions';
	import { inventory } from '$lib/stores/inventory.svelte';
	import type { InstanceTreeItem } from '$lib/api/types';

	interface Props {
		pathname: string;
		onSelectVm: (id: string, name: string) => void;
		onContextMenu: (event: MouseEvent, instance: InstanceTreeItem) => void;
	}

	let { pathname, onSelectVm, onContextMenu }: Props = $props();

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

	function isActive(href: string, pathname: string): boolean {
		if (href === '/') return pathname === '/';
		return pathname === href || pathname.startsWith(`${href}/`);
	}
</script>

{#if pinnedVms.length > 0}
	<div class="flex flex-col gap-1">
		<div class="text-[10px] font-bold uppercase text-[var(--color-neutral-500)] mb-1 pl-2 tracking-wider">Pinned</div>
		{#each pinnedVms as vm}
			{@const inst = vmToTreeItem(vm)}
			{@const isVmActive = isActive(`/vms/${vm.id}`, pathname)}
			<div
				class="app-nav__pinned-row group {isVmActive ? 'app-nav__tree-link--active' : ''}"
				role="button"
				tabindex="0"
				aria-label="Pinned instance {vm.name}"
				onclick={() => onSelectVm(vm.id, vm.name)}
				onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onSelectVm(vm.id, vm.name); } }}
				oncontextmenu={(e) => onContextMenu(e, inst)}
			>
				<Pin size={12} />
				<span class="truncate">{vm.name}</span>
				<InstanceStatusBadge status={inst.status} showText={false} />
			</div>
		{/each}
	</div>
{/if}

<style>
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
