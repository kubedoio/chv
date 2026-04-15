<script lang="ts">
	import type { Snippet } from 'svelte';
	import { createAPIClient, getStoredToken } from '$lib/api/client';
	import KeyboardShortcutsHelp from '$lib/components/KeyboardShortcutsHelp.svelte';
	import QuickActions from '$lib/components/QuickActions.svelte';
	import SearchModal from '$lib/components/SearchModal.svelte';
	import AppShell from '$lib/components/shell/AppShell.svelte';
	import ToastContainer from '$lib/components/ToastContainer.svelte';
	import {
		createGlobalShortcuts,
		initKeyboardShortcuts,
		registerShortcuts,
		setActiveContext
	} from '$lib/stores/keyboard.svelte';
	import { buildSearchIndex, loadRecentSearches } from '$lib/stores/search.svelte';
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

		if (token) {
			loadSearchIndex(token);
		}

		return () => {
			cleanupKeyboard?.();
			unregisterGlobals();
		};
	});

	async function loadSearchIndex(token: string) {
		try {
			const client = createAPIClient({ token });
			const [vms, images, networks, storagePools] = await Promise.all([
				client.listVMs().catch(() => []),
				client.listImages().catch(() => []),
				client.listNetworks().catch(() => []),
				client.listStoragePools().catch(() => [])
			]);

			buildSearchIndex({
				vms: vms ?? [],
				images: images ?? [],
				networks: networks ?? [],
				storagePools: storagePools ?? []
			});
		} catch {
			buildSearchIndex({});
		}
	}
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
