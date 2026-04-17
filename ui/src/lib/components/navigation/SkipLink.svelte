<script lang="ts">
	interface Props {
		target?: string;
		text?: string;
	}

	let { target = 'main-content', text = 'Skip to main content' }: Props = $props();

	function handleClick(event: MouseEvent) {
		event.preventDefault();
		const element = document.getElementById(target);
		if (element) {
			element.focus();
			element.scrollIntoView({ behavior: 'smooth' });
		}
	}
</script>

<a
	href={`#${target}`}
	class="skip-link"
	onclick={handleClick}
>
	{text}
</a>

<style>
	.skip-link {
		position: absolute;
		top: -40px;
		left: 0;
		z-index: 100;
		padding: 8px 16px;
		background: var(--color-primary);
		color: white;
		font-weight: 500;
		text-decoration: none;
		border-radius: 0 0 4px 0;
		transition: top 0.2s ease;
	}

	.skip-link:focus {
		top: 0;
		outline: 2px solid white;
		outline-offset: -2px;
	}
</style>
