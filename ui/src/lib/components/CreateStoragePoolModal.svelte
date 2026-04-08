<script lang="ts">
	import Modal from '$lib/components/Modal.svelte';
	import FormField from '$lib/components/FormField.svelte';
	import Input from '$lib/components/Input.svelte';
	import Select from '$lib/components/Select.svelte';
	import { createAPIClient, getStoredToken } from '$lib/api/client';
	import { toast } from '$lib/stores/toast';
	import type { CreateStoragePoolInput } from '$lib/api/types';

	interface Props {
		open?: boolean;
		onSuccess?: () => void;
		existingNames?: string[];
	}

	let { open = $bindable(false), onSuccess, existingNames = [] }: Props = $props();

	const client = createAPIClient({ token: getStoredToken() ?? undefined });

	// Form state
	let name = $state('');
	let poolType = $state('localdisk');
	let path = $state('');
	let capacity = $state('');
	let submitting = $state(false);
	let formError = $state('');

	// Field-specific errors
	let nameError = $state('');
	let pathError = $state('');

	// Validation regex
	const nameRegex = /^[a-z0-9-]+$/;

	const typeOptions = [{ value: 'localdisk', label: 'localdisk' }];

	function resetForm() {
		name = '';
		poolType = 'localdisk';
		path = '';
		capacity = '';
		formError = '';
		nameError = '';
		pathError = '';
	}

	function validateName(): boolean {
		if (!name.trim()) {
			nameError = 'Name is required';
			return false;
		}
		if (!nameRegex.test(name)) {
			nameError = 'Name must contain only lowercase letters, numbers, and hyphens';
			return false;
		}
		if (name.startsWith('-') || name.endsWith('-')) {
			nameError = 'Name cannot start or end with a hyphen';
			return false;
		}
		if (existingNames.includes(name.trim())) {
			nameError = 'A storage pool with this name already exists';
			return false;
		}
		nameError = '';
		return true;
	}

	function validatePath(): boolean {
		if (!path.trim()) {
			pathError = 'Path is required';
			return false;
		}
		if (!path.startsWith('/')) {
			pathError = 'Path must be an absolute path (start with "/")';
			return false;
		}
		pathError = '';
		return true;
	}

	function validate(): boolean {
		const validations = [validateName(), validatePath()];
		return validations.every(Boolean);
	}

	function isValid(): boolean {
		// Quick check without setting errors - used for button disabled state
		if (!name.trim() || !path.trim()) {
			return false;
		}
		if (!nameRegex.test(name) || name.startsWith('-') || name.endsWith('-')) {
			return false;
		}
		if (existingNames.includes(name.trim())) {
			return false;
		}
		if (!path.startsWith('/')) {
			return false;
		}
		return true;
	}

	async function handleSubmit(event?: Event) {
		event?.preventDefault();

		if (!validate()) return;

		submitting = true;
		formError = '';

		const data: CreateStoragePoolInput = {
			name: name.trim(),
			pool_type: 'localdisk',
			path: path.trim()
		};

		// Add capacity if provided
		const capacityNum = capacity.trim() ? parseInt(capacity.trim(), 10) : NaN;
		if (!isNaN(capacityNum) && capacityNum > 0) {
			data.capacity_bytes = capacityNum;
		}

		try {
			await client.createStoragePool(data);
			toast.success(`Storage pool "${name}" created successfully`);
			open = false;
			onSuccess?.();
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Failed to create storage pool';
			formError = message;
			toast.error(message);
		} finally {
			submitting = false;
		}
	}

	// Reset form when modal closes
	$effect(() => {
		if (!open) {
			resetForm();
		}
	});
</script>

<Modal bind:open title="Create Storage Pool" closeOnBackdrop={!submitting}>
	<form id="create-storage-pool-form" onsubmit={handleSubmit} class="space-y-5">
		{#if formError}
			<div class="rounded border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">
				{formError}
			</div>
		{/if}

		<FormField label="Name" error={nameError} required labelFor="pool-name">
			<Input
				id="pool-name"
				bind:value={name}
				placeholder="my-pool"
				disabled={submitting}
				onblur={validateName}
			/>
		</FormField>

		<FormField
			label="Type"
			helper="Only 'localdisk' type is supported in MVP-1"
			labelFor="pool-type"
		>
			<Select id="pool-type" bind:value={poolType} options={typeOptions} disabled />
		</FormField>

		<FormField
			label="Path"
			error={pathError}
			required
			helper="Absolute path on host filesystem"
			labelFor="pool-path"
		>
			<Input
				id="pool-path"
				bind:value={path}
				placeholder="/var/lib/chv/storage/my-pool"
				disabled={submitting}
				onblur={validatePath}
			/>
		</FormField>

		<FormField
			label="Capacity"
			helper="Optional - Storage capacity in bytes (for display only)"
			labelFor="pool-capacity"
		>
			<Input
				id="pool-capacity"
				bind:value={capacity}
				type="number"
				placeholder="10737418240"
				disabled={submitting}
				min="0"
			/>
		</FormField>
	</form>

	{#snippet footer()}
		<button
			type="button"
			onclick={() => (open = false)}
			disabled={submitting}
			class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
		>
			Cancel
		</button>
		<button
			type="submit"
			form="create-storage-pool-form"
			disabled={!isValid() || submitting}
			class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2"
		>
			{#if submitting}
				<svg
					class="animate-spin h-4 w-4"
					xmlns="http://www.w3.org/2000/svg"
					fill="none"
					viewBox="0 0 24 24"
					aria-hidden="true"
				>
					<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
					<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
				</svg>
			{/if}
			{submitting ? 'Creating...' : 'Create Pool'}
		</button>
	{/snippet}
</Modal>
