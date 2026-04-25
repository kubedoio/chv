<script lang="ts">
	interface Props {
		title: string;
		icon?: any;
		children: import('svelte').Snippet;
		actions?: import('svelte').Snippet;
		badgeLabel?: string;
		badgeTone?: 'healthy' | 'warning' | 'degraded' | 'failed' | 'neutral';
		collapsed?: boolean;
	}

	let { title, icon: Icon, children, actions, badgeLabel, badgeTone = 'neutral', collapsed = false }: Props = $props();

	const badgeToneClasses: Record<string, string> = {
		healthy: 'bg-[var(--color-success-light)] text-[var(--color-success-dark)]',
		warning: 'bg-[var(--color-warning-light)] text-[var(--color-warning-dark)]',
		degraded: 'bg-[var(--color-danger-light)] text-[var(--color-danger-dark)]',
		failed: 'bg-[var(--color-danger-light)] text-[var(--color-danger-dark)]',
		neutral: 'bg-[var(--shell-line)] text-[var(--shell-text-muted)]'
	};
</script>

<section class="bg-[var(--shell-surface)] border border-[var(--shell-line)] rounded-[0.35rem] flex flex-col overflow-hidden">
	<header class="flex justify-between items-center px-3 py-[0.65rem] border-b border-[var(--shell-line)] bg-[var(--shell-surface-muted)]">
		<div class="flex items-center gap-2">
			{#if Icon}<Icon size={14} class="text-[var(--shell-text-muted)]" />{/if}
			<h3 class="text-[length:var(--text-xs)] font-bold uppercase tracking-[0.05em] text-[var(--shell-text-muted)] m-0">{title}</h3>
			{#if badgeLabel}
				<span class="text-[10px] font-bold px-[0.4rem] py-[0.1rem] rounded-full {badgeToneClasses[badgeTone]}">{badgeLabel}</span>
			{/if}
		</div>
		{#if actions}
			<div class="flex gap-2">
				{@render actions()}
			</div>
		{/if}
	</header>
	{#if !collapsed}
		<div class="p-3 text-[length:var(--text-sm)]">
			{@render children()}
		</div>
	{/if}
</section>
