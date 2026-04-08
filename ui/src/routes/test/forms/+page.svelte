<script lang="ts">
	import Input from '$lib/components/Input.svelte';
	import Select from '$lib/components/Select.svelte';
	import FormField from '$lib/components/FormField.svelte';

	// Test values
	let nameValue = $state('');
	let emailValue = $state('');
	let typeValue = $state('');
	let categoryValue = $state('');

	// Test errors
	let nameError = $state('');
	let emailError = $state('Email is required');

	const typeOptions = [
		{ value: 'bridge', label: 'Bridge' },
		{ value: 'nat', label: 'NAT' },
		{ value: 'host-only', label: 'Host Only' },
		{ value: 'disabled-opt', label: 'Disabled Option', disabled: true }
	];

	const categoryOptions = [
		{ value: 'vm', label: 'Virtual Machine' },
		{ value: 'network', label: 'Network' },
		{ value: 'storage', label: 'Storage' }
	];

	function toggleNameError() {
		nameError = nameError ? '' : 'Name must be at least 3 characters';
	}
</script>

<div class="p-8 max-w-2xl">
	<h1 class="text-2xl font-semibold mb-8">Form Components Test</h1>

	<!-- Input Section -->
	<section class="mb-12">
		<h2 class="text-lg font-medium mb-4 pb-2 border-b border-line">Input Component</h2>
		
		<div class="space-y-6">
			<!-- Default -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">Default State</h3>
				<Input bind:value={nameValue} placeholder="Enter your name" />
			</div>

			<!-- Focus (click to see) -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">Focus State (click input)</h3>
				<Input placeholder="Click to focus" />
			</div>

			<!-- With Value -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">With Value</h3>
				<Input value="John Doe" />
			</div>

			<!-- Disabled -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">Disabled State</h3>
				<Input value="Disabled value" disabled />
			</div>

			<!-- Error State -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">Error State</h3>
				<Input bind:value={emailValue} error={emailError} placeholder="Enter email" type="email" />
			</div>

			<!-- Different Types -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">Password Type</h3>
				<Input type="password" value="secret123" placeholder="Enter password" />
			</div>

			<div>
				<h3 class="text-sm font-medium text-muted mb-2">Number Type</h3>
				<Input type="number" placeholder="Enter number" />
			</div>
		</div>
	</section>

	<!-- Select Section -->
	<section class="mb-12">
		<h2 class="text-lg font-medium mb-4 pb-2 border-b border-line">Select Component</h2>
		
		<div class="space-y-6">
			<!-- Default with placeholder -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">With Placeholder</h3>
				<Select bind:value={typeValue} options={typeOptions} placeholder="Select network type" />
				<p class="text-xs text-muted mt-2">Selected: {typeValue || '(none)'}</p>
			</div>

			<!-- Pre-selected value -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">Pre-selected Value</h3>
				<Select value="vm" options={categoryOptions} />
			</div>

			<!-- Disabled -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">Disabled State</h3>
				<Select options={categoryOptions} disabled />
			</div>

			<!-- Error State -->
			<div>
				<h3 class="text-sm font-medium text-muted mb-2">Error State</h3>
				<Select options={categoryOptions} error="Please select a category" />
			</div>
		</div>
	</section>

	<!-- FormField Section -->
	<section class="mb-12">
		<h2 class="text-lg font-medium mb-4 pb-2 border-b border-line">FormField Component</h2>
		
		<div class="space-y-8">
			<!-- Basic -->
			<FormField label="Full Name">
				<Input bind:value={nameValue} placeholder="Enter your full name" />
			</FormField>

			<!-- With Helper -->
			<FormField label="Bridge Name" helper="Name of the bridge interface on the host">
				<Input placeholder="e.g., chvbr0" />
			</FormField>

			<!-- Required -->
			<FormField label="Email Address" required>
				<Input type="email" placeholder="user@example.com" />
			</FormField>

			<!-- With Error (toggleable) -->
			<FormField label="Username" error={nameError} required>
				<Input bind:value={nameValue} placeholder="Choose a username" />
			</FormField>
			<button
				onclick={toggleNameError}
				class="mt-2 px-3 py-1.5 text-sm border border-line rounded hover:bg-chrome transition-colors"
			>
				{nameError ? 'Clear Error' : 'Show Error'}
			</button>

			<!-- Select in FormField -->
			<FormField label="Resource Type" helper="Select the type of resource to create">
				<Select options={categoryOptions} placeholder="Select type" />
			</FormField>

			<!-- Disabled FormField -->
			<FormField label="Instance ID" helper="Auto-generated identifier">
				<Input value="inst-12345" disabled />
			</FormField>
		</div>
	</section>

	<!-- Combined Example -->
	<section class="mb-12">
		<h2 class="text-lg font-medium mb-4 pb-2 border-b border-line">Create Network Example</h2>
		
		<div class="p-6 border border-line rounded-lg bg-white space-y-6">
			<FormField label="Network Name" required helper="Unique name for this network">
				<Input placeholder="e.g., production-vms" />
			</FormField>

			<FormField label="Mode" required>
				<Select options={[{ value: 'bridge', label: 'Bridge' }]} value="bridge" disabled />
			</FormField>

			<FormField label="Bridge Interface" required helper="Host bridge interface name">
				<Input placeholder="e.g., br0" />
			</FormField>

			<FormField label="CIDR" required helper="Network CIDR in format x.x.x.x/x">
				<Input placeholder="e.g., 10.0.0.0/24" />
			</FormField>

			<FormField label="Gateway IP" helper="Default gateway for this network">
				<Input placeholder="e.g., 10.0.0.1" />
			</FormField>

			<div class="flex gap-3 pt-4 border-t border-line">
				<button class="button-primary px-4 py-2 rounded text-sm font-medium">
					Create Network
				</button>
				<button class="button-secondary px-4 py-2 rounded text-sm">
					Cancel
				</button>
			</div>
		</div>
	</section>
</div>
