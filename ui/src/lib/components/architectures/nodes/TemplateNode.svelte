<script lang="ts">
	import type { NodeProps } from '@xyflow/svelte';
	import { Layers } from 'lucide-svelte';
	import NodeShell from './NodeShell.svelte';

	type FindingSeverity = 'error' | 'warning' | 'info' | 'clean';
	interface Props extends NodeProps {
		findingSeverity?: FindingSeverity;
	}
	let props: Props = $props();
</script>

<NodeShell {...props} Icon={Layers} kindLabel="Template" kindId="template" />
