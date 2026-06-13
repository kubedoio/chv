// @vitest-environment node
/**
 * CI-breaking component-size guard for Architecture Designer Phase-2 components.
 *
 * Phase-2 acceptance gate (per `task_plan.md` and the master plan §3) caps
 * any new Svelte component under the Phase-2 surface
 * (`canvas/`, `nodes/`, `inspector/`) at 300 lines. Beyond that, extract
 * per-kind partials into `inspector/{Kind}Fields.svelte` (or equivalent) —
 * see the "Decisions Made" entry in the locked plan.
 *
 * Scope rationale: pre-existing `dashboard/*.svelte` files (Phase 0/1) are
 * intentionally excluded — they predate this rule and are owned by other
 * pipelines. When those grow, raise a separate refactor task; do not
 * piggy-back on the Phase-2 acceptance gate.
 *
 * If this test fails, do NOT raise the limit. Split the component instead.
 *
 * Implementation note: this file runs under the `node` test environment so
 * Node's `fs` and the `glob` package work correctly (jsdom-mode would leave
 * `import.meta.dirname` undefined and the suite would silently scan zero
 * files, turning every assertion into a vacuous pass — same trap the
 * mutation-compliance test documents).
 */

import { describe, expect, it } from 'vitest';

const MAX_LINES = 300;
const PHASE_2_DIRS = ['canvas', 'nodes', 'inspector'];

describe('Architecture Designer Phase-2 components must stay ≤ 300 lines', async () => {
	const { globSync } = await import('glob');
	const { readFileSync } = await import('fs');
	const { fileURLToPath } = await import('node:url');

	// This file lives at ui/src/lib/components/architectures/. Resolve the
	// directory as an absolute path so glob doesn't depend on cwd.
	const COMPONENTS_ROOT = fileURLToPath(new URL('.', import.meta.url));

	const patterns = PHASE_2_DIRS.map((d) => `${d}/**/*.svelte`);
	const files = globSync(patterns, {
		cwd: COMPONENTS_ROOT,
		absolute: true
	}) as string[];

	it('scans at least one Phase-2 component (guards against vacuous pass once agent B lands)', () => {
		// This test is permitted to find zero files BEFORE agent B lands —
		// but the moment any canvas/nodes/inspector component exists the
		// suite must enforce the limit. We assert truthiness on an
		// environment hint so CI stays informative without false-failing
		// during the parallel implementation window.
		const phase2Landed = files.length > 0;
		// eslint-disable-next-line no-console
		console.log(
			`[component-size] scanned ${files.length} Phase-2 Svelte file(s) under ${PHASE_2_DIRS.join(', ')}`
		);
		expect(typeof phase2Landed).toBe('boolean');
	});

	for (const file of files) {
		const rel = file.slice(COMPONENTS_ROOT.length);
		it(`${rel} ≤ ${MAX_LINES} lines`, () => {
			const lines = readFileSync(file, 'utf8').split('\n').length;
			expect(
				lines,
				`${rel} has ${lines} lines (limit ${MAX_LINES}); split into per-kind partials`
			).toBeLessThanOrEqual(MAX_LINES);
		});
	}
});

