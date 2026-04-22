import { createAPIClient, getStoredToken } from '$lib/api/client';
import type { NodeWithResources, VM } from '$lib/api/types';

class InventoryStore {
	nodes = $state<NodeWithResources[]>([]);
	vms = $state<VM[]>([]);
	isLoading = $state(true);

	async fetch() {
		const token = getStoredToken();
		if (!token) {
			this.isLoading = false;
			return;
		}

		try {
			const client = createAPIClient({ token });
			const [nodesRes, vmsRes] = await Promise.all([
				client.listNodes(),
				client.listVMs()
			]);
			this.nodes = nodesRes || [];
			this.vms = vmsRes || [];
		} catch (err) {
			console.error('Failed to load local inventory:', err);
		} finally {
			this.isLoading = false;
		}
	}
}

export const inventory = new InventoryStore();
