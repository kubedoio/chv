<script lang="ts">
	import type { NetboxPlanSummary, NetboxProjectionPlan } from '#lib/bff/architectures.ts';
	import { NETBOX_ACTION_LABELS, NETBOX_KIND_LABELS } from './types.ts';

	/**
	 * Deterministic render of the dry-run projection plan.
	 *
	 * The plan arrives ordered by (kind rank, name, action rank) and is
	 * rendered in that order — no client-side re-sorting, so two dry-runs
	 * of the same state paint identically (the contract's determinism
	 * requirement, made visible).
	 *
	 * Conflict rows carry the ownership cue: per the mapping contract no
	 * request is ever sent that would modify an object whose
	 * `chv_managed_by` is not `chv` — a conflict means CHV does not own
	 * the object (foreign owner or occupied natural key) and it is never
	 * written.
	 */

	interface Props {
		plan: NetboxProjectionPlan;
	}

	let { plan }: Props = $props();

	// Chip strip over the summary counts, in the contract's fixed order.
	// Zero counts render dimmed rather than disappearing so the strip is
	// a stable five-slot readout (mirrors DriftSummaryChips).
	const CHIPS: ReadonlyArray<{ key: keyof NetboxPlanSummary; label: string }> = [
		{ key: 'create', label: 'Creates' },
		{ key: 'update', label: 'Updates' },
		{ key: 'no_op', label: 'Unchanged' },
		{ key: 'conflict', label: 'Conflicts' },
		{ key: 'stale', label: 'Stale' }
	];

	function naturalKeyText(entry: NetboxProjectionPlan['entries'][number]): string {
		// BTreeMap-ordered on the server; Object.entries preserves
		// insertion order for JSON-parsed keys, so this is stable.
		return Object.entries(entry.netbox_natural_key)
			.map(([k, v]) => `${k}=${v}`)
			.join(' ');
	}

	const hasConflicts = $derived(plan.summary.conflict > 0);
</script>

<div class="table" data-testid="netbox-dry-run-table">
	<div class="chips" aria-label="Dry-run summary by action">
		{#each CHIPS as chip (chip.key)}
			<span
				class="chip"
				class:chip-zero={plan.summary[chip.key] === 0}
				class:chip-conflict={chip.key === 'conflict' && plan.summary[chip.key] > 0}
				aria-label={`${chip.label}: ${plan.summary[chip.key]}`}
				data-testid="netbox-summary-chip"
				data-netbox-action={chip.key}
			>
				<span class="chip-label">{chip.label}</span>
				<span class="chip-count" data-testid="netbox-summary-chip-count">{plan.summary[chip.key]}</span>
			</span>
		{/each}
	</div>

	<p class="meta" data-testid="netbox-dry-run-meta">
		Projection of applied version {plan.architecture_version}
		(mapping {plan.mapping_version}, retention {plan.retention === 'delete' ? 'delete' : 'mark stale'})
	</p>

	{#if hasConflicts}
		<div
			class="conflict-banner"
			role="alert"
			data-testid="netbox-conflict-banner"
		>
			<strong>{plan.summary.conflict} object{plan.summary.conflict === 1 ? '' : 's'} CHV does not own.</strong>
			<span>
				Conflicting objects exist in NetBox but carry no CHV ownership marker — they are never
				written, and the export proceeds with the remaining entries.
			</span>
		</div>
	{/if}

	{#if plan.entries.length === 0}
		<div class="empty" role="status" data-testid="netbox-dry-run-empty">
			<p class="empty-title">Nothing to project.</p>
			<p class="empty-text">The applied topology maps to no NetBox objects (or NetBox already matches).</p>
		</div>
	{:else}
		<ul class="entries" aria-label="Dry-run plan entries">
			{#each plan.entries as entry, i (i)}
				<li
					class="entry"
					class:entry-conflict={entry.action === 'conflict'}
					data-testid="netbox-plan-entry"
					data-netbox-entry-action={entry.action}
					data-netbox-entry-kind={entry.kind}
				>
					<div class="entry-header">
						<span
							class="action-badge action-{entry.action}"
							role="img"
							aria-label={`Planned action: ${NETBOX_ACTION_LABELS[entry.action] ?? entry.action}`}
							data-testid="netbox-action-badge"
							data-netbox-action={entry.action}
						>
							{NETBOX_ACTION_LABELS[entry.action] ?? entry.action}
						</span>
						<span class="entry-kind" aria-label="NetBox object kind">
							{NETBOX_KIND_LABELS[entry.kind] ?? entry.kind}
						</span>
						<span class="entry-name" data-testid="netbox-entry-name">
							{entry.chv_resource_ref}
						</span>
						<span class="entry-natural-key">{naturalKeyText(entry)}</span>
					</div>
					<div class="entry-reason" data-testid="netbox-entry-reason">{entry.reason}</div>
					{#if entry.action === 'conflict'}
						<div class="ownership-cue" data-testid="netbox-conflict-cue">
							CHV does not own this object — it will not be written.
						</div>
					{/if}
					{#if entry.changes.length > 0}
						<ul class="changes" aria-label="Field changes">
							{#each entry.changes as change, j (j)}
								<li class="change" data-testid="netbox-entry-change">{change}</li>
							{/each}
						</ul>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.table { display: flex; flex-direction: column; gap: 0.6rem; }
	.chips { display: flex; flex-wrap: wrap; gap: 0.4rem; }
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.15rem 0.5rem 0.15rem 0.55rem;
		font-size: 11px;
		font-weight: 600;
		border-radius: var(--radius-xs);
		background: var(--color-neutral-50, #f8fafc);
		border: 1px solid var(--color-neutral-200);
		color: var(--color-neutral-600);
	}
	.chip-zero { opacity: 0.7; }
	.chip-conflict {
		background: rgba(180, 83, 9, 0.1);
		border-color: rgba(180, 83, 9, 0.35);
		color: rgb(120, 53, 15);
	}
	.chip-count { font-variant-numeric: tabular-nums; font-weight: 700; }
	.meta { margin: 0; font-size: 12px; color: var(--color-neutral-500); }
	.conflict-banner {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		padding: 0.6rem 0.85rem;
		border-radius: var(--radius-xs);
		background: rgba(180, 83, 9, 0.1);
		border: 1px solid rgba(180, 83, 9, 0.4);
		color: rgb(120, 53, 15);
		font-size: var(--text-sm);
	}
	.empty {
		padding: 1rem;
		border: 1px dashed var(--color-neutral-300);
		border-radius: var(--radius-xs);
		text-align: center;
		background: var(--color-neutral-50, #f8fafc);
	}
	.empty-title {
		margin: 0;
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-neutral-700);
	}
	.empty-text { margin: 0.25rem 0 0 0; font-size: 12px; color: var(--color-neutral-600); }
	.entries { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.5rem; }
	.entry {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		padding: 0.6rem 0.75rem;
		background: var(--bg-surface);
		border: 1px solid var(--color-neutral-200);
		border-radius: var(--radius-xs);
	}
	.entry-conflict {
		border-color: rgba(180, 83, 9, 0.45);
		background: rgba(180, 83, 9, 0.04);
	}
	.entry-header { display: flex; align-items: baseline; gap: 0.5rem; flex-wrap: wrap; }
	.action-badge {
		display: inline-block;
		padding: 0.1rem 0.5rem;
		border-radius: var(--radius-xs);
		font-size: 11px;
		font-weight: 700;
		letter-spacing: 0.02em;
		color: white;
	}
	.action-create { background: #15803d; }
	.action-update { background: #1d4ed8; }
	.action-no_op { background: #6b7280; }
	.action-conflict { background: #b45309; }
	.action-stale { background: #7c3aed; }
	.entry-kind {
		font-size: 11px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-neutral-500);
	}
	.entry-name {
		font-family: var(--font-mono, ui-monospace, monospace);
		font-size: 12px;
		color: var(--color-neutral-700);
	}
	.entry-natural-key {
		font-family: var(--font-mono, ui-monospace, monospace);
		font-size: 11px;
		color: var(--color-neutral-400);
	}
	.entry-reason { font-size: var(--text-sm); color: var(--color-neutral-800); line-height: 1.4; }
	.ownership-cue { font-size: 12px; font-weight: 600; color: rgb(120, 53, 15); }
	.changes { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.15rem; }
	.change {
		padding: 0.2rem 0.5rem;
		background: var(--color-neutral-50, #f8fafc);
		border: 1px dashed var(--color-neutral-200);
		border-radius: var(--radius-xs);
		font-family: var(--font-mono, ui-monospace, monospace);
		font-size: 11px;
		color: var(--color-neutral-700);
	}
</style>
