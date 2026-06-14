/**
 * Feature flag helpers.
 *
 * Flags are read from SvelteKit's `$env/static/public` module, which is the
 * SvelteKit-canonical way to access `PUBLIC_` env vars at build time. Vite's
 * raw `import.meta.env` does NOT auto-inline `PUBLIC_*` (it only inlines
 * `VITE_*`), so reading those values via `import.meta.env.PUBLIC_*` returns
 * `undefined` in production builds even when the env var was set.
 *
 * `$env/static/public` is processed by SvelteKit's vite plugin and emits a
 * static module that contains every `PUBLIC_*` env var present at build time.
 * Variables absent at build time are simply absent from the module — we
 * defend against that by typing the import as `Partial<...>` via the
 * top-level `// @ts-expect-error` shim below if a flag isn't yet wired.
 *
 * All flags here default to OFF. A flag is enabled only when its env var is
 * the literal string `'1'`. This is intentionally strict: `'true'`,
 * `'on'`, `1` (number), and `true` (boolean) all evaluate to false. Keep the
 * matrix simple — one switch, one accepted truthy value.
 */

import { env } from '$env/dynamic/public';

/**
 * Returns true when the architecture designer Svelte Flow canvas should
 * mount. Default OFF — production builds without `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS=1`
 * keep showing the Phase-1 placeholder.
 *
 * Uses `$env/dynamic/public` (read at runtime) rather than
 * `$env/static/public` (read at build time) so:
 *   1. Tests can flip the flag by setting the var on the dev server without
 *      requiring a full rebuild.
 *   2. The same compiled bundle works in any deployment with the var set.
 *   3. CI environments that run Playwright after a generic `npm run build`
 *      can still toggle the canvas via the run-time env without rebuilding.
 */
export function architectureDesignerCanvasEnabled(): boolean {
	return env.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS === '1';
}
