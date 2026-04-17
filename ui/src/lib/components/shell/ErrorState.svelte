<script lang="ts">
	import InfrastructureState from './InfrastructureState.svelte';
	
	interface Props {
		title?: string;
		description?: string;
		hint?: string;
	}

	let { 
		title = 'Infrastructure communication failure', 
		description = 'The control plane responded with an error or is unreachable.',
		hint = 'Verify your network connection and API server status.'
	}: Props = $props();
</script>

<InfrastructureState variant="error" {title} {description} {hint} />
