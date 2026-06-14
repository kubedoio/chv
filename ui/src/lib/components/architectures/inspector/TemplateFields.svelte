<script lang="ts">
	/** Inspector fields for `template` nodes. */
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
	<label for="{nodeId}-image">Image (ref)</label>
	<input
		id="{nodeId}-image"
		type="text"
		value={read('image')}
		oninput={(e) => update({ image: e.currentTarget.value })}
		placeholder="ubuntu-22.04"
	/>

	<label for="{nodeId}-cpu">vCPU</label>
	<input
		id="{nodeId}-cpu"
		type="number"
		min="1"
		value={read('cpu')}
		oninput={(e) => update({ cpu: asNumberOrNull(e.currentTarget.value) })}
	/>

	<label for="{nodeId}-memory-mb">Memory (MiB)</label>
	<input
		id="{nodeId}-memory-mb"
		type="number"
		min="1"
		value={read('memory_mb')}
		oninput={(e) => update({ memory_mb: asNumberOrNull(e.currentTarget.value) })}
	/>

	<label for="{nodeId}-disk-gb">Disk (GiB)</label>
	<input
		id="{nodeId}-disk-gb"
		type="number"
		min="1"
		value={read('disk_gb')}
		oninput={(e) => update({ disk_gb: asNumberOrNull(e.currentTarget.value) })}
	/>
</div>
