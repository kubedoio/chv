<script lang="ts">
	import { Database } from 'lucide-svelte';
</script>

<div class="empty-state">
	<div class="empty-icon">
		<Database size={32} />
	</div>
	<p>SELECT_INFRA_ENTITY_TO_INSPECT_TELEMETRY</p>
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
		font-size: 10px;
		font-weight: 800;
		color: var(--color-neutral-300);
		letter-spacing: 0.05em;
		max-width: 200px;
		line-height: 1.5;
	}
</style>
