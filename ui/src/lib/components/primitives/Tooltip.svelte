<script lang="ts">
	interface Props {
		content: string;
		position?: 'top' | 'bottom' | 'left' | 'right';
		delay?: number;
	}

	let {
		content,
		position = 'top',
		delay = 200,
		children
	}: Props & { children?: import('svelte').Snippet } = $props();

	let isVisible = $state(false);
	let tooltipRef = $state<HTMLDivElement | null>(null);
	let triggerRef = $state<HTMLDivElement | null>(null);
	let timeoutId = $state<ReturnType<typeof setTimeout> | null>(null);

	const showTooltip = () => {
		timeoutId = setTimeout(() => {
			isVisible = true;
		}, delay);
	};

	const hideTooltip = () => {
		if (timeoutId) {
			clearTimeout(timeoutId);
			timeoutId = null;
		}
		isVisible = false;
	};

	const positionClasses = {
		top: 'tooltip-top',
		bottom: 'tooltip-bottom',
		left: 'tooltip-left',
		right: 'tooltip-right'
	};
</script>

<div
	class="tooltip-trigger"
	bind:this={triggerRef}
	onmouseenter={showTooltip}
	onmouseleave={hideTooltip}
	onfocus={showTooltip}
	onblur={hideTooltip}
	role="button"
	tabindex="0"
	aria-describedby={isVisible ? 'tooltip-content' : undefined}
>
	{@render children?.()}
	{#if isVisible}
		<div
			id="tooltip-content"
			bind:this={tooltipRef}
			class="tooltip {positionClasses[position]}"
			role="tooltip"
		>
			{content}
		</div>
	{/if}
</div>

<style>
	.tooltip-trigger {
		position: relative;
		display: inline-flex;
	}

	.tooltip {
		position: absolute;
		padding: var(--space-2) var(--space-3);
		font-size: var(--text-xs);
		font-weight: 500;
		color: white;
		background: var(--color-neutral-800);
		border-radius: var(--radius-sm);
		white-space: nowrap;
		z-index: 50;
		pointer-events: none;
		animation: fadeIn var(--duration-fast) var(--ease-default);
	}

	.tooltip::before {
		content: '';
		position: absolute;
		width: 6px;
		height: 6px;
		background: var(--color-neutral-800);
		transform: rotate(45deg);
	}

	/* Position variants */
	.tooltip-top {
		bottom: calc(100% + var(--space-2));
		left: 50%;
		transform: translateX(-50%);
	}

	.tooltip-top::before {
		bottom: -3px;
		left: 50%;
		transform: translateX(-50%) rotate(45deg);
	}

	.tooltip-bottom {
		top: calc(100% + var(--space-2));
		left: 50%;
		transform: translateX(-50%);
	}

	.tooltip-bottom::before {
		top: -3px;
		left: 50%;
		transform: translateX(-50%) rotate(45deg);
	}

	.tooltip-left {
		right: calc(100% + var(--space-2));
		top: 50%;
		transform: translateY(-50%);
	}

	.tooltip-left::before {
		right: -3px;
		top: 50%;
		transform: translateY(-50%) rotate(45deg);
	}

	.tooltip-right {
		left: calc(100% + var(--space-2));
		top: 50%;
		transform: translateY(-50%);
	}

	.tooltip-right::before {
		left: -3px;
		top: 50%;
		transform: translateY(-50%) rotate(45deg);
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
			transform: translateX(-50%) translateY(-4px);
		}
		to {
			opacity: 1;
			transform: translateX(-50%) translateY(0);
		}
	}
</style>
