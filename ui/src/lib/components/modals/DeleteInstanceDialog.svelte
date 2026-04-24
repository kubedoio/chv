<script lang="ts">
	import Modal from './Modal.svelte';
	import { AlertTriangle } from 'lucide-svelte';
	import Button from '$lib/components/primitives/Button.svelte';

	interface Props {
		open?: boolean;
		instanceName: string;
		instanceId: string;
		onConfirm: () => void;
		onCancel: () => void;
	}

	let {
		open = $bindable(false),
		instanceName,
		instanceId,
		onConfirm,
		onCancel
	}: Props = $props();

	let confirmText = $state('');
	let isDeleting = $state(false);

	const trimmedConfirm = $derived(confirmText.trim());
	const canDelete = $derived(trimmedConfirm === instanceName);

	async function handleConfirm() {
		if (!canDelete) return;
		isDeleting = true;
		try {
			onConfirm();
		} finally {
			isDeleting = false;
			confirmText = '';
		}
	}

	function handleCancel() {
		confirmText = '';
		isDeleting = false;
		onCancel();
	}

	$effect(() => {
		if (!open) {
			confirmText = '';
			isDeleting = false;
		}
	});
</script>

<Modal bind:open closeOnBackdrop={true} onClose={handleCancel}>
	{#snippet header()}
		<div class="flex items-center gap-3">
			<AlertTriangle class="h-5 w-5 text-[var(--color-danger)]" aria-hidden="true" />
			<h2 id="modal-title" class="text-base font-semibold text-[var(--shell-text)]">
				Delete instance "{instanceName}"?
			</h2>
		</div>
	{/snippet}

	<div class="space-y-4">
		<p class="text-sm text-[var(--shell-text-secondary)]">
			This permanently removes the instance configuration and selected related resources. This action
			cannot be undone.
		</p>

		<div class="bg-[var(--shell-surface-muted)] rounded-lg p-3 border border-[var(--shell-line)]">
			<p class="text-xs font-medium text-[var(--shell-text-muted)] uppercase tracking-wider mb-2">
				This will delete
			</p>
			<ul class="space-y-1 text-sm text-[var(--shell-text)]">
				<li class="flex items-center gap-2">
					<span class="w-1 h-1 rounded-full bg-[var(--color-danger)]" aria-hidden="true"></span>
					Instance configuration
				</li>
				<li class="flex items-center gap-2">
					<span class="w-1 h-1 rounded-full bg-[var(--color-danger)]" aria-hidden="true"></span>
					Root disk
				</li>
				<li class="flex items-center gap-2">
					<span class="w-1 h-1 rounded-full bg-[var(--color-danger)]" aria-hidden="true"></span>
					Cloud-init disk
				</li>
				<li class="flex items-center gap-2">
					<span class="w-1 h-1 rounded-full bg-[var(--color-danger)]" aria-hidden="true"></span>
					Runtime state
				</li>
			</ul>
		</div>

		<div class="space-y-2 pt-2 border-t border-[var(--shell-line)]">
			<p class="text-sm text-[var(--shell-text-secondary)]">
				To confirm, type <code class="bg-[var(--shell-surface-muted)] px-1.5 py-0.5 rounded text-xs font-mono border border-[var(--shell-line)]">{instanceName}</code>
				below:
			</p>
			<input
				type="text"
				bind:value={confirmText}
				placeholder={`Type "${instanceName}" to confirm`}
				class="w-full px-3 py-2 text-sm border border-[var(--shell-line)] rounded bg-[var(--shell-surface)] text-[var(--shell-text)] placeholder:text-[var(--shell-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]/30 focus:border-[var(--color-primary)]"
				aria-label="Type instance name to confirm deletion"
			/>
		</div>
	</div>

	{#snippet footer()}
		<Button variant="secondary" size="sm" onclick={handleCancel}>Cancel</Button>
		<Button
			variant="danger"
			size="sm"
			disabled={!canDelete || isDeleting}
			loading={isDeleting}
			onclick={handleConfirm}
			ariaLabel="Permanently delete instance {instanceName}"
		>
			{isDeleting ? 'Deleting…' : 'Delete Instance'}
		</Button>
	{/snippet}
</Modal>
