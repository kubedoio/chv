<script lang="ts">
	import type { Snippet } from 'svelte';
	import { getStoredToken } from '$lib/api/client';
	import { syncAuthCookieFromLocalStorage } from '$lib/bff/auth-cookie';
	import KeyboardShortcutsHelp from '$lib/components/shared/KeyboardShortcutsHelp.svelte';
	import QuickActions from '$lib/components/shared/QuickActions.svelte';
	import SearchModal from '$lib/components/modals/SearchModal.svelte';
	import AppShell from '$lib/components/shell/AppShell.svelte';
	import ToastContainer from '$lib/components/feedback/ToastContainer.svelte';
	import {
		createGlobalShortcuts,
		initKeyboardShortcuts,
		registerShortcuts,
		setActiveContext
	} from '$lib/stores/keyboard.svelte';
	import { buildSearchIndex, loadRecentSearches } from '$lib/stores/search.svelte';
	import { theme } from '$lib/stores/theme.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import '../app.css';

	let { children }: { children?: Snippet } = $props();
	const publicPaths = ['/login', '/install'];
	let searchOpen = $state(false);
	let quickActionsOpen = $state(false);

	const isPublicRoute = $derived(
		publicPaths.some((publicPath) => $page.url.pathname.startsWith(publicPath))
	);

	function getContextFromPath(pathname: string): string {
		if (pathname === '/vms') return 'vms';
		if (pathname.startsWith('/vms/')) return 'vm-detail';
		return 'global';
	}

	$effect(() => {
		setActiveContext(getContextFromPath($page.url.pathname));
	});

	onMount(() => {
		const token = getStoredToken();
		syncAuthCookieFromLocalStorage();
		theme.init();

		if (!isPublicRoute && !getStoredToken()) {
			goto('/login');
		}

		loadRecentSearches();
		buildSearchIndex({});

		const cleanupKeyboard = initKeyboardShortcuts();
		const unregisterGlobals = registerShortcuts(
			createGlobalShortcuts(
				() => (searchOpen = true),
				() => (quickActionsOpen = true)
			)
		);

		return () => {
			cleanupKeyboard?.();
			unregisterGlobals();
		};
	});
</script>

<ToastContainer />
<SearchModal bind:open={searchOpen} />
<KeyboardShortcutsHelp />
<QuickActions bind:open={quickActionsOpen} />

{#if isPublicRoute}
	{@render children?.()}
{:else}
	<AppShell>
		{@render children?.()}
	</AppShell>
{/if}
