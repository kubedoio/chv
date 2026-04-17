<script lang="ts">
	interface Property {
		label: string;
		value: string | number;
		subtext?: string;
		tone?: 'healthy' | 'warning' | 'degraded' | 'failed' | 'neutral';
	}

	interface Props {
		title?: string;
		properties: Property[];
		columns?: number;
	}

	let { title, properties, columns = 2 }: Props = $props();
</script>

<section class="property-grid-container">
	{#if title}<h3 class="grid-title">{title}</h3>{/if}
	
	<div class="property-grid" style="--cols: {columns}">
		{#each properties as prop}
			<div class="property-item">
				<span class="label">{prop.label}</span>
				<div class="value-container">
					<span class="value" class:tone-healthy={prop.tone === 'healthy'} class:tone-warning={prop.tone === 'warning'} class:tone-failed={prop.tone === 'failed'}>
						{prop.value}
					</span>
					{#if prop.subtext}
						<span class="subtext">{prop.subtext}</span>
					{/if}
				</div>
			</div>
		{/each}
	</div>
</section>

<style>
	.property-grid-container {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.grid-title {
		font-size: var(--text-xs);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--shell-text-muted);
		border-bottom: 1px solid var(--shell-line);
		padding-bottom: 0.25rem;
		margin: 0;
	}

	.property-grid {
		display: grid;
		grid-template-columns: repeat(var(--cols), 1fr);
		gap: 1rem;
	}

	.property-item {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
	}

	.label {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
		font-weight: 500;
	}

	.value-container {
		display: flex;
		flex-direction: column;
	}

	.value {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--shell-text);
	}

	.subtext {
		font-size: var(--text-xs);
		color: var(--shell-text-muted);
	}

	.tone-healthy { color: var(--color-success); }
	.tone-warning { color: var(--color-warning-dark); }
	.tone-failed { color: var(--color-danger); }

	@media (max-width: 600px) {
		.property-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
