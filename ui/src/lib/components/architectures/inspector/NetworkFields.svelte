<script lang="ts">
	/** Inspector fields for `network` nodes. */
	import type { CanvasNodeData } from '$lib/stores/architecture-canvas-store.svelte';
	import './field-styles.css';

	interface Props {
		nodeId: string;
		data: CanvasNodeData;
		update: (partial: Record<string, unknown>) => void;
	}

	let { nodeId, data, update }: Props = $props();

	function read(field: string): string {
		const v = data[field];
		return typeof v === 'string' || typeof v === 'number' ? String(v) : '';
	}

	function asNumberOrNull(value: string): number | null {
		if (value.trim() === '') return null;
		const n = Number(value);
		return Number.isFinite(n) ? n : null;
	}
</script>

<div class="chv-fields-grid">
	<label for="{nodeId}-type">Type</label>
	<select
		id="{nodeId}-type"
		value={read('type') || 'bridge'}
		onchange={(e) => update({ type: e.currentTarget.value })}
	>
		<option value="bridge">bridge</option>
		<option value="vlan">vlan</option>
		<option value="nat">nat</option>
		<option value="isolated">isolated</option>
		<option value="routed">routed</option>
	</select>

	<label for="{nodeId}-bridge">Bridge</label>
	<input
		id="{nodeId}-bridge"
		type="text"
		value={read('bridge')}
		oninput={(e) => update({ bridge: e.currentTarget.value })}
		placeholder="br0"
	/>

	<label for="{nodeId}-vlan-id">VLAN ID</label>
	<input
		id="{nodeId}-vlan-id"
		type="number"
		min="0"
		max="4094"
		value={read('vlan_id')}
		oninput={(e) => update({ vlan_id: asNumberOrNull(e.currentTarget.value) })}
	/>

	<label for="{nodeId}-cidr">CIDR</label>
	<input
		id="{nodeId}-cidr"
		type="text"
		value={read('cidr')}
		oninput={(e) => update({ cidr: e.currentTarget.value })}
		placeholder="10.0.0.0/24"
	/>

	<label for="{nodeId}-gateway">Gateway</label>
	<input
		id="{nodeId}-gateway"
		type="text"
		value={read('gateway')}
		oninput={(e) => update({ gateway: e.currentTarget.value })}
		placeholder="10.0.0.1"
	/>
</div>
