<script lang="ts">
	interface Props {
		width?: string;
		height?: string;
		circle?: boolean;
		animate?: boolean;
	}

	let {
		width = '100%',
		height = '1rem',
		circle = false,
		animate = true
	}: Props = $props();

	const borderRadius = $derived(circle ? '50%' : 'var(--radius-sm)');
</script>

<div
	class="skeleton"
	class:animate={animate}
	style="width: {width}; height: {height}; border-radius: {borderRadius};"
	aria-hidden="true"
></div>

<style>
	.skeleton {
		background: linear-gradient(
			90deg,
			var(--color-neutral-200) 25%,
			var(--color-neutral-100) 50%,
			var(--color-neutral-200) 75%
		);
		background-size: 200% 100%;
	}

	.skeleton.animate {
		animation: skeleton-loading 1.5s ease-in-out infinite;
	}

	@keyframes skeleton-loading {
		0% {
			background-position: 200% 0;
		}
		100% {
			background-position: -200% 0;
		}
	}
</style>
