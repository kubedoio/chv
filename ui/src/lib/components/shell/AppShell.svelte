<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/stores';
	import SidebarNav from '$lib/components/shell/SidebarNav.svelte';
	import TopCommandBar from '$lib/components/shell/TopCommandBar.svelte';
	import InspectDrawer from '$lib/components/shell/InspectDrawer.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';

	let { children }: { children?: Snippet } = $props();

	const currentPage = $derived(getPageDefinition($page.url.pathname));
</script>

<a class="shell-skip-link" href="#shell-main">Skip to content</a>

<div class="app-shell dot-grid">
	<aside class="app-shell__nav">
		<SidebarNav />
	</aside>

	<div class="app-shell__main">
		<TopCommandBar page={currentPage} />
		<div class="app-shell__body">
			<main id="shell-main" class="app-shell__content">
				{@render children?.()}
			</main>
			<aside class="app-shell__inspector">
				<InspectDrawer />
			</aside>
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
		grid-template-columns: 1fr var(--inspector-width, 320px);
		min-height: 0;
	}

	.app-shell__content {
		padding: 1rem;
		overflow-y: auto;
	}

	.app-shell__inspector {
		border-left: 1px solid var(--shell-line);
		background: var(--shell-surface);
		overflow-y: auto;
	}

	.inspector-placeholder {
		padding: 1rem;
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.1em;
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

	@media (max-width: 960px) {
		.app-shell {
			grid-template-columns: 1fr;
		}

		.app-shell__nav {
			position: static;
			height: auto;
			border-right: 0;
			border-bottom: 1px solid var(--shell-line);
		}
	}
</style>
