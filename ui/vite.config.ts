import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
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
