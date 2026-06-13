import { afterEach, describe, expect, it, vi } from 'vitest';

import { architectureDesignerCanvasEnabled } from './feature-flags';

afterEach(() => {
	vi.unstubAllEnvs();
});

describe('architectureDesignerCanvasEnabled', () => {
	it('returns false when env var is unset', () => {
		vi.stubEnv('PUBLIC_ARCHITECTURE_DESIGNER_CANVAS', '');
		expect(architectureDesignerCanvasEnabled()).toBe(false);
	});

	it('returns true only when env var is exactly "1"', () => {
		vi.stubEnv('PUBLIC_ARCHITECTURE_DESIGNER_CANVAS', '1');
		expect(architectureDesignerCanvasEnabled()).toBe(true);
	});

	it('rejects "true" / "on" / "yes" — only "1" enables', () => {
		for (const v of ['true', 'on', 'yes', 'TRUE', '0', 'False']) {
			vi.stubEnv('PUBLIC_ARCHITECTURE_DESIGNER_CANVAS', v);
			expect(architectureDesignerCanvasEnabled()).toBe(false);
		}
	});
});
