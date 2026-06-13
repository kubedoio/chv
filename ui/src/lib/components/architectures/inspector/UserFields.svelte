<script lang="ts">
	/** Inspector fields for `user` nodes. */
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
	<label for="{nodeId}-display-name">Display name</label>
	<input
		id="{nodeId}-display-name"
		type="text"
		value={read('display_name')}
		oninput={(e) => update({ display_name: e.currentTarget.value })}
		placeholder="Jane Operator"
	/>

	<label for="{nodeId}-email">Email</label>
	<input
		id="{nodeId}-email"
		type="email"
		value={read('email')}
		oninput={(e) => update({ email: e.currentTarget.value })}
		placeholder="jane@example.com"
	/>

	<label for="{nodeId}-roles">Roles (comma-separated)</label>
	<input
		id="{nodeId}-roles"
		type="text"
		value={read('roles')}
		oninput={(e) => update({ roles: e.currentTarget.value })}
		placeholder="operator, viewer"
	/>
</div>
