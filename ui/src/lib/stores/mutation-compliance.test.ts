/**
 * CI-breaking mutation compliance tests.
 *
 * These tests scan all +page.svelte files under src/routes/ and enforce that
 * pages use mutateWithRefresh() for cache invalidation instead of manual
 * invalidateAll() / invalidatePattern() calls.
 *
 * If these tests fail, CI must break. Do not weaken the regexes to make them
 * pass — fix the source files instead (or adjust the regex with equivalent
 * strictness).
 *
 * Note: invalidateAll and invalidatePattern are ONLY allowed inside:
 *   - src/lib/stores/mutation.svelte.ts
 *   - src/lib/stores/live-state.svelte.ts
 * The glob pattern routes/…/+page.svelte already excludes those files.
 */

import { describe, expect, it } from 'vitest';

function stripComments(code: string): string {
	// Remove single-line comments
	code = code.replace(/\/\/.*$/gm, '');
	// Remove multi-line comments
	code = code.replace(/\/\*[\s\S]*?\*\//g, '');
	return code;
}

describe('Mutation compliance — all pages must use mutateWithRefresh()', async () => {
	// Dynamic imports avoid pulling Node types into svelte-check's view,
	// which would fail in the jsdom/browser test environment.
	const { globSync } = await import('glob');
	const { readFileSync } = await import('fs');
	const path = await import('path');
	const UI_ROOT = path.resolve(import.meta.dirname, '../../../src');

	const files = globSync('routes/**/+page.svelte', { cwd: UI_ROOT, absolute: true });

	it('scans at least one +page.svelte file', () => {
		expect(files.length, 'No +page.svelte files found — glob pattern or path is broken').toBeGreaterThan(0);
	});

	it('no +page.svelte imports invalidateAll from $app/navigation', () => {
		const offenders = files.filter((f) => {
			const content = readFileSync(f, 'utf-8');
			return /import\s+\{[^}]*invalidateAll[^}]*\}\s+from\s+['"]\$app\/navigation['"]/.test(content);
		});
		expect(
			offenders,
			`Offending files: ${offenders.map((f) => f.replace(UI_ROOT, '')).join(', ')}`
		).toEqual([]);
	});

	it('no +page.svelte imports invalidatePattern from $lib/stores/api-cache.svelte.ts', () => {
		const offenders = files.filter((f) => {
			const content = readFileSync(f, 'utf-8');
			return /import\s+\{[^}]*invalidatePattern[^}]*\}\s+from\s+['"]\$lib\/stores\/api-cache\.svelte\.ts['"]/.test(
				content
			);
		});
		expect(
			offenders,
			`Offending files: ${offenders.map((f) => f.replace(UI_ROOT, '')).join(', ')}`
		).toEqual([]);
	});

	it('no +page.svelte calls invalidateAll() directly', () => {
		const offenders = files.filter((f) => {
			const content = readFileSync(f, 'utf-8');
			return /invalidateAll\s*\(/.test(stripComments(content));
		});
		expect(
			offenders,
			`Offending files: ${offenders.map((f) => f.replace(UI_ROOT, '')).join(', ')}`
		).toEqual([]);
	});

	it('no +page.svelte calls invalidatePattern() directly', () => {
		const offenders = files.filter((f) => {
			const content = readFileSync(f, 'utf-8');
			return /invalidatePattern\s*\(/.test(stripComments(content));
		});
		expect(
			offenders,
			`Offending files: ${offenders.map((f) => f.replace(UI_ROOT, '')).join(', ')}`
		).toEqual([]);
	});

	it('pages importing from $lib/api must also import mutateWithRefresh', () => {
		// Pages that are strictly read-only (no mutations) are exempt from importing
		// mutateWithRefresh even if they import from $lib/api.
		const EXEMPT_READ_ONLY_PAGES: string[] = [];

		const offenders = files.filter((f) => {
			const content = readFileSync(f, 'utf-8');
			// Exclude `import type` (type-only imports do not need mutateWithRefresh)
			const importsApi = /import\s+(?!type\b)[\s\S]*?from\s+['"]\$lib\/api['"]/.test(content);
			const importsMutate = /import\s+[\s\S]*?mutateWithRefresh/.test(content);
			const isExempt = EXEMPT_READ_ONLY_PAGES.some((p) => f.endsWith(p));

			return importsApi && !importsMutate && !isExempt;
		});
		expect(
			offenders,
			`Offending files (import from $lib/api but not mutateWithRefresh): ${offenders.map((f) => f.replace(UI_ROOT, '')).join(', ')}`
		).toEqual([]);
	});

	it('no +page.svelte uses goto with invalidateAll: true', () => {
		// Auth flows legitimately redirect with invalidateAll to refresh session state
		const EXEMPT_GOTO_INVALIDATEALL_PAGES = [
			'login/+page.svelte',
			'change-password/+page.svelte',
		];

		const offenders = files.filter((f) => {
			const content = readFileSync(f, 'utf-8');
			const isExempt = EXEMPT_GOTO_INVALIDATEALL_PAGES.some((p) => f.endsWith(p));
			if (isExempt) return false;
			return /goto\s*\([^)]*,\s*\{[^}]*invalidateAll\s*:\s*true[^}]*\}\s*\)/.test(stripComments(content));
		});
		expect(
			offenders,
			`Offending files (goto with invalidateAll: true): ${offenders.map((f) => f.replace(UI_ROOT, '')).join(', ')}`
		).toEqual([]);
	});
});
