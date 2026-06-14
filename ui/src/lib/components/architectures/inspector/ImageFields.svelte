<script lang="ts">
	/** Inspector fields for `image` nodes. */
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
	<label for="{nodeId}-source">Source URL</label>
	<input
		id="{nodeId}-source"
		type="text"
		value={read('source')}
		oninput={(e) => update({ source: e.currentTarget.value })}
		placeholder="https://cloud-images.example/ubuntu.qcow2"
	/>

	<label for="{nodeId}-format">Format</label>
	<select
		id="{nodeId}-format"
		value={read('format') || 'qcow2'}
		onchange={(e) => update({ format: e.currentTarget.value })}
	>
		<option value="qcow2">qcow2</option>
		<option value="raw">raw</option>
	</select>

	<label for="{nodeId}-datastore">Datastore (ref)</label>
	<input
		id="{nodeId}-datastore"
		type="text"
		value={read('datastore')}
		oninput={(e) => update({ datastore: e.currentTarget.value })}
		placeholder="default-ds"
	/>
</div>
