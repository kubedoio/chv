<script lang="ts">
	import { PageShell, StateBanner, Badge } from '$lib/components/system';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/settings');
	const model = $derived(data.settings);
</script>

<PageShell title={page.title} eyebrow={page.eyebrow} description={page.description}>
	{#if model.state === 'error'}
		<StateBanner
			variant="error"
			title="Settings unavailable"
			description="The settings view could not be loaded."
			hint="Navigation remains available. Retry once the control plane is reachable."
		/>
	{:else}
		<div class="settings-page">
			<section class="settings-section" aria-labelledby="env-title">
				<h2 id="env-title" class="settings-section__title">Environment</h2>
				<div class="settings-card">
					<div class="kv-row">
						<div class="kv-row__label">Control plane version</div>
						<div class="kv-row__value">{model.version}</div>
					</div>
					<div class="kv-row">
						<div class="kv-row__label">Build</div>
						<div class="kv-row__value mono">{model.build}</div>
					</div>
					<div class="kv-row">
						<div class="kv-row__label">Environment</div>
						<div class="kv-row__value"><Badge label={model.environment} tone="healthy" /></div>
					</div>
					<div class="kv-row">
						<div class="kv-row__label">API endpoint</div>
						<div class="kv-row__value mono">{model.api_endpoint}</div>
					</div>
				</div>
			</section>

			<section class="settings-section" aria-labelledby="access-title">
				<h2 id="access-title" class="settings-section__title">Access</h2>
				<div class="settings-card">
					<div class="kv-row">
						<div class="kv-row__label">Session TTL</div>
						<div class="kv-row__value">{model.session_ttl_hours} hours</div>
					</div>
					<div class="kv-row">
						<div class="kv-row__label">Active users</div>
						<div class="kv-row__value">{model.users.length}</div>
					</div>
				</div>
				<div class="user-list">
					{#each model.users as user}
						<div class="user-card">
							<div class="user-card__email">{user.email}</div>
							<Badge label={user.role} tone="unknown" />
						</div>
					{/each}
				</div>
			</section>

			<section class="settings-section" aria-labelledby="audit-title">
				<h2 id="audit-title" class="settings-section__title">Audit and configuration</h2>
				<div class="settings-card">
					<p class="settings-note">
						Advanced access controls, audit logging, and API key management will be available in a future release.
						Current settings are intentionally scoped to essential operational information.
					</p>
				</div>
			</section>
		</div>
	{/if}
</PageShell>

<style>
	.settings-page {
		display: grid;
		gap: 2rem;
		max-width: 800px;
	}

	.settings-section {
		display: grid;
		gap: 0.9rem;
	}

	.settings-section__title {
		font-size: 1rem;
		font-weight: 700;
		color: var(--shell-text);
		margin: 0;
	}

	.settings-card {
		border: 1px solid var(--shell-line);
		border-radius: 1rem;
		background: var(--shell-surface);
		padding: 1rem;
	}

	.kv-row {
		display: grid;
		grid-template-columns: minmax(10rem, 0.5fr) minmax(0, 1fr);
		gap: 0.9rem;
		padding: 0.65rem 0;
		border-bottom: 1px solid var(--shell-line);
		font-size: 0.95rem;
		align-items: center;
	}

	.kv-row:first-child {
		padding-top: 0;
	}

	.kv-row:last-child {
		padding-bottom: 0;
		border-bottom: 0;
	}

	.kv-row__label {
		color: var(--shell-text-muted);
	}

	.kv-row__value {
		color: var(--shell-text);
	}

	.mono {
		font-family: var(--font-mono);
		font-size: 0.9em;
	}

	.user-list {
		display: grid;
		gap: 0.6rem;
	}

	.user-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		border: 1px solid var(--shell-line);
		border-radius: 0.85rem;
		background: var(--shell-surface-muted);
		padding: 0.8rem 1rem;
	}

	.user-card__email {
		font-weight: 500;
		color: var(--shell-text);
	}

	.settings-note {
		margin: 0;
		font-size: 0.92rem;
		line-height: 1.55;
		color: var(--shell-text-secondary);
	}
</style>
