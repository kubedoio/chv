<script lang="ts">
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import { toast } from '$lib/stores/toast';

	// Test state
	let showDangerDialog = $state(false);
	let showPrimaryDialog = $state(false);
	let lastAction = $state<string | null>(null);

	function handleDangerConfirm() {
		lastAction = 'Danger confirmed - Item deleted!';
		toast.success('Item deleted successfully');
	}

	function handleDangerCancel() {
		lastAction = 'Danger cancelled';
	}

	function handlePrimaryConfirm() {
		lastAction = 'Primary confirmed - Action completed!';
		toast.success('Action completed');
	}

	function handlePrimaryCancel() {
		lastAction = 'Primary cancelled';
	}
</script>

<div class="container mx-auto max-w-4xl p-8">
	<h1 class="mb-8 text-2xl font-bold text-ink">ConfirmDialog Component Test</h1>

	<div class="space-y-8">
		<!-- Test Section: Danger Variant -->
		<section class="rounded-lg border border-line bg-white p-6">
			<h2 class="mb-4 text-lg font-semibold text-ink">Danger Variant (Default)</h2>
			<p class="mb-4 text-sm text-muted">
				Used for destructive actions like delete, remove, or irreversible operations.
			</p>
			<button
				onclick={() => (showDangerDialog = true)}
				class="rounded border border-danger px-4 py-2 text-danger transition-colors hover:bg-danger/5"
			>
				Open Danger Dialog
			</button>
		</section>

		<!-- Test Section: Primary Variant -->
		<section class="rounded-lg border border-line bg-white p-6">
			<h2 class="mb-4 text-lg font-semibold text-ink">Primary Variant</h2>
			<p class="mb-4 text-sm text-muted">
				Used for non-destructive confirmations that still need user approval.
			</p>
			<button
				onclick={() => (showPrimaryDialog = true)}
				class="rounded bg-primary px-4 py-2 text-white transition-colors hover:bg-primary/90"
			>
				Open Primary Dialog
			</button>
		</section>

		<!-- Action Log -->
		<section class="rounded-lg border border-line bg-chrome p-6">
			<h2 class="mb-4 text-lg font-semibold text-ink">Action Log</h2>
			{#if lastAction}
				<p class="text-sm text-ink" data-testid="action-log">{lastAction}</p>
			{:else}
				<p class="text-sm text-muted">No actions yet. Click the buttons above to test.</p>
			{/if}
		</section>

		<!-- Test Checklist -->
		<section class="rounded-lg border border-line bg-white p-6">
			<h2 class="mb-4 text-lg font-semibold text-ink">Verification Checklist</h2>
			<ul class="space-y-2 text-sm text-muted">
				<li class="flex items-center gap-2">
					<span class="text-success">✓</span>
					<span>Title prop displays correctly</span>
				</li>
				<li class="flex items-center gap-2">
					<span class="text-success">✓</span>
					<span>Description text shows consequences</span>
				</li>
				<li class="flex items-center gap-2">
					<span class="text-success">✓</span>
					<span>Warning icon appears for danger variant</span>
				</li>
				<li class="flex items-center gap-2">
					<span class="text-success">✓</span>
					<span>Danger button has red border and text</span>
				</li>
				<li class="flex items-center gap-2">
					<span class="text-success">✓</span>
					<span>Cancel button uses secondary style</span>
				</li>
				<li class="flex items-center gap-2">
					<span class="text-success">✓</span>
					<span>Buttons are right-aligned with 8px gap</span>
				</li>
				<li class="flex items-center gap-2">
					<span class="text-success">✓</span>
					<span>Confirm button is focused when opened</span>
				</li>
				<li class="flex items-center gap-2">
					<span class="text-success">✓</span>
					<span>onConfirm callback fires when confirmed</span>
				</li>
				<li class="flex items-center gap-2">
					<span class="text-success">✓</span>
					<span>onCancel callback fires when cancelled</span>
				</li>
			</ul>
		</section>
	</div>
</div>

<!-- Danger Dialog -->
<ConfirmDialog
	bind:open={showDangerDialog}
	title="Delete VM?"
	description="This action cannot be undone. The VM data will be permanently removed."
	confirmText="Delete"
	variant="danger"
	onConfirm={handleDangerConfirm}
	onCancel={handleDangerCancel}
/>

<!-- Primary Dialog -->
<ConfirmDialog
	bind:open={showPrimaryDialog}
	title="Apply Changes?"
	description="This will update the configuration and restart the service. The service will be unavailable for a few seconds."
	confirmText="Apply"
	variant="primary"
	onConfirm={handlePrimaryConfirm}
	onCancel={handlePrimaryCancel}
/>
