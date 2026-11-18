import { expect, it } from 'vitest';

const styles = import.meta.glob('/src/**/*.{svelte,css}', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;
const scripts = import.meta.glob('/src/**/*.{svelte,ts}', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;

function stripComments(s: string): string {
	return s.replace(/\/\*[\s\S]*?\*\//g, '').replace(/<!--[\s\S]*?-->/g, '');
}

// The layout follows the UI scale: widths come from `display` and `[data-width]`, never from the window.
it('has no width media queries in styles', () => {
	expect(Object.keys(styles).length).toBeGreaterThan(0);
	const bad = Object.entries(styles)
		.filter(([, src]) => /@media[^{]*\b(?:min|max)?-?width\b/.test(stripComments(src)))
		.map(([p]) => p);
	expect(bad).toEqual([]);
});

it('does not build MediaQuery objects', () => {
	const bad = Object.entries(scripts)
		.filter(([p]) => p !== '/src/lib/design/no-width-media.test.ts')
		.filter(([, src]) => /new\s+MediaQuery\s*\(/.test(stripComments(src)))
		.map(([p]) => p);
	expect(bad).toEqual([]);
});
