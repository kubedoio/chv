import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// Mock `$env/dynamic/public` because vitest doesn't run inside SvelteKit's
// vite plugin pipeline. We expose a mutable `env` object and tweak its
// `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS` field per test.
const mockEnv: { PUBLIC_ARCHITECTURE_DESIGNER_CANVAS?: string } = {};

vi.mock('$env/dynamic/public', () => ({
	env: mockEnv
}));

// Import AFTER vi.mock so the module sees the stub.
const { architectureDesignerCanvasEnabled } = await import('./feature-flags');

beforeEach(() => {
	delete mockEnv.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS;
});

afterEach(() => {
	delete mockEnv.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS;
});

describe('architectureDesignerCanvasEnabled', () => {
	it('returns false when env var is unset', () => {
		expect(architectureDesignerCanvasEnabled()).toBe(false);
	});

	it('returns true only when env var is exactly "1"', () => {
		mockEnv.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS = '1';
		expect(architectureDesignerCanvasEnabled()).toBe(true);
	});

	it('rejects "true" / "on" / "yes" — only "1" enables', () => {
		for (const v of ['true', 'on', 'yes', 'TRUE', '0', 'False']) {
			mockEnv.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS = v;
			expect(architectureDesignerCanvasEnabled()).toBe(false);
		}
	});
});
