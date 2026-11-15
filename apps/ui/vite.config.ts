import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { svelteTesting } from '@testing-library/svelte/vite';

export default defineConfig({
	resolve: {
		alias: { $api: fileURLToPath(new URL('../../crates/mw-api/bindings', import.meta.url)) }
	},
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter({ fallback: 'index.html', pages: 'build', assets: 'build' })
		}),
		svelteTesting()
	],
	test: {
		environment: 'jsdom',
		setupFiles: ['src/test-setup.ts'],
		include: ['src/**/*.test.ts', 'scripts/**/*.test.ts'],
		expect: { requireAssertions: true }
	}
});
