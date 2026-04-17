<script lang="ts">
	import InfrastructureState from './InfrastructureState.svelte';
	
	interface Props {
		title?: string;
		description?: string;
		hint?: string;
	}

	let { 
		title = 'Loading infrastructure data...', 
		description = 'Connecting to control plane and streaming metrics.',
		hint = 'This usually takes less than a second.'
	}: Props = $props();
</script>

<InfrastructureState variant="loading" {title} {description} {hint} />
