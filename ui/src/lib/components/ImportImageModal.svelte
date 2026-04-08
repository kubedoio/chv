<script lang="ts">
	import { createAPIClient, getStoredToken } from '$lib/api/client';
	import Modal from './Modal.svelte';
	import FormField from './FormField.svelte';
	import Input from './Input.svelte';
	import { toast } from '$lib/stores/toast';

	interface Props {
		open?: boolean;
		onSuccess?: () => void;
	}

	let { open = $bindable(false), onSuccess }: Props = $props();

	const client = createAPIClient({ token: getStoredToken() ?? undefined });

	let name = $state('');
	let sourceUrl = $state('');
	let checksum = $state('');
	let osFamily = $state('linux');
	let architecture = $state('x86_64');
	let submitting = $state(false);
	let formError = $state('');

	function reset() {
		name = '';
		sourceUrl = '';
		checksum = '';
		osFamily = 'linux';
		architecture = 'x86_64';
		formError = '';
	}

	async function handleSubmit() {
		formError = '';

		if (!name || !sourceUrl) {
			formError = 'Name and source URL are required';
			return;
		}

		submitting = true;
		try {
			await client.importImage({
				name,
				source_url: sourceUrl,
				checksum: checksum || undefined,
				os_family: osFamily,
				architecture: architecture,
				format: 'qcow2' // Locked to qcow2 for MVP
			});

			toast.success('Image import started');
			open = false;
			reset();
			onSuccess?.();
		} catch (e: any) {
			formError = e.message || 'Import failed';
			toast.error(`Import failed: ${e.message}`);
		} finally {
			submitting = false;
		}
	}

	$effect(() => {
		if (!open) reset();
	});
</script>

<Modal bind:open title="Import Image">
	<form onsubmit={handleSubmit} class="space-y-4">
		{#if formError}
			<div class="error-banner">{formError}</div>
		{/if}

		<FormField label="Name" required>
			<Input bind:value={name} placeholder="ubuntu-22.04" />
		</FormField>

		<FormField label="Source URL" required helper="URL to qcow2 image">
			<Input bind:value={sourceUrl} placeholder="https://cloud-images.ubuntu.com/..." />
		</FormField>

		<FormField label="Checksum" helper="sha256:hash (optional)">
			<Input bind:value={checksum} placeholder="sha256:abc123..." />
		</FormField>

		<FormField label="OS Family">
			<select bind:value={osFamily} class="w-full border border-line rounded px-3 py-2 text-sm">
				<option value="linux">Linux</option>
			</select>
		</FormField>

		<FormField label="Architecture">
			<select bind:value={architecture} class="w-full border border-line rounded px-3 py-2 text-sm">
				<option value="x86_64">x86_64</option>
				<option value="aarch64">aarch64</option>
			</select>
		</FormField>

		<FormField label="Format">
			<Input value="qcow2" disabled />
			<p class="text-xs text-muted mt-1">Only qcow2 is supported in MVP-1</p>
		</FormField>
	</form>

	{#snippet footer()}
		<button type="button" onclick={() => (open = false)} class="button-secondary px-4 py-2 rounded text-sm">Cancel</button>
		<button type="button" onclick={handleSubmit} disabled={submitting} class="button-primary px-4 py-2 rounded text-sm">
			{submitting ? 'Starting...' : 'Start Import'}
		</button>
	{/snippet}
</Modal>

<style>
	.error-banner {
		background: #fff0f0;
		border: 1px solid #e60000;
		color: #e60000;
		padding: 12px;
		border-radius: 4px;
	}
</style>
