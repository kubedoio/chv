<script lang="ts">
	import { goto } from '$app/navigation';
	import Button from '$lib/components/primitives/Button.svelte';
	import ArchitectureMetaPanel from '$lib/components/architectures/dashboard/ArchitectureMetaPanel.svelte';
	import StaleVersionBanner from '$lib/components/architectures/dashboard/StaleVersionBanner.svelte';
	import ValidationFindingsPanel from '$lib/components/architectures/dashboard/ValidationFindingsPanel.svelte';
	import YamlSidePanel from '$lib/components/architectures/dashboard/YamlSidePanel.svelte';
	import Canvas from '$lib/components/architectures/canvas/Canvas.svelte';
	import Inspector from '$lib/components/architectures/inspector/Inspector.svelte';
	import { liveState } from '$lib/stores/live-state.svelte';
	import { architectureStore, StaleVersionError } from '$lib/stores/architecture-store.svelte';
	import {
		architectureCanvasStore,
		type GraphPayload
	} from '$lib/stores/architecture-canvas-store.svelte';
	import { architectureDesignerCanvasEnabled } from '$lib/feature-flags';
	import { BFFError } from '$lib/bff/client';
	import type { Architecture, ValidationResult } from '$lib/bff/architectures';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const detail = $derived(data.detail);

	let current = $state<Architecture | null>(null);
	let staleVersion = $state(false);

	// Canvas wiring (Phase 2). The flag default is OFF so production keeps the
	// Phase-1 placeholder; dev / e2e set PUBLIC_ARCHITECTURE_DESIGNER_CANVAS=1.
	const canvasEnabled = architectureDesignerCanvasEnabled();
	let canvasDirty = $state(false);
	let canvasSaving = $state(false);
	let lastHydratedId = $state<string | null>(null);

	// Tab state. The Validation and YAML tabs are Phase 1 additions; Overview
	// remains the default so existing playwright tests stay green.
	type Tab = 'overview' | 'yaml' | 'validation';
	let activeTab = $state<Tab>('overview');

	// Validation panel state. Findings are NOT persisted server-side; we keep
	// the latest result locally and re-fetch when the user clicks Re-validate
	// or re-enters the tab from a fresh load.
	let validationResult = $state<ValidationResult | null>(null);
	let validating = $state(false);

	// YAML side panel state. We also lazy-load on first tab activation.
	let yamlContent = $state<string | null>(null);
	let yamlLoading = $state(false);
	let yamlEmptyReason = $state<string | undefined>(undefined);

	$effect(() => {
		if (detail.state === 'ready') {
			current = detail.architecture;
			staleVersion = false;
			// Seed the YAML panel from the loader payload so a freshly imported
			// topology shows its YAML without requiring a Generate click.
			yamlContent = detail.latestYaml;
			// Hydrate the canvas store from the persisted graph blob, but only
			// on architecture-id transitions — re-hydrating after every save
			// would clobber unsaved local edits.
			if (canvasEnabled && lastHydratedId !== detail.architecture.id) {
				lastHydratedId = detail.architecture.id;
				canvasDirty = false;
				architectureCanvasStore.load(parseGraphBlob(detail.designGraphJson));
			}
		} else {
			current = null;
		}
	});

	const heading = $derived(current ? (current.display_name ?? current.name) : '');

	function handleUpdated(next: Architecture) {
		current = next;
	}

	function handleStaleVersion() {
		staleVersion = true;
	}

	async function handleReload() {
		staleVersion = false;
		await liveState.invalidateAndRefresh({ patterns: ['architectures:'], detailId: current?.id });
	}

	async function handleRevalidate() {
		if (!current || validating) return;
		validating = true;
		try {
			validationResult = await architectureStore.validate(current.id);
		} catch {
			// mutateWithRefresh has already toasted the error.
		} finally {
			validating = false;
		}
	}

	async function handleGenerateYaml() {
		if (!current || yamlLoading) return;
		yamlLoading = true;
		yamlEmptyReason = undefined;
		try {
			yamlContent = await architectureStore.generateYaml(current.id);
		} catch (err) {
			if (err instanceof BFFError && err.code === 'GRAPH_EMPTY') {
				yamlContent = null;
				yamlEmptyReason = 'This topology has no graph yet — design the topology first (Phase 2 canvas) or import YAML.';
			} else {
				yamlEmptyReason = err instanceof Error ? err.message : 'Failed to generate YAML';
			}
		} finally {
			yamlLoading = false;
		}
	}

	function parseGraphBlob(blob: string | null): GraphPayload | null {
		if (!blob) return null;
		try {
			const parsed = JSON.parse(blob);
			if (parsed && typeof parsed === 'object' && parsed.version === '1.0') {
				return parsed as GraphPayload;
			}
		} catch {
			// fall through — malformed blob loads as empty graph
		}
		return null;
	}

	function handleCanvasChange() {
		canvasDirty = true;
	}

	async function handleCanvasSave() {
		if (!current || canvasSaving) return;
		canvasSaving = true;
		try {
			const updated = await architectureCanvasStore.persist(
				current.id,
				current.version_number
			);
			current = updated;
			canvasDirty = false;
			// Findings re-fetch so per-node badges reflect the freshly persisted
			// graph. handleRevalidate() already toasts on its own errors.
			await handleRevalidate();
		} catch (err) {
			if (err instanceof StaleVersionError) {
				staleVersion = true;
			}
			// Any other error has already been toasted by mutateWithRefresh.
		} finally {
			canvasSaving = false;
		}
	}
</script>

<svelte:head>
	<title>
		{current ? `${heading} · Architecture` : 'Architecture · CellHV'}
	</title>
</svelte:head>

<div class="page" data-testid="architecture-detail-page">
	{#if detail.state === 'error' || !current}
		<div class="error-banner" role="alert">
			<strong>Could not load architecture {detail.state === 'error' ? detail.id : ''}.</strong>
			<span>
				{detail.state === 'error' ? detail.errorMessage : 'No data returned by the server.'}
			</span>
			<div>
				<Button variant="secondary" size="sm" onclick={() => goto('/architectures')}>
					Back to list
				</Button>
			</div>
		</div>
	{:else}
		<header class="page-header">
			<div>
				<button
					type="button"
					class="back-link"
					onclick={() => goto('/architectures')}
					aria-label="Back to architectures"
				>
					← Architectures
				</button>
				<h1 class="page-title" data-testid="architecture-name">{heading}</h1>
				<p class="page-subtitle">
					Phase 1 detail view. The visual designer arrives in Phase 2.
				</p>
			</div>
		</header>

		{#if staleVersion}
			<StaleVersionBanner onReload={handleReload} />
		{/if}

		<ArchitectureMetaPanel
			architecture={current}
			onUpdated={handleUpdated}
			onStaleVersion={handleStaleVersion}
		/>

		<div class="tablist" role="tablist" aria-label="Architecture views">
			<button
				type="button"
				role="tab"
				aria-selected={activeTab === 'overview'}
				aria-controls="tab-panel-overview"
				class="tab"
				class:tab-active={activeTab === 'overview'}
				onclick={() => (activeTab = 'overview')}
				data-testid="tab-overview"
			>
				Overview
			</button>
			<button
				type="button"
				role="tab"
				aria-selected={activeTab === 'yaml'}
				aria-controls="tab-panel-yaml"
				class="tab"
				class:tab-active={activeTab === 'yaml'}
				onclick={() => (activeTab = 'yaml')}
				data-testid="tab-yaml"
			>
				YAML
			</button>
			<button
				type="button"
				role="tab"
				aria-selected={activeTab === 'validation'}
				aria-controls="tab-panel-validation"
				class="tab"
				class:tab-active={activeTab === 'validation'}
				onclick={() => (activeTab = 'validation')}
				data-testid="tab-validation"
			>
				Validation
			</button>
		</div>

		{#if activeTab === 'overview'}
			{#if canvasEnabled && current.status !== 'archived'}
				<div
					id="tab-panel-overview"
					class="canvas-shell"
					role="tabpanel"
					aria-labelledby="tab-overview"
					data-testid="canvas-shell"
				>
					<div class="canvas-toolbar" data-testid="canvas-toolbar">
						<div class="toolbar-status">
							{#if architectureCanvasStore.dirty || canvasDirty}
								<span
									class="dirty-dot"
									data-testid="canvas-dirty-indicator"
									role="img"
									aria-label="Unsaved canvas changes"
									title="Unsaved canvas changes"
								></span>
								<span class="toolbar-status-text">Unsaved changes</span>
							{:else}
								<span class="toolbar-status-text">All changes saved</span>
							{/if}
						</div>
						<div class="toolbar-actions">
							<Button
								variant="secondary"
								size="sm"
								onclick={handleRevalidate}
								disabled={validating}
								data-testid="canvas-validate-button"
							>
								{validating ? 'Validating…' : 'Validate'}
							</Button>
							<Button
								variant="primary"
								size="sm"
								onclick={handleCanvasSave}
								disabled={canvasSaving || !(architectureCanvasStore.dirty || canvasDirty)}
								data-testid="canvas-save-button"
							>
								{canvasSaving ? 'Saving…' : 'Save canvas'}
							</Button>
						</div>
					</div>
					<div class="canvas-grid">
						<Canvas
							findings={validationResult?.findings ?? []}
							onChange={handleCanvasChange}
						/>
						<Inspector />
					</div>
				</div>
			{:else}
				<div
					id="tab-panel-overview"
					class="canvas-placeholder"
					role="tabpanel"
					aria-labelledby="tab-overview"
					aria-label="Designer canvas placeholder"
				>
					<div class="placeholder-title">Designer canvas coming in Phase 2</div>
					<p class="placeholder-text">
						This pane will host the Svelte Flow canvas, node palette and inspector
						once the YAML model and validator land. For now, the YAML and Validation
						tabs let you import topologies and see their findings.
					</p>
				</div>
			{/if}
		{:else if activeTab === 'yaml'}
			<div id="tab-panel-yaml" role="tabpanel" aria-labelledby="tab-yaml">
				<YamlSidePanel
					yaml={yamlContent}
					loading={yamlLoading}
					emptyReason={yamlEmptyReason}
					onGenerate={handleGenerateYaml}
				/>
			</div>
		{:else}
			<div id="tab-panel-validation" role="tabpanel" aria-labelledby="tab-validation">
				<ValidationFindingsPanel
					result={validationResult}
					loading={validating}
					onRevalidate={handleRevalidate}
				/>
			</div>
		{/if}
	{/if}
</div>

<style>
	.page {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.page-header {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.back-link {
		background: none;
		border: none;
		padding: 0;
		font-size: var(--text-xs);
		color: var(--color-neutral-500);
		cursor: pointer;
		align-self: flex-start;
	}

	.back-link:hover,
	.back-link:focus-visible {
		color: var(--color-primary);
		outline: none;
	}

	.page-title {
		font-size: var(--text-lg);
		font-weight: 700;
		margin: 0;
	}

	.page-subtitle {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-neutral-600);
	}

	.tablist {
		display: flex;
		gap: 0.25rem;
		border-bottom: 1px solid var(--color-neutral-200);
	}

	.tab {
		appearance: none;
		background: none;
		border: none;
		padding: 0.5rem 0.85rem;
		font-size: var(--text-sm);
		color: var(--color-neutral-600);
		cursor: pointer;
		border-bottom: 2px solid transparent;
		margin-bottom: -1px;
	}

	.tab:hover {
		color: var(--color-neutral-900);
	}

	.tab:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: 2px;
	}

	.tab-active {
		color: var(--color-primary);
		border-bottom-color: var(--color-primary);
		font-weight: 600;
	}

	.canvas-placeholder {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 2rem;
		background: var(--color-neutral-50, #f8fafc);
		border: 1px dashed var(--color-neutral-300);
		border-radius: var(--radius-sm);
		text-align: center;
	}

	.canvas-shell {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.canvas-toolbar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.75rem;
		background: var(--color-neutral-50, #f8fafc);
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-sm);
	}

	.toolbar-status {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: var(--text-xs);
		color: var(--color-neutral-600);
	}

	.toolbar-actions {
		display: flex;
		gap: 0.5rem;
	}

	.dirty-dot {
		display: inline-block;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-warning, #f59e0b);
	}

	.canvas-grid {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(280px, 320px);
		gap: 0.75rem;
		min-height: 520px;
	}

	.placeholder-title {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-neutral-700);
	}

	.placeholder-text {
		margin: 0 auto;
		max-width: 520px;
		font-size: 12px;
		color: var(--color-neutral-500);
		line-height: 1.5;
	}

	.error-banner {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0.75rem 1rem;
		background: rgba(220, 38, 38, 0.08);
		border: 1px solid rgba(220, 38, 38, 0.4);
		border-radius: var(--radius-xs);
		color: rgb(153, 27, 27);
		font-size: var(--text-sm);
	}
</style>
