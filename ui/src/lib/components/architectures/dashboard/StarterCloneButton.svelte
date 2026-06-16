<script lang="ts">
	import Button from '$lib/components/primitives/Button.svelte';

	/**
	 * Starter-specific affordances for the architecture detail page.
	 *
	 * Renders nothing on user-owned architectures; renders both a primary
	 * "Clone starter" button (header slot) and a read-only banner (body
	 * slot) when the parent has determined the architecture is a
	 * system-provided starter via `isStarter()` from `$lib/architectures/starter`.
	 *
	 * The wrapping component owns the actual clone request — this file is
	 * pure presentation so the detail page can stay close to its prior
	 * size and the new behavior is reviewable in isolation.
	 */
	interface Props {
		cloning: boolean;
		onClone: () => void;
	}

	let { cloning, onClone }: Props = $props();
</script>

<Button
	variant="primary"
	size="sm"
	onclick={onClone}
	disabled={cloning}
	data-testid="clone-starter"
>
	{cloning ? 'Cloning…' : 'Clone starter'}
</Button>
