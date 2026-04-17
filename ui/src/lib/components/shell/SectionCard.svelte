<script lang="ts">
	interface Props {
		title: string;
		icon?: any;
		children: import('svelte').Snippet;
		actions?: import('svelte').Snippet;
		badgeLabel?: string;
		badgeTone?: 'healthy' | 'warning' | 'degraded' | 'failed' | 'neutral';
	}

	let { title, icon: Icon, children, actions, badgeLabel, badgeTone = 'neutral' }: Props = $props();
</script>

<section class="section-card">
	<header class="section-card__header">
		<div class="header-left">
			{#if Icon}<Icon size={14} class="section-icon" />{/if}
			<h3 class="section-title">{title}</h3>
			{#if badgeLabel}
				<span class="section-badge tone-{badgeTone}">{badgeLabel}</span>
			{/if}
		</div>
		{#if actions}
			<div class="header-right">
				{@render actions()}
			</div>
		{/if}
	</header>
	<div class="section-card__content">
		{@render children()}
	</div>
</section>

<style>
	.section-card {
		background: var(--shell-surface);
		border: 1px solid var(--shell-line);
		border-radius: 0.35rem;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.section-card__header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.65rem 0.75rem;
		border-bottom: 1px solid var(--shell-line);
		background: var(--shell-surface-muted);
	}

	.header-left {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.section-icon {
		color: var(--shell-text-muted);
	}

	.section-title {
		font-size: var(--text-xs);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--shell-text-muted);
		margin: 0;
	}

	.section-badge {
		font-size: 10px;
		font-weight: 700;
		padding: 0.1rem 0.4rem;
		border-radius: 9999px;
		background: var(--shell-line);
		color: var(--shell-text-muted);
	}

	.section-badge.tone-healthy { background: var(--color-success-light); color: var(--color-success-dark); }
	.section-badge.tone-warning { background: var(--color-warning-light); color: var(--color-warning-dark); }
	.section-badge.tone-failed { background: var(--color-danger-light); color: var(--color-danger-dark); }

	.section-card__content {
		padding: 0.75rem;
		font-size: var(--text-sm);
	}

	.header-right {
		display: flex;
		gap: 0.5rem;
	}
</style>
