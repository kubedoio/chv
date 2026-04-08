<script lang="ts">
	import Modal from '$lib/components/Modal.svelte';

	let openDefault = $state(false);
	let openWide = $state(false);
	let openCustomHeader = $state(false);
	let openNoBackdropClose = $state(false);
</script>

<div class="p-8 space-y-6">
	<h1 class="text-2xl font-bold text-ink">Modal Component Test</h1>
	
	<div class="space-y-4">
		<h2 class="text-lg font-semibold text-ink">Basic Usage</h2>
		<div class="flex gap-4">
			<button
				onclick={() => openDefault = true}
				class="px-4 py-2 bg-primary text-white rounded hover:bg-primary/90"
			>
				Open Default Modal
			</button>
			<button
				onclick={() => openWide = true}
				class="px-4 py-2 bg-primary text-white rounded hover:bg-primary/90"
			>
				Open Wide Modal (640px)
			</button>
		</div>
	</div>

	<div class="space-y-4">
		<h2 class="text-lg font-semibold text-ink">Advanced Usage</h2>
		<div class="flex gap-4">
			<button
				onclick={() => openCustomHeader = true}
				class="px-4 py-2 bg-secondary text-ink border border-line rounded hover:bg-chrome"
			>
				Open Custom Header Modal
			</button>
			<button
				onclick={() => openNoBackdropClose = true}
				class="px-4 py-2 bg-secondary text-ink border border-line rounded hover:bg-chrome"
			>
				Open Modal (No Backdrop Close)
			</button>
		</div>
	</div>

	<div class="space-y-2">
		<h2 class="text-lg font-semibold text-ink">Keyboard Navigation Test</h2>
		<p class="text-muted text-sm">When modal is open:</p>
		<ul class="list-disc list-inside text-sm text-muted space-y-1">
			<li>Press ESC to close</li>
			<li>Press Tab to cycle through focusable elements</li>
			<li>Shift+Tab to cycle backwards</li>
			<li>Click backdrop to close (if enabled)</li>
		</ul>
	</div>
</div>

<!-- Default Modal -->
<Modal bind:open={openDefault} title="Confirm Action">
	<p class="text-ink">Are you sure you want to proceed?</p>
	<p class="text-muted text-sm mt-2">This is a default modal with standard width (480px).</p>
	
	{#snippet footer()}
		<button
			onclick={() => openDefault = false}
			class="px-4 py-2 border border-line rounded text-ink hover:bg-chrome"
		>
			Cancel
		</button>
		<button
			onclick={() => { alert('Confirmed!'); openDefault = false; }}
			class="px-4 py-2 bg-primary text-white rounded hover:bg-primary/90"
		>
			Confirm
		</button>
	{/snippet}
</Modal>

<!-- Wide Modal -->
<Modal bind:open={openWide} title="Create Network" width="wide">
	<div class="space-y-4">
		<div>
			<label for="network-name" class="block text-sm font-medium text-ink mb-1">Name</label>
			<input id="network-name" type="text" class="w-full px-3 py-2 border border-line rounded focus:border-primary focus:outline-none" placeholder="Enter network name" />
		</div>
		<div>
			<label for="network-cidr" class="block text-sm font-medium text-ink mb-1">CIDR</label>
			<input id="network-cidr" type="text" class="w-full px-3 py-2 border border-line rounded focus:border-primary focus:outline-none" placeholder="10.0.0.0/24" />
		</div>
		<div>
			<label for="network-gateway" class="block text-sm font-medium text-ink mb-1">Gateway</label>
			<input id="network-gateway" type="text" class="w-full px-3 py-2 border border-line rounded focus:border-primary focus:outline-none" placeholder="10.0.0.1" />
		</div>
	</div>
	
	{#snippet footer()}
		<button
			onclick={() => openWide = false}
			class="px-4 py-2 border border-line rounded text-ink hover:bg-chrome"
		>
			Cancel
		</button>
		<button
			onclick={() => openWide = false}
			class="px-4 py-2 bg-primary text-white rounded hover:bg-primary/90"
		>
			Create
		</button>
	{/snippet}
</Modal>

<!-- Custom Header Modal -->
<Modal bind:open={openCustomHeader}>
	{#snippet header()}
		<div class="flex items-center gap-2">
			<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-primary">
				<circle cx="12" cy="12" r="10"/>
				<path d="M12 16v-4"/>
				<path d="M12 8h.01"/>
			</svg>
			<h2 class="text-base font-semibold text-ink">Custom Header</h2>
		</div>
	{/snippet}
	
	<p class="text-ink">This modal uses a custom header snippet with an icon.</p>
	
	{#snippet footer()}
		<button
			onclick={() => openCustomHeader = false}
			class="px-4 py-2 bg-primary text-white rounded hover:bg-primary/90"
		>
			Got it
		</button>
	{/snippet}
</Modal>

<!-- No Backdrop Close Modal -->
<Modal bind:open={openNoBackdropClose} title="Important Notice" closeOnBackdrop={false}>
	<p class="text-ink">This modal cannot be closed by clicking the backdrop. You must use the buttons or ESC key.</p>
	<p class="text-muted text-sm mt-2">Useful for critical confirmations where accidental dismissal should be prevented.</p>
	
	{#snippet footer()}
		<button
			onclick={() => openNoBackdropClose = false}
			class="px-4 py-2 bg-primary text-white rounded hover:bg-primary/90"
		>
			I Understand
		</button>
	{/snippet}
</Modal>
