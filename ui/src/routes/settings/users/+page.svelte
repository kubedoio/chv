<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { Users, Plus, Pencil, Trash2 } from 'lucide-svelte';
	import PageHeaderWithAction from '$lib/components/shell/PageHeaderWithAction.svelte';
	import SectionCard from '$lib/components/shell/SectionCard.svelte';
	import StatusBadge from '$lib/components/shell/StatusBadge.svelte';
	import ErrorState from '$lib/components/shell/ErrorState.svelte';
	import Modal from '$lib/components/modals/Modal.svelte';
	import ConfirmAction from '$lib/components/modals/ConfirmAction.svelte';
	import { toast } from '$lib/stores/toast';
	import { getStoredToken, getStoredRole } from '$lib/api/client';
	import { listUsers, createUser, updateUser, deleteUser, type UserItem } from '$lib/bff/users';
	import type { ShellTone } from '$lib/shell/app-shell';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const token = getStoredToken() ?? undefined;

	// Users state
	let users: UserItem[] = $state([]);
	let loading = $state(false);
	let error = $state(false);

	$effect(() => {
		users = data.model?.users ?? [];
		error = data.model?.state === 'error';
	});

	// Modal state
	let createOpen = $state(false);
	let editOpen = $state(false);
	let deleteOpen = $state(false);
	let selectedUser = $state<UserItem | null>(null);

	// Create form state
	let createForm = $state({
		username: '',
		password: '',
		role: 'viewer',
		display_name: '',
		email: '',
		submitting: false,
		error: ''
	});

	// Edit form state
	let editForm = $state({
		role: '',
		display_name: '',
		email: '',
		password: '',
		submitting: false,
		error: ''
	});

	const currentRole = getStoredRole();

	// Page definition for header
	const page = {
		href: '/settings/users',
		navLabel: 'User Management',
		shortLabel: 'Users',
		title: 'User Management',
		eyebrow: 'Administration',
		description: 'Manage operator accounts, roles, and access to the control plane.',
		icon: Users,
		badges: [
			{ label: 'Admin Only', tone: 'warning' as ShellTone },
			{ label: 'Auditable', tone: 'healthy' as ShellTone }
		],
		summary: [],
		focusAreas: [],
		states: {
			loading: { title: '', description: '', hint: '' },
			empty: { title: '', description: '', hint: '' },
			error: { title: '', description: '', hint: '' }
		}
	};

	function roleTone(role: string): ShellTone {
		switch (role) {
			case 'admin': return 'warning';
			case 'operator': return 'healthy';
			case 'viewer': return 'unknown';
			default: return 'unknown';
		}
	}

	function formatDate(dateStr: string | null | undefined): string {
		if (!dateStr) return '—';
		try {
			return new Date(dateStr).toLocaleDateString('en-US', {
				year: 'numeric',
				month: 'short',
				day: 'numeric'
			});
		} catch {
			return dateStr;
		}
	}

	async function reloadUsers() {
		loading = true;
		error = false;
		try {
			const res = await listUsers(token);
			users = res.items ?? [];
		} catch {
			error = true;
			toast.error('Failed to load users');
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

	function openDelete(user: UserItem) {
		selectedUser = user;
		deleteOpen = true;
	}

	async function handleCreate() {
		if (!createForm.username.trim()) {
			createForm.error = 'Username is required';
			return;
		}
		if (createForm.password.length < 8) {
			createForm.error = 'Password must be at least 8 characters';
			return;
		}
		createForm.submitting = true;
		createForm.error = '';
		try {
			await createUser({
				username: createForm.username.trim(),
				password: createForm.password,
				role: createForm.role,
				display_name: createForm.display_name.trim() || undefined,
				email: createForm.email.trim() || undefined
			}, token);
			toast.success('User created');
			createOpen = false;
			await reloadUsers();
		} catch (err: any) {
			createForm.error = err?.message ?? 'Failed to create user';
		} finally {
			createForm.submitting = false;
		}
	}

	async function handleEdit() {
		if (!selectedUser) return;
		editForm.submitting = true;
		editForm.error = '';
		try {
			await updateUser({
				user_id: selectedUser.user_id,
				role: editForm.role || undefined,
				display_name: editForm.display_name.trim() || undefined,
				email: editForm.email.trim() || undefined,
				password: editForm.password.trim() || undefined
			}, token);
			toast.success('User updated');
			editOpen = false;
			await reloadUsers();
		} catch (err: any) {
			editForm.error = err?.message ?? 'Failed to update user';
		} finally {
			editForm.submitting = false;
		}
	}

	async function handleDelete() {
		if (!selectedUser) return;
		try {
			await deleteUser(selectedUser.user_id, token);
			toast.success('User deleted');
			await reloadUsers();
		} catch (err: any) {
			toast.error(err?.message ?? 'Failed to delete user');
		}
	}

	onMount(() => {
		if (!token) {
			goto('/login');
			return;
		}
		if (currentRole !== 'admin') {
			goto('/settings');
			return;
		}
	});
</script>

<div class="users-page">
	<PageHeaderWithAction page={page}>
		{#snippet actions()}
			<button
				onclick={openCreate}
				class="btn-primary"
			>
				<Plus size={14} />
				Create User
			</button>
		{/snippet}
	</PageHeaderWithAction>

	{#if error}
		<ErrorState title="Users Unavailable" description="Failed to load user accounts from the control plane." />
	{:else}
		<SectionCard title="Registered Operators" icon={Users}>
			{#if loading}
				<div class="loading-state">Loading users...</div>
			{:else if users.length === 0}
				<div class="empty-state">
					<p>No users have been created yet. Create the first operator account.</p>
				</div>
			{:else}
				<div class="users-table-wrap">
					<table class="users-table">
						<thead>
							<tr>
								<th>Username</th>
								<th>Display Name</th>
								<th>Role</th>
								<th>Email</th>
								<th>Last Login</th>
								<th>Actions</th>
							</tr>
						</thead>
						<tbody>
							{#each users as user}
								<tr>
									<td class="username-cell">{user.username}</td>
									<td>{user.display_name ?? '—'}</td>
									<td>
										<StatusBadge label={user.role} tone={roleTone(user.role)} />
									</td>
									<td class="email-cell">{user.email ?? '—'}</td>
									<td>{formatDate(user.last_login_at)}</td>
									<td>
										<div class="action-buttons">
											<button
												onclick={() => openEdit(user)}
												class="btn-icon"
												title="Edit user"
											>
												<Pencil size={14} />
											</button>
											<button
												onclick={() => openDelete(user)}
												class="btn-icon btn-icon--danger"
												title="Delete user"
											>
												<Trash2 size={14} />
											</button>
										</div>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</SectionCard>
	{/if}
</div>

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
		<button type="button" onclick={() => createOpen = false} class="btn-secondary">Cancel</button>
		<button type="button" onclick={handleCreate} disabled={createForm.submitting} class="btn-primary">
			{createForm.submitting ? 'Creating...' : 'Create User'}
		</button>
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
		<button type="button" onclick={() => editOpen = false} class="btn-secondary">Cancel</button>
		<button type="button" onclick={handleEdit} disabled={editForm.submitting} class="btn-primary">
			{editForm.submitting ? 'Saving...' : 'Save Changes'}
		</button>
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
