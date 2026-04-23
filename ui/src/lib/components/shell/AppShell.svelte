<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/stores';
	import MobileNav from '$lib/components/navigation/MobileNav.svelte';
	import SidebarNav from '$lib/components/shell/SidebarNav.svelte';
	import TopCommandBar from '$lib/components/shell/TopCommandBar.svelte';
	import InspectDrawer from '$lib/components/shell/InspectDrawer.svelte';
	import { selection } from '$lib/stores/selection.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';

	let { children }: { children?: Snippet } = $props();

	const currentPage = $derived(getPageDefinition($page.url.pathname));
	const showInspector = $derived(Boolean(selection.active.id));
</script>

<a class="shell-skip-link" href="#shell-main">Skip to content</a>

<div class="app-shell dot-grid">
	<div class="app-shell__mobile-nav">
		<MobileNav />
	</div>

	<aside class="app-shell__nav">
		<SidebarNav />
	</aside>

	<div class="app-shell__main">
		<TopCommandBar page={currentPage} />
		<div class="app-shell__body" class:app-shell__body--with-inspector={showInspector}>
			<main id="shell-main" class="app-shell__content">
				{@render children?.()}
			</main>
			{#if showInspector}
				<aside class="app-shell__inspector">
					<InspectDrawer />
				</aside>
			{/if}
		</div>
	</div>
</div>

<style>
	.app-shell {
		display: grid;
		grid-template-columns: var(--sidebar-width) 1fr;
		min-height: 100vh;
		background: var(--shell-bg);
		color: var(--shell-text);
	}

	.app-shell__nav {
		position: sticky;
		top: 0;
		height: 100vh;
		padding: 1rem;
		border-right: 1px solid var(--shell-line);
		background: var(--bg-sidebar);
		color: #ffffff; /* Contrast for dark sidebar */
		z-index: 10;
	}

	.app-shell__main {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.app-shell__body {
		flex: 1;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		min-height: 0;
	}

	.app-shell__body--with-inspector {
		grid-template-columns: minmax(0, 1fr) minmax(18rem, var(--inspector-width, 320px));
	}

	.app-shell__content {
		min-width: 0;
		padding: 1rem 1.25rem 1.5rem;
		overflow-y: auto;
	}

	.app-shell__inspector {
		border-left: 1px solid var(--shell-line);
		background: var(--shell-surface);
		min-width: 0;
		overflow-y: auto;
	}

	.app-shell__mobile-nav {
		display: none;
	}

	.shell-skip-link {
		position: absolute;
		top: -4rem;
		left: 1rem;
		z-index: 20;
		padding: 0.65rem 0.9rem;
		border-radius: 0.75rem;
		background: var(--shell-accent);
		color: #fffaf3;
		text-decoration: none;
	}

	.shell-skip-link:focus {
		top: 1rem;
	}

	@media (max-width: 1279px) {
		.app-shell__body--with-inspector {
			grid-template-columns: minmax(0, 1fr);
		}

		.app-shell__inspector {
			display: none;
		}
	}

	@media (max-width: 960px) {
		.app-shell {
			grid-template-columns: 1fr;
		}

		.app-shell__nav {
			display: none;
		}

		.app-shell__mobile-nav {
			display: block;
		}

		.app-shell__main {
			padding-top: 56px;
		}

		.app-shell__content {
			padding: 0.875rem 0.875rem 1.25rem;
		}
	}
</style>
