<script lang="ts">
	import { Moon, Sun, Settings, LogOut } from 'lucide-svelte';
	import { theme } from '$lib/stores/theme.svelte';

	interface Props {
		onLogout: () => void;
	}

	let { onLogout }: Props = $props();
</script>

<div class="flex gap-2">
	<button
		type="button"
		class="w-7 h-7 grid place-items-center bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] text-[var(--color-neutral-400)] cursor-pointer transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-700)] hover:text-[var(--color-sidebar-text-active,#ffffff)]"
		onclick={() => theme.toggle()}
		aria-label="Toggle theme"
	>
		{#if theme.value === 'dark'}<Sun size={12} />{:else}<Moon size={12} />{/if}
	</button>
	<a
		href="/settings"
		class="w-7 h-7 grid place-items-center bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] text-[var(--color-neutral-400)] cursor-pointer transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-700)] hover:text-[var(--color-sidebar-text-active,#ffffff)]"
		aria-label="Settings"
	>
		<Settings size={12} />
	</a>
	<button
		type="button"
		class="w-7 h-7 grid place-items-center bg-[var(--color-neutral-800)] border border-[var(--color-neutral-700)] rounded-[var(--radius-xs)] text-[var(--color-neutral-400)] cursor-pointer transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-700)] hover:text-[var(--color-sidebar-text-active,#ffffff)]"
		onclick={onLogout}
		aria-label="Sign out"
	>
		<LogOut size={12} />
	</button>
</div>
