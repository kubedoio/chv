<script lang="ts">
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import PropertyGrid from '$lib/components/shell/PropertyGrid.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import { 
		Settings, 
		Shield, 
		Fingerprint, 
		Users as UsersIcon, 
		History, 
		Lock,
		Terminal,
		User,
		Cpu
	} from 'lucide-svelte';
	import { getPageDefinition } from '$lib/shell/app-shell';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const page = getPageDefinition('/settings');
	const model = $derived(data.settings);

	const envProps = $derived([
		{ label: 'Control Plane', value: model.version },
		{ label: 'Build Hash', value: model.build, isMono: true },
		{ label: 'Environment', value: model.environment },
		{ label: 'API Endpoint', value: model.api_endpoint, isMono: true }
	]);

	const accessProps = $derived([
		{ label: 'Session TTL', value: `${model.session_ttl_hours} hours` },
		{ label: 'Active Users', value: String(model.users.length) }
	]);
</script>

<div class="settings-page">
	{#if model.state === 'error'}
		<ErrorState title="Settings Unavailable" description="Failed to retrieve configuration from the control plane." />
	{:else}
		<PageHeaderWithAction page={page} />

		<main class="settings-grid">
			<div class="settings-main">
				<SectionCard title="System Environment" icon={Terminal}>
					<PropertyGrid properties={envProps} columns={2} />
				</SectionCard>

				<SectionCard title="Access & Security" icon={Shield}>
					<PropertyGrid properties={accessProps} columns={2} />
					
					<div class="users-roster">
						<div class="roster-header">Registered Operators</div>
						<ul class="user-list">
							{#each model.users as user}
								<li>
									<div class="user-entry">
										<div class="user-info">
											<User size={12} class="user-icon" />
											<span class="user-email">{user.email}</span>
										</div>
										<StatusBadge label={user.role} tone="unknown" />
									</div>
								</li>
							{/each}
						</ul>
					</div>
				</SectionCard>

				<SectionCard title="Audit and Hardening" icon={Lock}>
					<div class="audit-banner">
						<Fingerprint size={16} />
						<div class="audit-text">
							<strong>Advanced governance pending</strong>
							<p>Audit logging, API key rotation, and RBAC policies will be available in the next control plane release.</p>
						</div>
					</div>
				</SectionCard>
			</div>

			<aside class="settings-side">
				<SectionCard title="Quick Actions" icon={Settings}>
					<div class="action-buttons">
						<button class="btn-secondary w-full justify-start">
							<History size={14} />
							View Audit Logs
						</button>
						<button class="btn-secondary w-full justify-start">
							<Lock size={14} />
							Rotate Root Keys
						</button>
						<a href="/settings/hypervisor" class="btn-secondary w-full justify-start">
							<Cpu size={14} />
							Hypervisor Settings
						</a>
						<button class="btn-secondary w-full justify-start">
							<UsersIcon size={14} />
							Invite Operator
						</button>
					</div>
				</SectionCard>
			</aside>
		</main>
	{/if}
</div>

<style>
	.settings-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.settings-grid {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.settings-main {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.settings-side {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.users-roster {
		margin-top: 1.25rem;
		padding-top: 1rem;
		border-top: 1px solid var(--shell-line);
	}

	.roster-header {
		font-size: 10px;
		font-weight: 700;
		text-transform: uppercase;
		color: var(--shell-text-muted);
		letter-spacing: 0.05em;
		margin-bottom: 0.75rem;
	}

	.user-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.user-entry {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem 0.75rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
		border-radius: 0.25rem;
	}

	.user-info {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.user-icon {
		opacity: 0.5;
	}

	.user-email {
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.audit-banner {
		display: flex;
		gap: 1rem;
		padding: 0.75rem;
		background: var(--shell-surface-muted);
		border: 1px solid var(--shell-line);
		border-radius: 0.35rem;
		color: var(--shell-text-secondary);
	}

	.audit-text strong {
		display: block;
		font-size: var(--text-sm);
		color: var(--shell-text);
		margin-bottom: 0.15rem;
	}

	.audit-text p {
		font-size: 11px;
		line-height: 1.4;
		margin: 0;
	}

	.action-buttons {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.w-full { width: 100%; }
	.justify-start { justify-content: flex-start; }

	@media (max-width: 1100px) {
		.settings-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
