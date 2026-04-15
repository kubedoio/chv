import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

declare const process: { env?: Record<string, string> };

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
				target: 'http://localhost:8888',
				changeOrigin: true
			}
		}
	}
});
