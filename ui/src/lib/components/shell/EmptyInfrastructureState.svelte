<script lang="ts">
	import InfrastructureState from './InfrastructureState.svelte';
	
	interface Props {
		title: string;
		description: string;
		hint: string;
	}

	let { title, description, hint }: Props = $props();
</script>

<InfrastructureState variant="empty" {title} {description} {hint} />
