/**
 * Pure helpers for the Architecture Designer canvas wiring.
 *
 * The Svelte Flow component is configured in `Canvas.svelte`; the small
 * pieces of logic that don't depend on Svelte runes — drag-payload parsing,
 * default-name generation, finding-severity reduction — live here so the
 * Canvas component itself stays under the 300-line component budget.
 */

import type { Finding } from '$lib/bff/architectures';
import type { CanvasNode } from '$lib/stores/architecture-canvas-store.svelte';
import type { NodeKind } from './edge-rules';
import { MVP_NODE_KINDS } from './edge-rules';

/** Drag data type carried by palette items. */
export const PALETTE_DRAG_TYPE = 'application/chv-palette-kind';

/**
 * Read a NodeKind from a `dragover` / `drop` event payload, or `null` if the
 * payload is missing or not a recognised kind. Strict — never coerces.
 */
export function readDraggedKind(event: DragEvent): NodeKind | null {
	const raw = event.dataTransfer?.getData(PALETTE_DRAG_TYPE);
	if (!raw) return null;
	return (MVP_NODE_KINDS as ReadonlyArray<string>).includes(raw) ? (raw as NodeKind) : null;
}

/**
 * Generate a unique default name for a newly dropped node of the given kind.
 * Pattern: `${kind}-N` where N is the smallest positive integer that doesn't
 * collide with an existing node id. The store's `addNode` rejects duplicate
 * ids, so the canvas needs a deterministic generator instead of always
 * picking `${kind}-1`.
 */
export function nextDefaultName(kind: NodeKind, existing: ReadonlyArray<CanvasNode>): string {
	const taken = new Set<string>();
	for (const n of existing) {
		if (n.data.kind === kind && typeof n.data.name === 'string') {
			taken.add(n.data.name);
		}
	}
	let i = 1;
	let candidate = `${kind}-${i}`;
	while (taken.has(candidate)) {
		i += 1;
		candidate = `${kind}-${i}`;
	}
	return candidate;
}

export type FindingSeverity = 'error' | 'warning' | 'info' | 'clean';

/**
 * Reduce the findings list down to a single per-node severity. Errors trump
 * warnings, warnings trump info, and an empty match-set is `clean`.
 *
 * Match key: `<kind>/<name>` against `finding.resource_ref` — matches the
 * shape produced by the Rust validator (see graph-contract.md §"Per-node
 * validation binding key" and the spec's Q3).
 */
export function severityForNode(
	node: CanvasNode,
	findings: ReadonlyArray<Finding>
): FindingSeverity {
	// Defensive: a node persisted from a malformed external graph blob could
	// arrive without a name. The validator's `resource_ref` shape is
	// `<kind>/<name>` with a non-empty name segment, so a blank-named node
	// can never legitimately match — short-circuit so a finding whose ref
	// happens to end in `/` can't be misattributed to the wrong node.
	if (typeof node.data.name !== 'string' || node.data.name.length === 0) {
		return 'clean';
	}
	const ref = `${node.data.kind}/${node.data.name}`;
	let best: FindingSeverity = 'clean';
	for (const f of findings) {
		if (f.resource_ref !== ref) continue;
		if (f.severity === 'error') return 'error'; // can't be beaten
		if (f.severity === 'warning') {
			best = 'warning';
		} else if (f.severity === 'info' && best === 'clean') {
			best = 'info';
		}
	}
	return best;
}
