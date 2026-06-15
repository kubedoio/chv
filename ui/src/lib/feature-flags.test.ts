import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// Mock `$env/dynamic/public` because vitest doesn't run inside SvelteKit's
// vite plugin pipeline. We expose a mutable `env` object and tweak its
// flag fields per test.
const mockEnv: {
	PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED?: string;
} = {};

vi.mock('$env/dynamic/public', () => ({
	env: mockEnv
}));

// Import AFTER vi.mock so the module sees the stub.
const { architectureDesignerCanvasEnabled } = await import('./feature-flags');

beforeEach(() => {
	delete mockEnv.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED;
});

afterEach(() => {
	delete mockEnv.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED;
});

describe('architectureDesignerCanvasEnabled', () => {
	it('defaults to true when the disable env var is unset', () => {
		expect(architectureDesignerCanvasEnabled()).toBe(true);
	});

	it('returns false only when the disable env var is exactly "1"', () => {
		mockEnv.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED = '1';
		expect(architectureDesignerCanvasEnabled()).toBe(false);
	});

	it('only "1" disables — "true" / "on" / etc. keep the canvas mounted', () => {
		for (const v of ['true', 'on', 'yes', 'TRUE', '0', 'False', '']) {
			mockEnv.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED = v;
			expect(architectureDesignerCanvasEnabled()).toBe(true);
		}
	});
});
