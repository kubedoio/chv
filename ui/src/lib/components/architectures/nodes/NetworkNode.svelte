<script lang="ts">
	import type { NodeProps } from '@xyflow/svelte';
	import { Network } from 'lucide-svelte';
	import NodeShell from './NodeShell.svelte';

	type FindingSeverity = 'error' | 'warning' | 'info' | 'clean';
	interface Props extends NodeProps {
		findingSeverity?: FindingSeverity;
	}
	let props: Props = $props();
</script>

<NodeShell {...props} Icon={Network} kindLabel="Network" kindId="network" />
