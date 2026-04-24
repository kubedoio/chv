<script lang="ts">
	import type { InstanceStatus } from '$lib/api/types';

	interface Props {
		status: InstanceStatus;
		showText?: boolean;
	}

	let { status, showText = true }: Props = $props();

	const config: Record<
		InstanceStatus,
		{ label: string; toneClass: string; dotClass: string }
	> = {
		running: {
			label: 'RUNNING',
			toneClass: 'text-[var(--status-healthy-text)]',
			dotClass: 'bg-[var(--color-success)]'
		},
		stopped: {
			label: 'STOPPED',
			toneClass: 'text-[var(--color-neutral-500)]',
			dotClass: 'bg-[var(--color-neutral-500)]'
		},
		error: {
			label: 'ERROR',
			toneClass: 'text-[var(--status-failed-text)]',
			dotClass: 'bg-[var(--color-danger)]'
		},
		paused: {
			label: 'PAUSED',
			toneClass: 'text-[var(--status-warning-text)]',
			dotClass: 'bg-[var(--color-warning)]'
		},
		unknown: {
			label: 'UNKNOWN',
			toneClass: 'text-[var(--status-unknown-text)]',
			dotClass: 'bg-[var(--color-neutral-400)]'
		}
	};

	const c = $derived(config[status] ?? config.unknown);
</script>

<span class="inline-flex items-center gap-1.5 text-[length:var(--text-xs)] font-medium {c.toneClass}" aria-label="Status: {c.label}">
	<span class="w-[6px] h-[6px] rounded-full {c.dotClass}" aria-hidden="true"></span>
	{#if showText}
		<span>{c.label}</span>
	{/if}
</span>
