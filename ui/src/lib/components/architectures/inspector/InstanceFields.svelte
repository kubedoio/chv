<script lang="ts">
	/**
	 * Inspector fields for `instance` nodes.
	 *
	 * `placement` and `resources` are nested in the YAML contract; we flatten
	 * them onto `data` as `placement_server`, `resources_cpu`, `resources_memory_mb`
	 * and reassemble them in `architecture-canvas-store.generateYaml()` when
	 * the contract grows. For Phase-2 the inspector simply edits the flat keys.
	 */
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
	<label for="{nodeId}-template">Template (ref)</label>
	<input
		id="{nodeId}-template"
		type="text"
		value={read('template')}
		oninput={(e) => update({ template: e.currentTarget.value })}
		placeholder="web-template"
	/>

	<label for="{nodeId}-placement-server">Placement: server (ref)</label>
	<input
		id="{nodeId}-placement-server"
		type="text"
		value={read('placement_server')}
		oninput={(e) => update({ placement_server: e.currentTarget.value })}
		placeholder="host-01"
	/>

	<label for="{nodeId}-resources-cpu">Resources: vCPU</label>
	<input
		id="{nodeId}-resources-cpu"
		type="number"
		min="1"
		value={read('resources_cpu')}
		oninput={(e) => update({ resources_cpu: asNumberOrNull(e.currentTarget.value) })}
	/>

	<label for="{nodeId}-resources-memory-mb">Resources: memory (MiB)</label>
	<input
		id="{nodeId}-resources-memory-mb"
		type="number"
		min="1"
		value={read('resources_memory_mb')}
		oninput={(e) => update({ resources_memory_mb: asNumberOrNull(e.currentTarget.value) })}
	/>
</div>
