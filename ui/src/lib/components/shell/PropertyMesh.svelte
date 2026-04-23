<script lang="ts">
	interface Props {
		architecture?: string;
		providerType?: string;
		version?: string;
	}

	let { architecture = 'x86_64', providerType = 'LOCAL_HOST', version = 'v3.4.12-rc' }: Props = $props();
</script>

<div class="prop-matrix">
	<div class="prop-row">
		<span class="p-key">ARCHITECTURE</span>
		<span class="p-val">{architecture}</span>
	</div>
	<div class="prop-row">
		<span class="p-key">FABRIC_DOMAIN</span>
		<span class="p-val">{providerType}</span>
	</div>
	<div class="prop-row">
		<span class="p-key">VER_REGISTRY</span>
		<span class="p-val">{version}</span>
	</div>
</div>

<style>
	.prop-matrix {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.prop-row {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
		padding-bottom: 0.35rem;
		border-bottom: 1px solid var(--border-subtle);
	}

	.p-key { color: var(--color-neutral-500); font-weight: 700; }
	.p-val { font-weight: 800; color: var(--color-neutral-900); font-family: var(--font-mono); }
</style>
