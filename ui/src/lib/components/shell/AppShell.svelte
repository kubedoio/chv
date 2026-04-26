<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/stores';
	import MobileNav from '$lib/components/shell/MobileNav.svelte';
	import SidebarNav from '$lib/components/shell/SidebarNav.svelte';
	import TopCommandBar from '$lib/components/shell/TopCommandBar.svelte';
	import InspectDrawer from '$lib/components/shell/InspectDrawer.svelte';
	import { selection } from '$lib/stores/selection.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';

	let { children }: { children?: Snippet } = $props();

	const currentPage = $derived(getPageDefinition($page.url.pathname));
	const showInspector = $derived(Boolean(selection.active.id));
</script>

<a class="absolute -top-16 left-4 z-20 px-[0.9rem] py-[0.65rem] rounded-[0.75rem] bg-[var(--shell-accent)] text-[#fffaf3] no-underline focus:top-4" href="#shell-main">Skip to content</a>

<div class="grid min-h-screen bg-[var(--shell-bg)] text-[var(--shell-text)] dot-grid max-[960px]:!grid-cols-1" style="grid-template-columns: var(--sidebar-width) 1fr;">
	<div class="hidden max-[960px]:block">
		<MobileNav />
	</div>

	<aside class="sticky top-0 h-screen p-4 border-r border-[var(--shell-line)] bg-[var(--bg-sidebar)] text-white z-10 max-[960px]:hidden">
		<SidebarNav />
	</aside>

	<div class="flex flex-col min-w-0 max-[960px]:pt-[56px]">
		<TopCommandBar page={currentPage} />
		<div class="flex-1 grid min-h-0 {showInspector ? 'max-[1279px]:!grid-cols-1' : ''}" style="grid-template-columns: {showInspector ? 'minmax(0, 1fr) minmax(18rem, var(--inspector-width, 320px))' : 'minmax(0, 1fr)'};">
			<main id="shell-main" class="min-w-0 px-5 py-4 pb-6 overflow-y-auto max-[960px]:px-[0.875rem] max-[960px]:py-[0.875rem] max-[960px]:pb-5">
				{@render children?.()}
			</main>
			{#if showInspector}
				<aside class="border-l border-[var(--shell-line)] bg-[var(--shell-surface)] min-w-0 overflow-y-auto max-[1279px]:hidden">
					<InspectDrawer />
				</aside>
			{/if}
		</div>
	</div>
</div>
