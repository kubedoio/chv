import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

declare const process: { env?: Record<string, string> };

const bffProxyTarget =
	process.env?.CHV_WEBUI_BFF_PROXY_TARGET ??
	process.env?.PUBLIC_CHV_API_PROXY_TARGET ??
	'http://localhost:8888';

export default defineConfig({
	plugins: [sveltekit()],
	resolve:
		typeof process !== 'undefined' && process.env?.VITEST
			? {
					conditions: ['browser']
				}
			: undefined,
	test: {
		environment: 'jsdom',
		include: ['src/**/*.test.ts']
	},
	server: {
		proxy: {
			'/api': {
				target: bffProxyTarget,
				changeOrigin: true
			},
			'/v1': {
				target: bffProxyTarget,
				changeOrigin: true
			},
			'/ws': {
				target: bffProxyTarget,
				changeOrigin: true,
				ws: true
			}
		}
	}
});
