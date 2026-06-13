<script lang="ts">
	import CreateArchitectureForm from '$lib/components/architectures/dashboard/CreateArchitectureForm.svelte';
</script>

<svelte:head>
	<title>New Architecture · CellHV</title>
</svelte:head>

<div class="page">
	<CreateArchitectureForm />
</div>

<style>
	.page {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 0.5rem 0;
	}
</style>
