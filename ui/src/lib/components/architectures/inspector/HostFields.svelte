<script lang="ts">
	/**
	 * Inspector fields for `host` nodes.
	 *
	 * Field set is the Phase-2 minimal (per spec): management_ip, role, cpu_cores,
	 * memory_gb. All edits flow back through `architectureCanvasStore.updateNodeData`,
	 * which the parent Inspector wires up via the `update` callback.
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
	<label for="{nodeId}-management-ip">Management IP</label>
	<input
		id="{nodeId}-management-ip"
		type="text"
		value={read('management_ip')}
		oninput={(e) => update({ management_ip: e.currentTarget.value })}
		placeholder="10.0.0.10"
	/>

	<label for="{nodeId}-role">Role</label>
	<select
		id="{nodeId}-role"
		value={read('role') || 'compute'}
		onchange={(e) => update({ role: e.currentTarget.value })}
	>
		<option value="compute">compute</option>
		<option value="storage">storage</option>
		<option value="network">network</option>
		<option value="management">management</option>
		<option value="mixed">mixed</option>
	</select>

	<label for="{nodeId}-cpu-cores">CPU cores</label>
	<input
		id="{nodeId}-cpu-cores"
		type="number"
		min="1"
		value={read('cpu_cores')}
		oninput={(e) => update({ cpu_cores: asNumberOrNull(e.currentTarget.value) })}
	/>

	<label for="{nodeId}-memory-gb">Memory (GiB)</label>
	<input
		id="{nodeId}-memory-gb"
		type="number"
		min="1"
		value={read('memory_gb')}
		oninput={(e) => update({ memory_gb: asNumberOrNull(e.currentTarget.value) })}
	/>
</div>
