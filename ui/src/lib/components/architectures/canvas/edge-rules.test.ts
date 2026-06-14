import { describe, expect, it } from 'vitest';

import {
	EDGE_RULES,
	MVP_EDGE_TYPES,
	MVP_NODE_KINDS,
	inferEdgeType,
	isEdgeAllowed,
	type EdgeType,
	type NodeKind
} from './edge-rules';

/** Stable ground-truth lookup for whether a triple is in EDGE_RULES. */
function expected(source: NodeKind, target: NodeKind, edgeType: EdgeType): boolean {
	return EDGE_RULES.some(
		(r) => r.source === source && r.target === target && r.edgeType === edgeType
	);
}

describe('EDGE_RULES matrix', () => {
	it('contains exactly the 6 MVP-subset rows from graph-contract.md', () => {
		// 7 contract rows minus 1 post-MVP row (uses_backup_policy) = 6.
		expect(EDGE_RULES.length).toBe(6);
	});

	it('every rule references only MVP node kinds', () => {
		for (const rule of EDGE_RULES) {
			expect(MVP_NODE_KINDS).toContain(rule.source);
			expect(MVP_NODE_KINDS).toContain(rule.target);
		}
	});

	it('every rule references only MVP edge types', () => {
		for (const rule of EDGE_RULES) {
			expect(MVP_EDGE_TYPES).toContain(rule.edgeType);
		}
	});

	it('contains no duplicate (source,target,edgeType) triples', () => {
		const seen = new Set<string>();
		for (const r of EDGE_RULES) {
			const key = `${r.source}|${r.target}|${r.edgeType}`;
			expect(seen.has(key)).toBe(false);
			seen.add(key);
		}
	});

	it('matches the contract rows exactly', () => {
		const expectedRows: ReadonlyArray<{ source: NodeKind; target: NodeKind; edgeType: EdgeType }> = [
			{ source: 'instance', target: 'host', edgeType: 'placed_on' },
			{ source: 'instance', target: 'network', edgeType: 'attached_to_network' },
			{ source: 'instance', target: 'datastore', edgeType: 'uses_datastore' },
			{ source: 'template', target: 'image', edgeType: 'uses_image' },
			{ source: 'instance', target: 'template', edgeType: 'uses_template' },
			{ source: 'user', target: 'role', edgeType: 'has_role' }
		];
		// Compare as multisets (order-independent).
		expect(new Set(EDGE_RULES.map((r) => `${r.source}|${r.target}|${r.edgeType}`))).toEqual(
			new Set(expectedRows.map((r) => `${r.source}|${r.target}|${r.edgeType}`))
		);
	});
});

describe('isEdgeAllowed', () => {
	it('matches "is the triple in EDGE_RULES?" for every (kind,kind,edgeType) combination', () => {
		// 8 × 8 × 6 = 384 cases. Every one is checked.
		let total = 0;
		for (const source of MVP_NODE_KINDS) {
			for (const target of MVP_NODE_KINDS) {
				for (const edgeType of MVP_EDGE_TYPES) {
					total += 1;
					expect(isEdgeAllowed(source, target, edgeType)).toBe(expected(source, target, edgeType));
				}
			}
		}
		expect(total).toBe(MVP_NODE_KINDS.length * MVP_NODE_KINDS.length * MVP_EDGE_TYPES.length);
	});

	it('rejects every triple whose target is the same as source (no self-loops in MVP rules)', () => {
		for (const k of MVP_NODE_KINDS) {
			for (const e of MVP_EDGE_TYPES) {
				expect(isEdgeAllowed(k, k, e)).toBe(false);
			}
		}
	});

	it('rejects swapped direction for every directional rule', () => {
		// e.g. host → instance with placed_on must be false even though
		// instance → host with placed_on is true.
		for (const r of EDGE_RULES) {
			expect(isEdgeAllowed(r.target, r.source, r.edgeType)).toBe(false);
		}
	});

	it('accepts every rule in EDGE_RULES', () => {
		for (const r of EDGE_RULES) {
			expect(isEdgeAllowed(r.source, r.target, r.edgeType)).toBe(true);
		}
	});

	it('rejects post-MVP edge types when used between MVP kinds', () => {
		// `uses_backup_policy` is in EdgeType but not in MVP_EDGE_TYPES.
		// No MVP rule mentions it, so every (any,any,uses_backup_policy)
		// must be rejected.
		for (const source of MVP_NODE_KINDS) {
			for (const target of MVP_NODE_KINDS) {
				expect(isEdgeAllowed(source, target, 'uses_backup_policy')).toBe(false);
			}
		}
	});
});

describe('inferEdgeType', () => {
	it('returns the only allowed edge type for every directional MVP rule', () => {
		for (const r of EDGE_RULES) {
			expect(inferEdgeType(r.source, r.target)).toBe(r.edgeType);
		}
	});

	it('returns null when no rule matches', () => {
		// host → network: not in EDGE_RULES.
		expect(inferEdgeType('host', 'network')).toBeNull();
		// role → user: reverse direction of has_role.
		expect(inferEdgeType('role', 'user')).toBeNull();
		// user → host: not in any rule.
		expect(inferEdgeType('user', 'host')).toBeNull();
	});

	it('covers every (source, target) pair across the 8 kinds', () => {
		// Tabulate the full 64-entry truth table, then assert it agrees with
		// the FIRST matching row in EDGE_RULES (declaration order).
		for (const source of MVP_NODE_KINDS) {
			for (const target of MVP_NODE_KINDS) {
				const firstMatch = EDGE_RULES.find((r) => r.source === source && r.target === target);
				expect(inferEdgeType(source, target)).toBe(firstMatch?.edgeType ?? null);
			}
		}
	});

	it('never returns a self-loop edge for MVP rules', () => {
		for (const k of MVP_NODE_KINDS) {
			expect(inferEdgeType(k, k)).toBeNull();
		}
	});
});
