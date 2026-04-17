<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/stores';
	import SidebarNav from '$lib/components/shell/SidebarNav.svelte';
	import TopCommandBar from '$lib/components/shell/TopCommandBar.svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';

	let { children }: { children?: Snippet } = $props();

	const currentPage = $derived(getPageDefinition($page.url.pathname));
</script>

<a class="shell-skip-link" href="#shell-main">Skip to content</a>

<div class="app-shell">
	<aside class="app-shell__nav">
		<SidebarNav />
	</aside>

	<div class="app-shell__main">
		<TopCommandBar page={currentPage} />
		<main id="shell-main" class="app-shell__content">
			{@render children?.()}
		</main>
	</div>
</div>

<style>
	.app-shell {
		display: grid;
		grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
		min-height: 100vh;
		background:
			radial-gradient(circle at top left, rgba(143, 90, 42, 0.06), transparent 22%),
			linear-gradient(180deg, rgba(252, 250, 245, 0.98), rgba(244, 240, 232, 0.98));
		color: var(--shell-text);
	}

	.app-shell__nav {
		position: sticky;
		top: 0;
		height: 100vh;
		padding: 1.2rem 1rem;
		border-right: 1px solid var(--shell-line);
		background: rgba(251, 248, 242, 0.92);
		backdrop-filter: blur(12px);
	}

	.app-shell__main {
		display: grid;
		align-content: start;
		gap: 1rem;
		padding: 1rem;
	}

	.app-shell__content {
		width: min(100%, 82rem);
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
