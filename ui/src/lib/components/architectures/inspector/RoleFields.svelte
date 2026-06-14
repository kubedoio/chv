<script lang="ts">
	/**
	 * Inspector fields for `role` nodes.
	 *
	 * Permissions are entered as one-per-line in a textarea and stored as a
	 * `\n`-joined string on `data.permissions`. Generating canonical YAML
	 * splits on newlines — see `architecture-canvas-store.generateYaml()`.
	 * A future Phase will swap this for a chip/multiselect, but Phase-2 keeps
	 * the inspector trivial.
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
		return typeof v === 'string' ? v : '';
	}
</script>

<div class="chv-fields-grid">
	<label for="{nodeId}-permissions">Permissions (one per line)</label>
	<textarea
		id="{nodeId}-permissions"
		value={read('permissions')}
		oninput={(e) => update({ permissions: e.currentTarget.value })}
		placeholder={'instances:read\ninstances:write'}
	></textarea>
</div>
