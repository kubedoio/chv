<script lang="ts">
	/** Inspector fields for `datastore` nodes. */
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
</script>

<div class="chv-fields-grid">
	<label for="{nodeId}-type">Type</label>
	<select
		id="{nodeId}-type"
		value={read('type') || 'qcow2-dir'}
		onchange={(e) => update({ type: e.currentTarget.value })}
	>
		<option value="qcow2-dir">qcow2-dir</option>
		<option value="ceph-rbd">ceph-rbd</option>
		<option value="nfs">nfs</option>
		<option value="lvm">lvm</option>
		<option value="zfs">zfs</option>
	</select>

	<label for="{nodeId}-path">Path</label>
	<input
		id="{nodeId}-path"
		type="text"
		value={read('path')}
		oninput={(e) => update({ path: e.currentTarget.value })}
		placeholder="/var/lib/chv/datastore"
	/>

	<label for="{nodeId}-pool">Pool</label>
	<input
		id="{nodeId}-pool"
		type="text"
		value={read('pool')}
		oninput={(e) => update({ pool: e.currentTarget.value })}
		placeholder="rbd-pool"
	/>
</div>
