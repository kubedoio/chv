import { liveState } from './live-state.svelte';

export const inventory = {
	get nodes() { return liveState.nodes; },
	get vms() { return liveState.vms; },
	get isLoading() { return liveState.inventoryLoading; },
	async fetch() { return liveState.fetchInventory(); },
};
