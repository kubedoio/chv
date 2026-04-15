<script lang="ts">
	import PlaceholderPage from '$lib/components/shell/PlaceholderPage.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';

	interface Props {
		href: string;
	}

	let { href }: Props = $props();

	const page = $derived(getPageDefinition(href));
</script>

<svelte:head>
	<title>{page.title} | CHV</title>
</svelte:head>

<PlaceholderPage {page} />
