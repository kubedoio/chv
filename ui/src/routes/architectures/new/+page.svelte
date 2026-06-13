<script lang="ts">
	// New-architecture page. Two paths:
	//
	// 1. Manual sketch — fill the form, server creates an empty draft, user
	//    is redirected into the designer to lay it out by hand.
	// 2. Import from YAML — paste/upload existing topology YAML; we still
	//    need a row to attach it to, so the dialog creates a placeholder
	//    architecture (name derived from the YAML metadata) and immediately
	//    runs `importYaml` against it. The user lands on the freshly
	//    populated detail page with the canonical `latest_yaml` and a
	//    validation pill.
	import { goto } from '$app/navigation';
	import Button from '$lib/components/primitives/Button.svelte';
	import CreateArchitectureForm from '$lib/components/architectures/dashboard/CreateArchitectureForm.svelte';
	import ImportYamlDialog from '$lib/components/architectures/dashboard/ImportYamlDialog.svelte';
	import { architectureStore } from '$lib/stores/architecture-store.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import type { ValidationResult } from '$lib/bff/architectures';

	let importOpen = $state(false);

	// Pull `metadata.name` out of YAML so we can pre-create with a sensible
	// name. Cheap regex avoids pulling a YAML parser into the bundle just for
	// this lookup; if the YAML is malformed the server-side import will fail
	// loudly anyway, so we degrade to a timestamped placeholder.
	function deriveName(yaml: string): string {
		const match = yaml.match(/^\s*metadata:\s*\n(?:[^\n]*\n)*?\s*name:\s*([A-Za-z0-9][A-Za-z0-9-]{1,62})/m);
		if (match) return match[1];
		return `imported-${new Date().toISOString().replace(/[^0-9]/g, '').slice(0, 14)}`;
	}

	async function handleImport(yaml: string): Promise<ValidationResult> {
		// Two-step: create then import. If creation fails the dialog will
		// surface the error via toast; we just rethrow so its catch handler
		// keeps the dialog open.
		const name = deriveName(yaml);
		const arch = await architectureStore.create({
			name,
			description: 'Imported from YAML',
			environment: 'development'
		});
		try {
			const result = await architectureStore.importYaml(arch.id, yaml);
			// Only navigate when there are no blocking errors; if there are,
			// keep the dialog open so the user can iterate. The new draft
			// already exists either way — the user can find it in the list.
			if (result.status !== 'invalid') {
				await goto(`/architectures/${arch.id}`);
			}
			return result;
		} catch (err) {
			toast.error(
				err instanceof Error ? err.message : 'Failed to import YAML into the new architecture'
			);
			throw err;
		}
	}
</script>

<svelte:head>
	<title>New Architecture · CellHV</title>
</svelte:head>

<div class="page">
	<div class="header">
		<h1 class="visually-hidden">New architecture</h1>
		<Button
			variant="secondary"
			size="sm"
			onclick={() => (importOpen = true)}
			ariaLabel="Import from YAML"
			data-testid="open-import-yaml"
		>
			Import from YAML…
		</Button>
	</div>
	<CreateArchitectureForm />
</div>

<ImportYamlDialog
	bind:open={importOpen}
	onSubmit={handleImport}
	onClose={() => (importOpen = false)}
/>

<style>
	.page {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 0.5rem 0;
	}

	.header {
		display: flex;
		justify-content: flex-end;
	}

	.visually-hidden {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
