import { selection } from '$lib/stores/selection.svelte';
import { inventory } from '$lib/stores/inventory.svelte';
import { getDisplayNodes, getDisplayVms, getStatusColor } from './topology-layout.svelte';

export { getStatusColor };

const _selectedResource = $derived(
	(() => {
		if (!selection.active.id) {
			return {
				type: 'fleet',
				name: 'Global Fleet',
				status: 'Nominal',
				tone: 'healthy',
				meta: `${inventory.nodes.length} hosts · ${inventory.vms.length} workloads`,
				actions: [
					{ label: 'All instances', href: '/vms' },
					{ label: 'Tasks', href: '/tasks' },
					{ label: 'Events', href: '/events' }
				]
			};
		}

		if (selection.active.type === 'node') {
			const node = getDisplayNodes().find((item) => item.id === selection.active.id);
			if (!node) return null;
			return {
				type: 'host',
				name: node.name,
				status: node.status,
				tone: node.status,
				meta: `${node.vmCount} workloads · ${node.status}`,
				actions: [
					{ label: 'Open host', href: `/nodes/${node.id}` },
					{ label: 'Instances', href: `/vms?node_id=${node.id}` },
					{ label: 'Storage', href: `/storage?node_id=${node.id}` }
				]
			};
		}

		if (selection.active.type === 'vm') {
			const vm = getDisplayVms().find((item) => item.id === selection.active.id);
			if (!vm) return null;
			return {
				type: 'instance',
				name: vm.name,
				status: vm.stateLabel,
				tone: vm.status,
				meta: `${vm.nodeId} · ${vm.vcpu} vCPU · ${vm.memory_mb} MB`,
				actions: [
					{ label: 'Open instance', href: `/vms/${vm.id}` },
					{ label: 'Console', href: `/vms/${vm.id}?tab=console` },
					{ label: 'Events', href: `/events?resource_id=${vm.id}` }
				]
			};
		}

		return null;
	})()
);

export function getSelectedResource() { return _selectedResource; }
