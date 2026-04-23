<script lang="ts">
	import { Database } from 'lucide-svelte';
</script>

<div class="empty-state">
	<div class="empty-icon">
		<Database size={32} />
	</div>
	<p>Select a node or VM to inspect its live status and metadata.</p>
</div>

<style>
	.empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 2rem;
		text-align: center;
		gap: 1.5rem;
	}

	.empty-icon {
		color: var(--color-neutral-200);
	}

	.empty-state p {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--shell-text-muted);
		max-width: 16rem;
		line-height: 1.5;
	}
</style>
