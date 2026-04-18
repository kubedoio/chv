<script lang="ts">
	import Modal from '$lib/components/modals/Modal.svelte';
	import FormField from '$lib/components/forms/FormField.svelte';
	import Input from '$lib/components/Input.svelte';
	import { createVm } from '$lib/bff/vms';
	import { listImages } from '$lib/bff/images';
	import { listNetworks } from '$lib/bff/networks';
	import { getStoredToken } from '$lib/api/client';
	import { toast } from '$lib/stores/toast';

	interface Props {
		open?: boolean;
		onSuccess?: () => void;
	}

	let {
		open = $bindable(false),
		onSuccess
	}: Props = $props();

	let step = $state(1); // 1: Basic, 2: Cloud-init, 3: Review

	// Data loaded from BFF
	let images = $state<{ image_id: string; name: string }[]>([]);
	let networks = $state<{ network_id: string; name: string; dhcp_enabled: boolean; ipam_mode: string; is_default: boolean }[]>([]);
	let loadingData = $state(false);

	// Basic config
	let name = $state('');
	let imageId = $state('');
	let networkId = $state('');
	let vcpu = $state(2);
	let memoryMb = $state(2048);
	let volumeSizeGb = $state(10);

	// Cloud-init
	let userData = $state('#cloud-config\n');
	let username = $state('admin');
	let sshKey = $state('');

	let submitting = $state(false);
	let formError = $state('');

	// Field errors
	let nameError = $state('');

	const nameRegex = /^[a-z0-9-]+$/;

	function resetForm() {
		step = 1;
		name = '';
		imageId = '';
		networkId = '';
		vcpu = 2;
		memoryMb = 2048;
		volumeSizeGb = 10;
		userData = '#cloud-config\n';
		username = 'admin';
		sshKey = '';
		formError = '';
		nameError = '';
	}

	async function loadData() {
		loadingData = true;
		try {
			const token = getStoredToken() ?? undefined;
			const [imgRes, netRes] = await Promise.all([
				listImages(token),
				listNetworks(token)
			]);
			images = (imgRes.items as any[]).map((i) => ({
				image_id: i.image_id as string,
				name: i.name as string
			}));
			networks = (netRes.items as any[]).map((n) => ({
				network_id: n.network_id as string,
				name: n.name as string,
				dhcp_enabled: n.dhcp_enabled as boolean,
				ipam_mode: n.ipam_mode as string,
				is_default: n.is_default as boolean
			}));
			// Pre-select first available image and network
			if (images.length > 0 && !imageId) {
				imageId = images[0].image_id;
			}
			if (networks.length > 0 && !networkId) {
				const defaultNet = networks.find((n) => n.is_default);
				networkId = defaultNet ? defaultNet.network_id : networks[0].network_id;
			}
		} catch (e) {
			console.error('Failed to load images/networks', e);
		} finally {
			loadingData = false;
		}
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
		nameError = '';
		return true;
	}

	function canProceedToStep2(): boolean {
		return (
			name.trim() !== '' &&
			nameRegex.test(name) &&
			!name.startsWith('-') &&
			!name.endsWith('-') &&
			imageId !== '' &&
			(imageId === 'default' || images.length > 0) &&
			networkId !== ''
		);
	}

	async function handleSubmit(event?: Event) {
		event?.preventDefault();

		submitting = true;
		formError = '';

		const token = getStoredToken() ?? undefined;
		const data = {
			name: name.trim(),
			image_id: imageId,
			network_id: networkId || 'default',
			vcpu,
			memory_mb: memoryMb,
			volume_size_gb: volumeSizeGb,
			requested_by: 'webui'
		};

		try {
			await createVm(data, token);
			toast.success('VM created successfully');
			open = false;
			onSuccess?.();
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Failed to create VM';
			formError = message;
			toast.error(message);
		} finally {
			submitting = false;
		}
	}

	// Get selected items for review
	const selectedImage = $derived(images.find((i) => i.image_id === imageId) ?? { name: imageId || '—' });
	const selectedNetwork = $derived(networks.find((n) => n.network_id === networkId) ?? { name: networkId || '—' });

	// Reset form when modal closes; load data when it opens
	$effect(() => {
		if (!open) {
			resetForm();
		} else {
			loadData();
		}
	});
</script>

<Modal bind:open title="Create VM - Step {step} of 3" closeOnBackdrop={!submitting} width="wide">
	{#if step === 1}
		<form id="create-vm-step1" class="space-y-5">
			{#if formError}
				<div
					class="rounded border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger"
					role="alert"
				>
					{formError}
				</div>
			{/if}

			<FormField label="Name" error={nameError} required labelFor="vm-name">
				<Input
					id="vm-name"
					bind:value={name}
					placeholder="my-vm"
					disabled={submitting}
					onblur={validateName}
				/>
			</FormField>

			<FormField label="Image" required labelFor="vm-image">
				{#if loadingData}
					<div class="text-sm text-muted">Loading images...</div>
				{:else if images.length === 0}
					<div class="rounded border border-warning/30 bg-warning/10 px-3 py-2 text-sm text-warning">
						No images available.
						<a href="/images" class="underline" onclick={() => (open = false)}>Go to Images</a>
						to import one first.
					</div>
				{:else}
					<select
						id="vm-image"
						bind:value={imageId}
						class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
						disabled={submitting}
					>
						<option value="">Select an image...</option>
						{#each images as img}
							<option value={img.image_id}>{img.name}</option>
						{/each}
					</select>
				{/if}
			</FormField>

			<FormField label="Network" required labelFor="vm-network">
				{#if loadingData}
					<div class="text-sm text-muted">Loading networks...</div>
				{:else if networks.length === 0}
					<select
						id="vm-network"
						bind:value={networkId}
						class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
						disabled={submitting}
					>
						<option value="default">default (auto-create)</option>
					</select>
					<p class="text-xs text-muted mt-1">A default network will be created automatically.</p>
				{:else}
					<select
						id="vm-network"
						bind:value={networkId}
						class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
						disabled={submitting}
					>
						<option value="">Select a network...</option>
						{#each networks as net}
							<option value={net.network_id}>{net.name}</option>
						{/each}
					</select>
				{/if}
			</FormField>

			<div class="grid grid-cols-2 gap-4">
				<FormField label="vCPUs" labelFor="vm-vcpu">
					<Input
						id="vm-vcpu"
						type="number"
						bind:value={vcpu}
						min={1}
						max={32}
						disabled={submitting}
					/>
				</FormField>
				<FormField label="Memory (MB)" labelFor="vm-memory">
					<Input
						id="vm-memory"
						type="number"
						bind:value={memoryMb}
						min={512}
						step={512}
						disabled={submitting}
					/>
				</FormField>
			</div>

			<FormField label="Disk Size (GB)" labelFor="vm-disk-size">
				<Input
					id="vm-disk-size"
					type="number"
					bind:value={volumeSizeGb}
					min={1}
					max={500}
					disabled={submitting}
				/>
			</FormField>
		</form>
	{:else if step === 2}
		<form id="create-vm-step2" class="space-y-5">
			<FormField label="Username" required labelFor="vm-username">
				<Input
					id="vm-username"
					bind:value={username}
					placeholder="admin"
					disabled={submitting}
				/>
			</FormField>

			<FormField label="SSH Public Key" labelFor="vm-ssh-key">
				<textarea
					id="vm-ssh-key"
					bind:value={sshKey}
					placeholder="ssh-rsa AAAA..."
					class="w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 font-mono text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
					rows={3}
					disabled={submitting}
				></textarea>
			</FormField>

			<FormField label="User Data (cloud-init)" helper="Advanced cloud-config" labelFor="vm-userdata">
				<textarea
					id="vm-userdata"
					bind:value={userData}
					class="w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 font-mono text-sm focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
					rows={6}
					disabled={submitting}
				></textarea>
			</FormField>
		</form>
	{:else}
		<div class="space-y-5">
			<h3 class="text-base font-semibold text-ink">Review Configuration</h3>

			<div class="space-y-3 text-sm">
				<div class="flex justify-between border-b border-line pb-2">
					<span class="text-muted">Name:</span>
					<span class="font-medium text-ink">{name}</span>
				</div>
				<div class="flex justify-between border-b border-line pb-2">
					<span class="text-muted">Image:</span>
					<span class="font-medium text-ink">{selectedImage?.name}</span>
				</div>
				<div class="flex justify-between border-b border-line pb-2">
					<span class="text-muted">Network:</span>
					<span class="font-medium text-ink">{selectedNetwork?.name}</span>
				</div>
				<div class="flex justify-between border-b border-line pb-2">
					<span class="text-muted">Disk:</span>
					<span class="font-medium text-ink">{volumeSizeGb} GB</span>
				</div>
				<div class="flex justify-between border-b border-line pb-2">
					<span class="text-muted">Resources:</span>
					<span class="font-medium text-ink">{vcpu} vCPU, {memoryMb} MB</span>
				</div>
				<div class="flex justify-between border-b border-line pb-2">
					<span class="text-muted">Username:</span>
					<span class="font-medium text-ink">{username}</span>
				</div>
			</div>

			<div class="rounded bg-chrome p-4 text-xs text-muted">
				<p class="font-medium mb-2">This will create:</p>
				<ul class="ml-4 list-disc space-y-1">
					<li>VM with {vcpu} vCPU, {memoryMb} MB RAM</li>
					<li>{volumeSizeGb} GB boot disk</li>
					<li>Network interface on {selectedNetwork?.name}</li>
				</ul>
			</div>
		</div>
	{/if}

	{#snippet footer()}
		{#if step === 1}
			<button
				type="button"
				onclick={() => (open = false)}
				disabled={submitting}
				class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
			>
				Cancel
			</button>
			<button
				type="button"
				onclick={() => (step = 2)}
				disabled={!canProceedToStep2() || submitting}
				class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed"
			>
				Next
			</button>
		{:else if step === 2}
			<button
				type="button"
				onclick={() => (step = 1)}
				disabled={submitting}
				class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
			>
				Back
			</button>
			<button
				type="button"
				onclick={() => (step = 3)}
				disabled={submitting}
				class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed"
			>
				Next
			</button>
		{:else}
			<button
				type="button"
				onclick={() => (step = 2)}
				disabled={submitting}
				class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
			>
				Back
			</button>
			<button
				type="button"
				onclick={handleSubmit}
				disabled={submitting}
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
						<circle
							class="opacity-25"
							cx="12"
							cy="12"
							r="10"
							stroke="currentColor"
							stroke-width="4"
						></circle>
						<path
							class="opacity-75"
							fill="currentColor"
							d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
						></path>
					</svg>
				{/if}
				{submitting ? 'Creating...' : 'Create VM'}
			</button>
		{/if}
	{/snippet}
</Modal>
