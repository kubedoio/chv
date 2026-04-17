<script lang="ts">
	import Modal from '$lib/components/modals/Modal.svelte';
	import FormField from '$lib/components/forms/FormField.svelte';
	import Input from '$lib/components/Input.svelte';
	import Select from '$lib/components/Select.svelte';
	import { createAPIClient, getStoredToken } from '$lib/api/client';
	import { toast } from '$lib/stores/toast';
	import type { CreateNetworkInput } from '$lib/api/types';

	interface Props {
		open?: boolean;
		onSuccess?: () => void;
	}

	let { open = $bindable(false), onSuccess }: Props = $props();

	const client = createAPIClient({ token: getStoredToken() ?? undefined });

	// Form state
	let name = $state('');
	let mode = $state('bridge');
	let bridgeName = $state('chvbr0');
	let cidr = $state('10.0.0.0/24');
	let gatewayIp = $state('10.0.0.1');
	let submitting = $state(false);
	let formError = $state('');

	// Field-specific errors
	let nameError = $state('');
	let bridgeNameError = $state('');
	let cidrError = $state('');
	let gatewayIpError = $state('');

	// Validation regexes
	const nameRegex = /^[a-z0-9-]+$/;
	const cidrRegex = /^(\d{1,3}\.){3}\d{1,3}\/\d{1,2}$/;
	const ipRegex = /^(\d{1,3}\.){3}\d{1,3}$/;

	const modeOptions = [{ value: 'bridge', label: 'bridge' }];

	function resetForm() {
		name = '';
		mode = 'bridge';
		bridgeName = 'chvbr0';
		cidr = '10.0.0.0/24';
		gatewayIp = '10.0.0.1';
		formError = '';
		nameError = '';
		bridgeNameError = '';
		cidrError = '';
		gatewayIpError = '';
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

	function validateBridgeName(): boolean {
		if (!bridgeName.trim()) {
			bridgeNameError = 'Bridge name is required';
			return false;
		}
		bridgeNameError = '';
		return true;
	}

	function validateCidr(): boolean {
		if (!cidr.trim()) {
			cidrError = 'CIDR is required';
			return false;
		}
		if (!cidrRegex.test(cidr)) {
			cidrError = 'CIDR must be in format x.x.x.x/x (e.g., 10.0.0.0/24)';
			return false;
		}
		// Validate octets are within range
		const [ip, prefix] = cidr.split('/');
		const octets = ip.split('.').map(Number);
		if (octets.some((o) => o < 0 || o > 255)) {
			cidrError = 'IP octets must be between 0 and 255';
			return false;
		}
		const prefixNum = Number(prefix);
		if (prefixNum < 0 || prefixNum > 32) {
			cidrError = 'Prefix must be between 0 and 32';
			return false;
		}
		cidrError = '';
		return true;
	}

	function validateGateway(): boolean {
		if (!gatewayIp.trim()) {
			gatewayIpError = 'Gateway IP is required';
			return false;
		}
		if (!ipRegex.test(gatewayIp)) {
			gatewayIpError = 'Gateway must be a valid IP address (e.g., 10.0.0.1)';
			return false;
		}
		// Validate octets are within range
		const octets = gatewayIp.split('.').map(Number);
		if (octets.some((o) => o < 0 || o > 255)) {
			gatewayIpError = 'IP octets must be between 0 and 255';
			return false;
		}
		gatewayIpError = '';
		return true;
	}

	function validate(): boolean {
		const validations = [validateName(), validateBridgeName(), validateCidr(), validateGateway()];
		return validations.every(Boolean);
	}

	function isValid(): boolean {
		// Quick check without setting errors - used for button disabled state
		if (!name.trim() || !bridgeName.trim() || !cidr.trim() || !gatewayIp.trim()) {
			return false;
		}
		if (!nameRegex.test(name) || name.startsWith('-') || name.endsWith('-')) {
			return false;
		}
		if (!cidrRegex.test(cidr)) return false;
		if (!ipRegex.test(gatewayIp)) return false;

		// Additional validation for octets
		const [ip] = cidr.split('/');
		if (ip.split('.').map(Number).some((o) => o < 0 || o > 255)) return false;
		if (gatewayIp.split('.').map(Number).some((o) => o < 0 || o > 255)) return false;

		return true;
	}

	async function handleSubmit(event?: Event) {
		event?.preventDefault();

		if (!validate()) return;

		submitting = true;
		formError = '';

		const data: CreateNetworkInput = {
			name: name.trim(),
			mode: 'bridge',
			bridge_name: bridgeName.trim(),
			cidr: cidr.trim(),
			gateway_ip: gatewayIp.trim()
		};

		try {
			await client.createNetwork(data);
			toast.success(`Network "${name}" created successfully`);
			open = false;
			onSuccess?.();
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Failed to create network';
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

<Modal bind:open title="Create Network" closeOnBackdrop={!submitting}>
	<form id="create-network-form" onsubmit={handleSubmit} class="space-y-5">
		{#if formError}
			<div class="rounded border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">
				{formError}
			</div>
		{/if}

		<FormField label="Name" error={nameError} required labelFor="network-name">
			<Input
				id="network-name"
				bind:value={name}
				placeholder="my-network"
				disabled={submitting}
				onblur={validateName}
			/>
		</FormField>

		<FormField label="Mode" helper="Only 'bridge' mode is supported in MVP-1" labelFor="network-mode">
			<Select id="network-mode" bind:value={mode} options={modeOptions} disabled />
		</FormField>

		<FormField label="Bridge Name" error={bridgeNameError} required labelFor="bridge-name">
			<Input
				id="bridge-name"
				bind:value={bridgeName}
				placeholder="chvbr0"
				disabled={submitting}
				onblur={validateBridgeName}
			/>
		</FormField>

		<FormField label="CIDR" error={cidrError} required labelFor="network-cidr">
			<Input
				id="network-cidr"
				bind:value={cidr}
				placeholder="10.0.0.0/24"
				disabled={submitting}
				onblur={validateCidr}
			/>
		</FormField>

		<FormField label="Gateway IP" error={gatewayIpError} required labelFor="gateway-ip">
			<Input
				id="gateway-ip"
				bind:value={gatewayIp}
				placeholder="10.0.0.1"
				disabled={submitting}
				onblur={validateGateway}
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
			form="create-network-form"
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
			{submitting ? 'Creating...' : 'Create Network'}
		</button>
	{/snippet}
</Modal>
