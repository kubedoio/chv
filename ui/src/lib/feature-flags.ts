/**
 * Feature flag helpers.
 *
 * Flags are read from SvelteKit's `$env/dynamic/public` module so the same
 * compiled bundle responds to env changes without rebuilding. All flags here
 * follow a single rule: a flag is *flipped* only when its env var is the
 * literal string `'1'`. `'true'`, `'on'`, `1` (number), and `true` (boolean)
 * all evaluate as the default. Keep the matrix simple — one switch, one
 * accepted truthy value.
 */

import { env } from '$env/dynamic/public';

/**
 * Returns true when the architecture designer Svelte Flow canvas should
 * mount. **Default ON** as of Phase 4 — the canvas has been stable through
 * Phases 2/3/4 with full E2E coverage. The opt-out env var
 * `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED=1` exists for emergency
 * rollback (e.g. an upstream Svelte Flow regression) without requiring a
 * code change.
 *
 * The legacy `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS=1` opt-in is no longer
 * honoured — flipping the default required inverting the gate to avoid the
 * "set the var or see a placeholder" footgun that hid the canvas from real
 * users while CI quietly passed.
 *
 * Uses `$env/dynamic/public` (read at runtime) so a single compiled bundle
 * supports both states across deployments and lets Playwright's web server
 * toggle without a rebuild.
 */
export function architectureDesignerCanvasEnabled(): boolean {
	return env.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED !== '1';
}
