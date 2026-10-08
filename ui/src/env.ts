import { defineEnvVars } from '@sveltejs/kit/env';

/**
 * Public environment variables (SvelteKit 3 `src/env.ts` contract — the
 * successor to reading `$env/dynamic/public`). Both are optional at runtime:
 * unset values are valid, so the schemas tolerate `undefined`.
 *
 * - `PUBLIC_CHV_API_BASE_URL` — base URL for BFF/API requests. Empty string
 *   (the default) means same origin.
 * - `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED` — set to `1` to opt out of
 *   the Architecture Designer canvas (opt-out; anything else keeps it on).
 */
export const variables = defineEnvVars({
	PUBLIC_CHV_API_BASE_URL: {
		public: true,
		schema: (value) => value ?? ''
	},
	PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED: {
		public: true,
		schema: (value) => value
	}
});
