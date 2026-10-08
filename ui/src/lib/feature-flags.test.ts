import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// Mock `$app/env/public` because vitest doesn't run inside SvelteKit's
// vite plugin pipeline. The getter keeps the binding live so tests can
// tweak the flag between assertions (the old `$env/dynamic/public` mock
// exposed a mutable `env` object; named exports need the getter instead).
const mockEnv: {
	PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED?: string;
} = {};

vi.mock('$app/env/public', () => ({
	get PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED() {
		return mockEnv.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED;
	}
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
