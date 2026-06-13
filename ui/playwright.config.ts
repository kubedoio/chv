import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
	webServer: {
		command: 'npm run build && npm run preview',
		port: 4173,
		reuseExistingServer: !process.env.CI,
		timeout: 120000,
		// Phase 2 architecture-designer canvas feature flag: required for the
		// `architectures-canvas.spec.ts` suite. Keep this set so the e2e canvas
		// tests can mount the Svelte Flow canvas. DO NOT REVERT — flag default
		// is OFF in production, ON for e2e/dev. See `ui/src/lib/feature-flags.ts`.
		env: {
			PUBLIC_ARCHITECTURE_DESIGNER_CANVAS: '1'
		}
	},
	testDir: 'tests/e2e',
	testMatch: /(.+\.)?(test|spec)\.[jt]s/,
	retries: process.env.CI ? 2 : 0,
	workers: process.env.CI ? 1 : undefined,
	reporter: 'html',
	use: {
		baseURL: 'http://localhost:4173',
		trace: 'on-first-retry',
		// Set BFF_BASE_URL so that the node server processes it correctly if needed
		extraHTTPHeaders: {}
	},
	projects: [
		{
			name: 'chromium',
			use: { ...devices['Desktop Chrome'] }
		}
	]
});
