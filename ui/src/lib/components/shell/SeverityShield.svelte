<script lang="ts">
	import { Info, AlertTriangle, ShieldAlert } from 'lucide-svelte';

	interface Props {
		severity: 'info' | 'warning' | 'critical';
		showLabel?: boolean;
	}

	let { severity, showLabel = false }: Props = $props();

	const config = $derived({
		info: { icon: Info, color: 'var(--shell-accent)', label: 'Info', bg: 'var(--shell-surface-muted)' },
		warning: { icon: AlertTriangle, color: 'var(--color-warning-dark)', label: 'Warning', bg: 'var(--color-warning-light)' },
		critical: { icon: ShieldAlert, color: 'var(--color-danger)', label: 'Critical', bg: 'var(--color-danger-light)' }
	}[severity]);
</script>

<div class="severity-shield severity-{severity}" style="--badge-bg: {config.bg}; --badge-color: {config.color}">
	<config.icon size={14} />
	{#if showLabel}
		<span class="label">{config.label}</span>
	{/if}
</div>

<style>
	.severity-shield {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.15rem 0.4rem;
		border-radius: 9999px;
		background: var(--badge-bg);
		color: var(--badge-color);
		font-weight: 700;
		font-size: 10px;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		border: 1px solid transparent;
	}

	.severity-critical {
		border-color: var(--color-danger);
		box-shadow: 0 0 4px rgba(239, 68, 68, 0.2);
	}

	.severity-warning {
		border-color: var(--color-warning-dark);
	}
</style>
