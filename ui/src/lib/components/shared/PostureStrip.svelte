<script lang="ts">
	interface Chip {
		label: string;
		value: string | number;
		variant?: 'default' | 'healthy' | 'warning' | 'degraded' | 'failed';
	}

	interface Props {
		chips: Chip[];
	}

	let { chips }: Props = $props();

	const variantValueClasses: Record<string, string> = {
		healthy: 'text-[var(--status-healthy-text)]',
		warning: 'text-[var(--status-warning-text)]',
		degraded: 'text-[var(--status-failed-text)]',
		failed: 'text-[var(--status-failed-text)]',
		default: 'text-[var(--shell-text)]'
	};
</script>

<div class="flex flex-wrap gap-2" role="list" aria-label="Fleet posture">
	{#each chips as chip}
		<div class="flex items-center gap-[0.4rem] border border-[var(--shell-line)] rounded-full bg-[var(--shell-surface)] px-[0.8rem] py-[0.35rem] text-[0.8rem]" role="listitem">
			<span class="text-[var(--shell-text-muted)]">{chip.label}</span>
			<span class="font-bold {variantValueClasses[chip.variant ?? 'default']}">{chip.value}</span>
		</div>
	{/each}
</div>
