<script lang="ts">
import Button from '$lib/components/primitives/Button.svelte';
	import { onMount } from 'svelte';
	import { Users, Plus, Pencil, Trash2, ShieldCheck, UserCheck, Key, ShieldAlert } from 'lucide-svelte';
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
  import CompactMetricCard from '$lib/components/shared/CompactMetricCard.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
  import InventoryTable from '$lib/components/shell/InventoryTable.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import Modal from '$lib/components/primitives/Modal.svelte';
	import ConfirmAction from '$lib/components/shared/ConfirmAction.svelte';
	import { toast } from '$lib/stores/toast';
	import { getStoredToken } from '$lib/api/client';
	import { listUsers, createUser, updateUser, deleteUser, type UserItem } from '$lib/bff/users';
	import type { ShellTone } from '$lib/shell/app-shell';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const token = getStoredToken() ?? undefined;

	let users: UserItem[] = $state([]);
	let loading = $state(false);
	let error = $state(false);

	$effect(() => {
		users = data.model?.users ?? [];
		error = data.model?.state === 'error';
	});

	let createOpen = $state(false);
	let editOpen = $state(false);
	let deleteOpen = $state(false);
	let selectedUser = $state<UserItem | null>(null);

	let createForm = $state({
		username: '', password: '', role: 'viewer', display_name: '', email: '',
		submitting: false, error: ''
	});

	let editForm = $state({
		role: '', display_name: '', email: '', password: '',
		submitting: false, error: ''
	});

	const pageDef = {
		title: 'Access Control Matrix',
		eyebrow: 'SET_IDENTITY_REGISTRY',
		description: 'Manage operator identities, RBAC assignments, and control plane access.',
		icon: Users,
		badges: [
			{ label: 'ADMIN_ONLY', tone: 'warning' as ShellTone },
			{ label: 'AUDITABLE', tone: 'healthy' as ShellTone }
		]
	};

	function roleTone(role: string): ShellTone {
		switch (role) {
			case 'admin': return 'warning';
			case 'operator': return 'healthy';
			default: return 'neutral' as any;
		}
	}

	async function reloadUsers() {
		loading = true;
		error = false;
		try {
			const res = await listUsers(token);
			users = res.items ?? [];
		} catch (err: any) {
			error = true;
			toast.error('Identity registry sync failed');
		} finally {
			loading = false;
		}
	}

	function openCreate() {
		createForm = { username: '', password: '', role: 'viewer', display_name: '', email: '', submitting: false, error: '' };
		createOpen = true;
	}

	function openEdit(user: UserItem) {
		selectedUser = user;
		editForm = { role: user.role, display_name: user.display_name ?? '', email: user.email ?? '', password: '', submitting: false, error: '' };
		editOpen = true;
	}

	async function handleCreate() {
		if (!createForm.username.trim()) { createForm.error = 'IDENTITY_ID_REQUIRED'; return; }
		if (createForm.password.length < 8) { createForm.error = 'PWD_MIN_LENGTH_ERR'; return; }
		createForm.submitting = true;
		try {
			await createUser({
				username: createForm.username.trim(), password: createForm.password,
				role: createForm.role, display_name: createForm.display_name.trim() || undefined,
				email: createForm.email.trim() || undefined
			}, token);
			toast.success('Identity created');
			createOpen = false;
			await reloadUsers();
		} catch (err: any) {
			createForm.error = err.message || 'Identity creation failed';
		} finally { createForm.submitting = false; }
	}

	async function handleEdit() {
		if (!selectedUser) return;
		editForm.submitting = true;
		try {
			await updateUser({
				user_id: selectedUser.user_id, role: editForm.role || undefined,
				display_name: editForm.display_name.trim() || undefined,
				email: editForm.email.trim() || undefined,
        password: editForm.password.trim() || undefined
			}, token);
			toast.success('Identity updated');
			editOpen = false;
			await reloadUsers();
		} catch (err: any) {
			editForm.error = err.message || 'Identity update failed';
		} finally { editForm.submitting = false; }
	}

	async function handleDelete() {
		if (!selectedUser) return;
		try {
			await deleteUser(selectedUser.user_id, token);
			toast.success('Identity purged');
			await reloadUsers();
		} catch (err: any) {
			toast.error(err.message || 'Identity purge failed');
		}
	}

  const columns = [
    { key: 'username', label: 'Identity ID' },
    { key: 'display_name', label: 'Alias' },
    { key: 'role', label: 'RBAC_Role', align: 'center' as const },
    { key: 'email', label: 'Fabric_Email' },
    { key: 'last_login_at', label: 'Last_Sync' },
    { key: 'actions', label: '', align: 'right' as const }
  ];

  const adminCount = $derived(users.filter(u => u.role === 'admin').length);
</script>

<div class="inventory-page">
	<PageHeaderWithAction page={pageDef as any}>
		{#snippet actions()}
			<div class="header-actions">
        <Button variant="primary" onclick={openCreate}>
          <Plus size={14} />
          ADD_IDENTITY
        </Button>
      </div>
		{/snippet}
	</PageHeaderWithAction>

	{#if error}
		<ErrorState title="Identity registry unreachable" description="Failed to load operator records from the control plane." />
	{:else}
    <div class="inventory-metrics">
			<CompactMetricCard label="Total Identities" value={users.length} color="primary" />
			<CompactMetricCard label="Admin Principals" value={adminCount} color="warning" />
			<CompactMetricCard label="MFA Compliance" value="100%" color="success" />
			<CompactMetricCard label="Registry Sync" value="NOMINAL" color="neutral" />
		</div>

		<main class="inventory-main">
			<div class="settings-content">
				<SectionCard title="Registered Operator Registry" icon={UserCheck}>
					{#if loading}
						<div class="skeleton-table"></div>
					{:else if users.length === 0}
						<p class="empty-hint">Identity registry is currently empty. Initialize first operator.</p>
					{:else}
						<InventoryTable 
							{columns} 
							rows={users.map(u => ({
                ...u,
                role: { label: u.role.toUpperCase(), tone: roleTone(u.role) }
              }))} 
						>
              {#snippet cell({ column, row })}
                {#if column.key === 'username'}
                  <span class="identity-id">{row.username}</span>
                {:else if column.key === 'role'}
                   <StatusBadge label={row.role.label} tone={row.role.tone} />
                {:else if column.key === 'actions'}
                   <div class="action-strip">
                      <button class="btn-icon" onclick={() => openEdit(row as unknown as UserItem)} title="MODIFY_ENTITY">
                        <Pencil size={12} />
                      </button>
                      <button class="btn-icon danger" onclick={() => { selectedUser = row as unknown as UserItem; deleteOpen = true; }} title="PURGE_ENTITY">
                        <Trash2 size={12} />
                      </button>
                   </div>
                {:else}
                   <span class="cell-text">{(row as Record<string, unknown>)[column.key] || '—'}</span>
                {/if}
              {/snippet}
            </InventoryTable>
					{/if}
				</SectionCard>
			</div>

			<aside class="support-area">
				<SectionCard title="Security Domain" icon={ShieldCheck}>
					<div class="domain-ops">
						<div class="safety-sign">
							<ShieldCheck size={16} />
							<span>RBAC_ENFORCED</span>
						</div>
            <p class="meta-hint">Identity federation is limited to local fabric database.</p>
					</div>
				</SectionCard>

        <SectionCard title="Fabric Roles" icon={Key}>
          <div class="role-matrix">
            <div class="role-item">
              <span class="tag warning">ADMIN</span>
              <span class="desc">UNRESTRICTED_ACCESS</span>
            </div>
            <div class="role-item">
              <span class="tag healthy">OPERATOR</span>
              <span class="desc">MUTATION_ACCESS</span>
            </div>
            <div class="role-item">
              <span class="tag neutral">VIEWER</span>
              <span class="desc">READ_ONLY_ACCESS</span>
            </div>
          </div>
        </SectionCard>

        <SectionCard title="Danger Zone" icon={ShieldAlert}>
           <p class="meta-hint">Account purges are permanent and logged in fabric audit.</p>
        </SectionCard>
			</aside>
		</main>
	{/if}
</div>

<Modal bind:open={createOpen} title="NEW_IDENTITY_ENTRY">
	{#snippet children()}
		<div class="params-column">
			<div class="field-control">
				<label class="label">IDENTITY_ID</label>
				<input type="text" bind:value={createForm.username} placeholder="operator_id" />
			</div>
			<div class="field-control">
				<label class="label">ACCESS_PWD</label>
				<input type="password" bind:value={createForm.password} placeholder="min_8_chars" />
			</div>
			<div class="field-control">
				<label class="label">RBAC_ROLE</label>
				<select bind:value={createForm.role}>
					<option value="viewer">VIEWER</option>
					<option value="operator">OPERATOR</option>
					<option value="admin">ADMIN</option>
				</select>
			</div>
			<div class="field-control">
				<label class="label">ALIAS</label>
				<input type="text" bind:value={createForm.display_name} placeholder="optional_name" />
			</div>
			<div class="field-control">
				<label class="label">EMAIL_LINK</label>
				<input type="email" bind:value={createForm.email} placeholder="optional_mail" />
			</div>
			{#if createForm.error}
				<div class="form-error-row">
          <ShieldAlert size={12} />
          <span>{createForm.error}</span>
        </div>
			{/if}
		</div>
	{/snippet}
	{#snippet footer()}
		<Button onclick={() => createOpen = false} variant="secondary">ABORT</Button>
		<Button onclick={handleCreate} disabled={createForm.submitting}  variant="primary">
			{createForm.submitting ? 'EXECUTING...' : 'COMMIT_ENTRY'}
		</Button>
	{/snippet}
</Modal>

<Modal bind:open={editOpen} title="MODIFY_IDENTITY_ENTRY">
	{#snippet children()}
		{#if selectedUser}
			<div class="params-column">
				<div class="field-control">
					<label class="label">IDENTITY_ID</label>
					<input type="text" value={selectedUser.username} disabled class="locked" />
				</div>
				<div class="field-control">
					<label class="label">RBAC_ROLE</label>
					<select bind:value={editForm.role}>
						<option value="viewer">VIEWER</option>
						<option value="operator">OPERATOR</option>
						<option value="admin">ADMIN</option>
					</select>
				</div>
				<div class="field-control">
					<label class="label">ALIAS</label>
					<input type="text" bind:value={editForm.display_name} />
				</div>
				<div class="field-control">
					<label class="label">EMAIL_LINK</label>
					<input type="email" bind:value={editForm.email} />
				</div>
				<div class="field-control">
					<label class="label">RESET_PWD</label>
					<input type="password" bind:value={editForm.password} placeholder="leave_blank_to_keep" />
				</div>
				{#if editForm.error}
					<div class="form-error-row">
            <ShieldAlert size={12} />
            <span>{editForm.error}</span>
          </div>
				{/if}
			</div>
		{/if}
	{/snippet}
	{#snippet footer()}
		<Button onclick={() => editOpen = false} variant="secondary">ABORT</Button>
		<Button onclick={handleEdit} disabled={editForm.submitting}  variant="primary">
			{editForm.submitting ? 'EXECUTING...' : 'PATCH_ENTRY'}
		</Button>
	{/snippet}
</Modal>

<ConfirmAction
	bind:open={deleteOpen}
	title="PURGE_IDENTITY"
	description={selectedUser ? `Permanent removal of principal "${selectedUser.username}". Continue?` : ''}
	actionType="delete"
	confirmText="PURGE_PRINCIPAL"
	onConfirm={handleDelete}
/>



<!-- Create User Modal -->
<Modal bind:open={createOpen} title="Create User">
	{#snippet children()}
		<form onsubmit={(e) => { e.preventDefault(); handleCreate(); }} class="form-fields">
			<div class="field">
				<label for="create-username">Username</label>
				<input id="create-username" type="text" bind:value={createForm.username} placeholder="e.g. jdoe" autocomplete="off" />
			</div>
			<div class="field">
				<label for="create-password">Password</label>
				<input id="create-password" type="password" bind:value={createForm.password} placeholder="Min 8 characters" autocomplete="new-password" />
			</div>
			<div class="field">
				<label for="create-role">Role</label>
				<select id="create-role" bind:value={createForm.role}>
					<option value="viewer">Viewer</option>
					<option value="operator">Operator</option>
					<option value="admin">Admin</option>
				</select>
			</div>
			<div class="field">
				<label for="create-display-name">Display Name (optional)</label>
				<input id="create-display-name" type="text" bind:value={createForm.display_name} placeholder="e.g. Jane Doe" autocomplete="off" />
			</div>
			<div class="field">
				<label for="create-email">Email (optional)</label>
				<input id="create-email" type="email" bind:value={createForm.email} placeholder="user@example.com" autocomplete="off" />
			</div>
			{#if createForm.error}
				<div class="form-error">{createForm.error}</div>
			{/if}
		</form>
	{/snippet}
	{#snippet footer()}
		<Button type="button" onclick={() => createOpen = false} variant="secondary">Cancel</Button>
		<Button type="button" onclick={handleCreate} disabled={createForm.submitting}  variant="primary">
			{createForm.submitting ? 'Creating...' : 'Create User'}
		</Button>
	{/snippet}
</Modal>

<!-- Edit User Modal -->
<Modal bind:open={editOpen} title="Edit User">
	{#snippet children()}
		{#if selectedUser}
			<form onsubmit={(e) => { e.preventDefault(); handleEdit(); }} class="form-fields">
				<div class="field">
					<label for="edit-username">Username</label>
					<input id="edit-username" type="text" value={selectedUser.username} disabled class="disabled-input" />
				</div>
				<div class="field">
					<label for="edit-role">Role</label>
					<select id="edit-role" bind:value={editForm.role}>
						<option value="viewer">Viewer</option>
						<option value="operator">Operator</option>
						<option value="admin">Admin</option>
					</select>
				</div>
				<div class="field">
					<label for="edit-display-name">Display Name</label>
					<input id="edit-display-name" type="text" bind:value={editForm.display_name} placeholder="e.g. Jane Doe" />
				</div>
				<div class="field">
					<label for="edit-email">Email</label>
					<input id="edit-email" type="email" bind:value={editForm.email} placeholder="user@example.com" />
				</div>
				<div class="field">
					<label for="edit-password">New Password (leave blank to keep current)</label>
					<input id="edit-password" type="password" bind:value={editForm.password} placeholder="Min 8 characters" autocomplete="new-password" />
				</div>
				{#if editForm.error}
					<div class="form-error">{editForm.error}</div>
				{/if}
			</form>
		{/if}
	{/snippet}
	{#snippet footer()}
		<Button type="button" onclick={() => editOpen = false} variant="secondary">Cancel</Button>
		<Button type="button" onclick={handleEdit} disabled={editForm.submitting}  variant="primary">
			{editForm.submitting ? 'Saving...' : 'Save Changes'}
		</Button>
	{/snippet}
</Modal>

<!-- Delete Confirmation -->
<ConfirmAction
	bind:open={deleteOpen}
	title="Delete User"
	description={selectedUser ? `Delete user "${selectedUser.username}"? This action cannot be undone.` : ''}
	actionType="delete"
	confirmText="Delete User"
	onConfirm={handleDelete}
/>

<style>
	.inventory-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

  .header-actions {
    display: flex;
    align-items: center;
  }

	.inventory-metrics {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: 0.75rem;
	}

	.inventory-main {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: 1rem;
		align-items: start;
	}

	.settings-content {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

  .identity-id {
    font-weight: 800;
    font-family: var(--font-mono);
    color: var(--color-neutral-900);
  }

  .cell-text {
    font-size: 11px;
    color: var(--color-neutral-600);
  }

  .action-strip {
    display: flex;
    gap: 0.25rem;
    justify-content: flex-end;
  }

	.btn-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.5rem;
		height: 1.5rem;
		border-radius: 2px;
		border: 1px solid var(--border-subtle);
		background: var(--bg-surface);
		color: var(--color-neutral-500);
		cursor: pointer;
	}

	.btn-icon:hover {
		background: var(--bg-surface-muted);
		color: var(--color-neutral-900);
	}

	.btn-icon.danger:hover {
		background: rgba(var(--color-danger-rgb), 0.1);
		color: var(--color-danger);
		border-color: var(--color-danger);
	}

	.support-area {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

  .safety-sign {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem;
		background: rgba(var(--color-success-rgb), 0.1);
		color: var(--color-success);
		font-size: 10px;
		font-weight: 800;
		border-radius: var(--radius-xs);
	}

  .meta-hint {
    font-size: 10px;
    color: var(--color-neutral-500);
    margin-top: 0.5rem;
    line-height: 1.4;
  }

  .role-matrix {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .role-item {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .tag {
    font-size: 9px;
    font-weight: 800;
    padding: 0.125rem 0.375rem;
    border-radius: 2px;
    min-width: 60px;
    text-align: center;
  }

  .tag.warning { background: rgba(var(--color-warning-rgb), 0.1); color: var(--color-warning); }
  .tag.healthy { background: rgba(var(--color-success-rgb), 0.1); color: var(--color-success); }
  .tag.neutral { background: var(--bg-surface-muted); color: var(--color-neutral-500); }

  .role-item .desc {
    font-size: 9px;
    font-weight: 700;
    color: var(--color-neutral-400);
  }

  .params-column {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 0.5rem 0;
  }

  .field-control {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .field-control .label {
    font-size: 10px;
    font-weight: 800;
    color: var(--color-neutral-500);
  }

  .field-control input, .field-control select {
    background: var(--bg-surface-muted);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-xs);
    padding: 0.4rem 0.625rem;
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-neutral-900);
    width: 100%;
  }

  .field-control input.locked {
    opacity: 0.6;
    background: var(--bg-surface-muted);
    cursor: not-allowed;
  }

  .form-error-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem;
    background: rgba(var(--color-danger-rgb), 0.1);
    color: var(--color-danger);
    font-size: 10px;
    font-weight: 800;
    border-radius: 2px;
  }

	.empty-hint {
		font-size: 11px;
		color: var(--color-neutral-400);
		padding: 2rem;
		text-align: center;
	}

  .w-full { width: 100%; justify-content: center; }

	@media (max-width: 1100px) {
		.inventory-main {
			grid-template-columns: 1fr;
		}
	}

	.users-page {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.loading-state,
	.empty-state {
		padding: 2rem;
		text-align: center;
		color: var(--shell-text-muted);
		font-size: var(--text-sm);
	}

	.users-table-wrap {
		overflow-x: auto;
	}

	.users-table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
	}

	.users-table th {
		text-align: left;
		padding: 0.5rem 0.75rem;
		font-size: 10px;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--shell-text-muted);
		border-bottom: 1px solid var(--shell-line);
		white-space: nowrap;
	}

	.users-table td {
		padding: 0.625rem 0.75rem;
		border-bottom: 1px solid var(--shell-line);
		color: var(--shell-text);
		vertical-align: middle;
	}

	.users-table tbody tr:last-child td {
		border-bottom: none;
	}

	.users-table tbody tr:hover {
		background: var(--shell-surface-muted);
	}

	.username-cell {
		font-weight: 500;
		font-family: var(--font-mono, monospace);
	}

	.email-cell {
		color: var(--shell-text-secondary);
		font-size: 11px;
	}

	.action-buttons {
		display: flex;
		align-items: center;
		gap: 0.25rem;
	}

	.btn-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.75rem;
		height: 1.75rem;
		border-radius: 0.25rem;
		border: 1px solid var(--shell-line);
		background: transparent;
		color: var(--shell-text-secondary);
		cursor: pointer;
		transition: background 0.1s, color 0.1s;
	}

	.btn-icon:hover {
		background: var(--shell-surface-muted);
		color: var(--shell-text);
	}

	.btn-icon--danger:hover {
		background: var(--status-failed-bg);
		border-color: var(--status-failed-border);
		color: var(--status-failed-text);
	}

	.btn-primary {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		padding: 0.4rem 0.875rem;
		border-radius: 0.25rem;
		background: var(--shell-accent);
		color: white;
		font-size: var(--text-sm);
		font-weight: 500;
		border: none;
		cursor: pointer;
		transition: opacity 0.1s;
	}

	.btn-primary:hover {
		opacity: 0.88;
	}

	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-secondary {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		padding: 0.4rem 0.875rem;
		border-radius: 0.25rem;
		background: transparent;
		color: var(--shell-text);
		font-size: var(--text-sm);
		font-weight: 500;
		border: 1px solid var(--shell-line);
		cursor: pointer;
		transition: background 0.1s;
	}

	.btn-secondary:hover {
		background: var(--shell-surface-muted);
	}

	.form-fields {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
	}

	.field label {
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--shell-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.field input,
	.field select {
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--shell-line-strong);
		border-radius: 0.25rem;
		background: var(--shell-surface);
		color: var(--shell-text);
		font-size: var(--text-sm);
		outline: none;
		transition: border-color 0.15s;
	}

	.field input:focus,
	.field select:focus {
		border-color: var(--shell-accent);
	}

	.disabled-input {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.form-error {
		padding: 0.5rem 0.75rem;
		background: var(--status-failed-bg);
		border: 1px solid var(--status-failed-border);
		border-radius: 0.25rem;
		color: var(--status-failed-text);
		font-size: var(--text-sm);
	}
</style>
