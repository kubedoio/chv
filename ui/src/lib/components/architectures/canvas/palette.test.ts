import { describe, expect, it } from 'vitest';

import { MVP_NODE_KINDS, type NodeKind } from './edge-rules';
import { PALETTE, type PaletteEntry } from './palette';

/**
 * Compile-time exhaustiveness: build a record keyed by every `NodeKind` and
 * mapped to the corresponding palette entry. If a `NodeKind` is added to
 * `edge-rules.ts` without a matching `PALETTE` entry, this object literal
 * will fail to type-check, and `satisfies` makes the failure precise.
 *
 * The `Record<NodeKind, PaletteEntry>` type forces every `NodeKind` key to
 * appear; the `satisfies` clause keeps the inferred value type narrow so
 * that downstream code still gets `PaletteEntry`.
 */
const PALETTE_BY_KIND = {
	host: PALETTE.find((e) => e.kind === 'host'),
	network: PALETTE.find((e) => e.kind === 'network'),
	datastore: PALETTE.find((e) => e.kind === 'datastore'),
	image: PALETTE.find((e) => e.kind === 'image'),
	template: PALETTE.find((e) => e.kind === 'template'),
	instance: PALETTE.find((e) => e.kind === 'instance'),
	user: PALETTE.find((e) => e.kind === 'user'),
	role: PALETTE.find((e) => e.kind === 'role')
} satisfies Record<NodeKind, PaletteEntry | undefined>;

describe('PALETTE registry', () => {
	it('has exactly one entry per MVP node kind', () => {
		expect(PALETTE.length).toBe(MVP_NODE_KINDS.length);
		for (const kind of MVP_NODE_KINDS) {
			const matches = PALETTE.filter((e) => e.kind === kind);
			expect(matches.length).toBe(1);
		}
	});

	it('every entry has non-empty label, description, and icon', () => {
		for (const entry of PALETTE) {
			expect(entry.label.length).toBeGreaterThan(0);
			expect(entry.description.length).toBeGreaterThan(0);
			expect(entry.icon.length).toBeGreaterThan(0);
		}
	});

	it('compile-time exhaustiveness check finds every NodeKind', () => {
		// If TS compiles this file, the `satisfies Record<NodeKind, ...>`
		// has already proven exhaustiveness. At runtime, double-check that
		// no entry came back undefined.
		for (const kind of MVP_NODE_KINDS) {
			expect(PALETTE_BY_KIND[kind]).toBeDefined();
			expect(PALETTE_BY_KIND[kind]?.kind).toBe(kind);
		}
	});
});
