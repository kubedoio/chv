<script lang="ts">
	import Modal from '$lib/components/Modal.svelte';
	import FormField from '$lib/components/FormField.svelte';
	import Input from '$lib/components/Input.svelte';
	import { createAPIClient, getStoredToken } from '$lib/api/client';
	import { toast } from '$lib/stores/toast';
	import { onMount } from 'svelte';
	import type { Image, Network, StoragePool, VM, UsageWithQuota } from '$lib/api/types';

	interface Props {
		open?: boolean;
		onSuccess?: () => void;
		images?: Image[];
		pools?: StoragePool[];
		networks?: Network[];
	}

	let {
		open = $bindable(false),
		onSuccess,
		images = [],
		pools = [],
		networks = []
	}: Props = $props();

	const client = createAPIClient({ token: getStoredToken() ?? undefined });

	// Quota checking
	let usageData = $state<UsageWithQuota | null>(null);
	let quotaError = $state<string | null>(null);

	let step = $state(1); // 1: Basic, 2: Cloud-init, 3: Review

	// Basic config
	let name = $state('');
	let imageId = $state('');
	let poolId = $state('');
	let networkId = $state('');
	let vcpu = $state(2);
	let memoryMb = $state(2048);
	let consoleType = $state<'serial'>('serial');

	// Cloud-init
	let userData = $state('#cloud-config\n');
	let username = $state('admin');
	let sshKey = $state('');

	let submitting = $state(false);
	let formError = $state('');
	let checkingQuota = $state(false);

	// Field errors
	let nameError = $state('');

	// Load quota data when modal opens
	$effect(() => {
		if (open) {
			loadQuotaData();
		}
	});

	async function loadQuotaData() {
		checkingQuota = true;
		quotaError = null;
		try {
			usageData = await client.getUsage();
		} catch (err) {
			console.error('Failed to load quota data:', err);
			// Don't block form if quota check fails
		} finally {
			checkingQuota = false;
		}
	}

	function getResourceUsagePercent(used: number, limit: number): number {
		if (limit === 0) return 0;
		return Math.min(100, Math.round((used / limit) * 100));
	}

	function wouldExceedQuota(): { exceeded: boolean; message: string } {
		if (!usageData) return { exceeded: false, message: '' };

		const { usage, quota } = usageData;

		// Check VMs
		if (usage.vms + 1 > quota.max_vms) {
			return { exceeded: true, message: `VM limit exceeded (${usage.vms}/${quota.max_vms})` };
		}

		// Check CPU
		if (usage.cpus + vcpu > quota.max_cpu) {
			return { exceeded: true, message: `CPU limit would be exceeded (${usage.cpus + vcpu}/${quota.max_cpu})` };
		}

		// Check Memory
		const memoryGB = Math.ceil(memoryMb / 1024);
		if (usage.memory_gb + memoryGB > quota.max_memory_gb) {
			return { exceeded: true, message: `Memory limit would be exceeded (${usage.memory_gb + memoryGB}GB/${quota.max_memory_gb}GB)` };
		}

		// Check Storage (estimate 10GB if can't determine)
		const storageGB = 10;
		if (usage.storage_gb + storageGB > quota.max_storage_gb) {
			return { exceeded: true, message: `Storage limit would be exceeded (${usage.storage_gb + storageGB}GB/${quota.max_storage_gb}GB)` };
		}

		return { exceeded: false, message: '' };
	}

	function getQuotaWarning(): string | null {
		if (!usageData) return null;
		const { usage, quota } = usageData;

		const warnings: string[] = [];

		// Check if approaching limits after this VM
		const vmPercent = getResourceUsagePercent(usage.vms + 1, quota.max_vms);
		const cpuPercent = getResourceUsagePercent(usage.cpus + vcpu, quota.max_cpu);
		const memoryGB = Math.ceil(memoryMb / 1024);
		const memoryPercent = getResourceUsagePercent(usage.memory_gb + memoryGB, quota.max_memory_gb);

		if (vmPercent >= 80) warnings.push(`VM usage will be at ${vmPercent}%`);
		if (cpuPercent >= 80) warnings.push(`CPU usage will be at ${cpuPercent}%`);
		if (memoryPercent >= 80) warnings.push(`Memory usage will be at ${memoryPercent}%`);

		if (warnings.length > 0) {
			return 'Approaching quota limits: ' + warnings.join(', ');
		}

		return null;
	}

	const nameRegex = /^[a-z0-9-]+$/;

	function resetForm() {
		step = 1;
		name = '';
		imageId = '';
		poolId = '';
		networkId = '';
		vcpu = 2;
		memoryMb = 2048;
		consoleType = 'pty';
		userData = '#cloud-config\n';
		username = 'admin';
		sshKey = '';
		formError = '';
		nameError = '';
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
		const quotaCheck = wouldExceedQuota();
		return (
			name.trim() !== '' &&
			nameRegex.test(name) &&
			!name.startsWith('-') &&
			!name.endsWith('-') &&
			imageId !== '' &&
			poolId !== '' &&
			networkId !== '' &&
			!quotaCheck.exceeded
		);
	}

	async function handleSubmit(event?: Event) {
		event?.preventDefault();

		submitting = true;
		formError = '';

		const data = {
			name: name.trim(),
			image_id: imageId,
			storage_pool_id: poolId,
			network_id: networkId,
			vcpu,
			memory_mb: memoryMb,
			user_data: userData,
			username,
			ssh_authorized_keys: sshKey ? [sshKey] : [],
			console_type: 'serial'
		};

		try {
			await client.createVM(data);
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
	const selectedImage = $derived(images.find((i) => i.id === imageId));
	const selectedPool = $derived(pools.find((p) => p.id === poolId));
	const selectedNetwork = $derived(networks.find((n) => n.id === networkId));

	// Reset form when modal closes
	$effect(() => {
		if (!open) {
			resetForm();
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
				<select
					id="vm-image"
					bind:value={imageId}
					class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
					disabled={submitting}
				>
					<option value="">Select an image...</option>
					{#each images as img}
						<option value={img.id}>{img.name} ({img.os_family})</option>
					{/each}
				</select>
			</FormField>

			<FormField label="Storage Pool" required labelFor="vm-pool">
				<select
					id="vm-pool"
					bind:value={poolId}
					class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
					disabled={submitting}
				>
					<option value="">Select a pool...</option>
					{#each pools as pool}
						<option value={pool.id}>{pool.name}</option>
					{/each}
				</select>
			</FormField>

			<FormField label="Network" required labelFor="vm-network">
				<select
					id="vm-network"
					bind:value={networkId}
					class="h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm"
					disabled={submitting}
				>
					<option value="">Select a network...</option>
					{#each networks as net}
						<option value={net.id}>{net.name} ({net.bridge_name})</option>
					{/each}
				</select>
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

			<!-- Quota Check Display -->
			{#if checkingQuota}
				<div class="rounded bg-slate-50 px-3 py-2 text-sm text-slate-600 flex items-center gap-2">
					<svg class="animate-spin h-4 w-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
						<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
						<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
					</svg>
					Checking quota...
				</div>
			{:else if usageData}
				{@const quotaCheck = wouldExceedQuota()}
				{@const warning = getQuotaWarning()}
				{#if quotaCheck.exceeded}
					<div class="rounded border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700" role="alert">
						<div class="font-medium mb-1">⚠️ Quota Exceeded</div>
						<div>{quotaCheck.message}</div>
						<div class="mt-2 text-xs">
							Current usage: {usageData.usage.vms} VMs, {usageData.usage.cpus} CPUs, {usageData.usage.memory_gb}GB RAM
						</div>
					</div>
				{:else if warning}
					<div class="rounded border border-amber-300 bg-amber-50 px-3 py-2 text-sm text-amber-700" role="alert">
						<div class="font-medium mb-1">⚡ Quota Warning</div>
						<div>{warning}</div>
					</div>
				{:else}
					<div class="rounded border border-green-300 bg-green-50 px-3 py-2 text-sm text-green-700">
						<div class="font-medium mb-1">✓ Within Quota</div>
						<div class="text-xs">
							After this VM: {usageData.usage.vms + 1}/{usageData.quota.max_vms} VMs,
							{usageData.usage.cpus + vcpu}/{usageData.quota.max_cpu} CPUs,
							{usageData.usage.memory_gb + Math.ceil(memoryMb / 1024)}/{usageData.quota.max_memory_gb}GB RAM
						</div>
					</div>
				{/if}
			{/if}
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
					<span class="text-muted">Storage:</span>
					<span class="font-medium text-ink">{selectedPool?.name} (localdisk)</span>
				</div>
				<div class="flex justify-between border-b border-line pb-2">
					<span class="text-muted">Network:</span>
					<span class="font-medium text-ink">
						{selectedNetwork?.name} ({selectedNetwork?.bridge_name})
					</span>
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
					<li>qcow2 disk cloned from {selectedImage?.format} image</li>
					<li>seed.iso with cloud-init configuration</li>
					<li>TAP device on {selectedNetwork?.bridge_name} bridge</li>
					<li>VM workspace at /var/lib/chv/vms/{name}</li>
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
