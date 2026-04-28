<script lang="ts">
	import Modal from '$lib/components/primitives/Modal.svelte';
	import { createVm } from '$lib/bff/vms';
	import { listImages } from '$lib/bff/images';
	import { listNetworks } from '$lib/bff/networks';
	import { getStoredToken } from '$lib/api/client';
	import { toast } from '$lib/stores/toast';
	import VmStep1Form from './VmStep1Form.svelte';
	import VmStep2Form from './VmStep2Form.svelte';
	import VmReviewPanel from './VmReviewPanel.svelte';

	interface Props {
		open?: boolean;
		onSuccess?: () => void;
	}

	let {
		open = $bindable(false),
		onSuccess
	}: Props = $props();

	let step = $state(1);

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

	// Advanced hypervisor overrides
	let advancedOpen = $state(false);
	let hvOverrides = $state<Record<string, boolean | string | undefined>>({});

	let submitting = $state(false);
	let formError = $state('');
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
		advancedOpen = false;
		hvOverrides = {};
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
			if (images.length > 0 && !imageId) {
				imageId = images[0].image_id;
			}
			if (networks.length > 0 && !networkId) {
				const defaultNet = networks.find((n) => n.is_default);
				networkId = defaultNet ? defaultNet.network_id : networks[0].network_id;
			}
		} catch (e) {
			// TODO: integrate structured logger instead of console
			// eslint-disable-next-line no-console
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

		let cloudInitUserdata: string | undefined;
		const hasUser = username.trim() !== '';
		const hasSshKey = sshKey.trim() !== '';
		const hasCustomUserdata = userData.trim() !== '' && userData.trim() !== '#cloud-config';

		if (hasUser || hasSshKey || hasCustomUserdata) {
			let parts: string[] = ['#cloud-config'];
			if (hasUser || hasSshKey) {
				parts.push('users:');
				parts.push(`  - name: ${username.trim() || 'admin'}`);
				parts.push('    sudo: ALL=(ALL) NOPASSWD:ALL');
				parts.push('    shell: /bin/bash');
				if (hasSshKey) {
					parts.push('    ssh_authorized_keys:');
					for (const key of sshKey.trim().split('\n').filter(k => k.trim())) {
						parts.push(`      - ${key.trim()}`);
					}
				}
			}
			if (hasCustomUserdata) {
				const customLines = userData.trim().split('\n');
				const startIdx = customLines[0].trim() === '#cloud-config' ? 1 : 0;
				const extra = customLines.slice(startIdx).join('\n').trim();
				if (extra) {
					parts.push(extra);
				}
			}
			cloudInitUserdata = parts.join('\n') + '\n';
		}

		const overrides: Record<string, boolean | string> = {};
		for (const [key, value] of Object.entries(hvOverrides)) {
			if (value !== undefined) overrides[key] = value;
		}

		const data = {
			name: name.trim(),
			image_id: imageId,
			network_id: networkId || 'default',
			vcpu,
			memory_mb: memoryMb,
			volume_size_gb: volumeSizeGb,
			requested_by: 'webui',
			...(cloudInitUserdata ? { cloud_init_userdata: cloudInitUserdata } : {}),
			...(Object.keys(overrides).length > 0 ? { hypervisor_overrides: overrides } : {})
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

	const selectedImage = $derived(images.find((i) => i.image_id === imageId) ?? { name: imageId || '—' });
	const selectedNetwork = $derived(networks.find((n) => n.network_id === networkId) ?? { name: networkId || '—' });

	const overrideCount = $derived(Object.values(hvOverrides).filter((v) => v !== undefined).length);
	const overrideSummary = $derived(() => {
		if (overrideCount === 0) return 'Inherit global settings';
		const entries = Object.entries(hvOverrides)
			.filter(([, v]) => v !== undefined)
			.map(([k, v]) => `${k}=${v}`);
		return `Custom (${overrideCount} override${overrideCount === 1 ? '' : 's'}): ${entries.join(', ')}`;
	});

	$effect(() => {
		if (!open) {
			resetForm();
		} else {
			loadData();
		}
	});
</script>

<Modal bind:open title="Create VM - Step {step} of 3" closeOnBackdrop={!submitting} width="wide">
	{#if formError}
		<div
			class="rounded border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger mb-4"
			role="alert"
		>
			{formError}
		</div>
	{/if}

	{#if step === 1}
		<VmStep1Form
			bind:name
			bind:imageId
			bind:networkId
			bind:vcpu
			bind:memoryMb
			bind:volumeSizeGb
			bind:nameError
			{images}
			{networks}
			{loadingData}
			bind:advancedOpen
			bind:hvOverrides
			{submitting}
			onNameBlur={validateName}
			onImageLinkClick={() => (open = false)}
		/>
	{:else if step === 2}
		<VmStep2Form
			bind:username
			bind:sshKey
			bind:userData
			{submitting}
		/>
	{:else}
		<VmReviewPanel
			{name}
			selectedImageName={selectedImage.name}
			selectedNetworkName={selectedNetwork.name}
			{volumeSizeGb}
			{vcpu}
			{memoryMb}
			{username}
			overrideSummary={overrideSummary()}
		/>
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
