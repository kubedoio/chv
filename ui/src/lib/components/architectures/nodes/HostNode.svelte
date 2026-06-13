<script lang="ts">
	import type { NodeProps } from '@xyflow/svelte';
	import { Server } from 'lucide-svelte';
	import NodeShell from './NodeShell.svelte';

	type FindingSeverity = 'error' | 'warning' | 'info' | 'clean';
	interface Props extends NodeProps {
		findingSeverity?: FindingSeverity;
	}
	let props: Props = $props();
</script>

<NodeShell {...props} Icon={Server} kindLabel="Host" kindId="host" />
